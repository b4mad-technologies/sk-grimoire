use crate::{Diagnostic, Severity};
use serde_json::Value;
use spellkave_schema::Manifest;

pub const MF_VAL_AREA_SHAPE_PARAMS: &str = "MF-VAL-361";

pub fn validate_area_shape_params(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for bundle in &manifest.bundles {
        validate_params(
            &bundle.default_extent_shape_id,
            &bundle.default_extent_params_json,
            "bundles.default_extent_params_json",
            diags,
        );
    }
    for node in &manifest.place_nodes {
        validate_params(
            &node.extent_shape_id,
            &node.extent_params_json,
            "place_nodes.extent_params_json",
            diags,
        );
    }
    for row in &manifest.consequence_frame_writes {
        walk_frame_write(
            &row.frame_write,
            "consequence_frame_writes.frame_write",
            diags,
        );
    }
}

fn walk_frame_write(value: &Value, path: &str, diags: &mut Vec<Diagnostic>) {
    let Some(fields) = value.get("ForEachActorInRadius").and_then(Value::as_object) else {
        return;
    };
    if let (Some(shape), Some(params)) = (
        fields.get("shape_id").and_then(Value::as_str),
        fields.get("shape_params_json").and_then(Value::as_str),
    ) {
        validate_params(shape, params, path, diags);
    }
    if let Some(body) = fields.get("body").and_then(Value::as_array) {
        for op in body {
            walk_frame_write(op, path, diags);
        }
    }
}

fn validate_params(shape: &str, source: &str, path: &str, diags: &mut Vec<Diagnostic>) {
    if shape.is_empty() {
        return;
    }
    let parsed: Value = match serde_json::from_str(source) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => return push(path, "area shape params must be a JSON object", diags),
    };
    let (required, overrides): (&str, &[&str]) = match shape {
        "core/area/cube" | "cube" => ("extent_m", &["extent_x_m", "extent_y_m", "extent_z_m"]),
        "core/area/sphere" | "sphere" => ("radius_m", &["radius_x_m", "radius_y_m", "radius_z_m"]),
        _ => return,
    };
    for key in std::iter::once(required).chain(overrides.iter().copied()) {
        let Some(value) = parsed.get(key) else {
            if key == required {
                push(
                    path,
                    &format!("area shape params require numeric '{key}'"),
                    diags,
                );
            }
            continue;
        };
        let Some(number) = value.as_f64() else {
            push(
                path,
                &format!("area shape param '{key}' must be numeric"),
                diags,
            );
            continue;
        };
        if !number.is_finite() || !(number as f32).is_finite() || number <= 0.0 {
            push(
                path,
                &format!("area shape param '{key}' must be finite and greater than zero"),
                diags,
            );
        }
    }
}

fn push(path: &str, message: &str, diags: &mut Vec<Diagnostic>) {
    diags.push(Diagnostic {
        code: MF_VAL_AREA_SHAPE_PARAMS.into(),
        severity: Severity::Fatal,
        message: message.into(),
        path: path.into(),
        span: None,
        hint: None,
        details: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_independent_positive_overrides_and_rejects_bad_dimensions() {
        let mut diags = Vec::new();
        validate_params(
            "cube",
            r#"{"extent_m":1,"extent_x_m":4,"extent_y_m":2}"#,
            "p",
            &mut diags,
        );
        assert!(diags.is_empty());
        validate_params(
            "sphere",
            r#"{"radius_m":1,"radius_z_m":0}"#,
            "p",
            &mut diags,
        );
        assert_eq!(diags[0].code, MF_VAL_AREA_SHAPE_PARAMS);
    }
}
