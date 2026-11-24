//! ADR-0101 D-1/D-7: the movement-default op — `[[movement_defaults]]`
//! (`crates/spellkave-schema/src/lib.rs` `MovementDefaultRow`), a NEW table
//! declaring the default traversal mode set for a bundle (`place_graph_id`
//! empty) or one place-graph. Unlike the no-`bundle_id`-field group, this
//! row DOES carry `bundle_id` — resolved the same way `place_graph_ops`'s
//! ops resolve it (`resolved_bundle_id`).
//!
//! Upsert by the composite (bundle_id, place_graph_id, mode_id,
//! affordance_template_id) — re-authoring the SAME scope+mode+template
//! triple replaces its `bindings_json` in place; a second template on the
//! same scope+mode is a sibling row (AND-join within the mode, the same
//! template-sibling rule `add_place_edge_affordance` documents), and a
//! second `mode_id` under the same scope is a third alternative (OR across
//! modes, D-2). `place_graph_id` and `mode_id` are ALWAYS written explicitly
//! (never omitted, even when empty) — unlike `place_edge_affordance.mode_id`
//! there is no pre-ADR-0101 legacy row shape to stay byte-identical with
//! (this table is brand new), so there is no reason to special-case the
//! empty-string case; every row this op ever authors carries all four key
//! columns, mirroring `add_place_edge`'s own always-present
//! `place_graph_id` field.
//!
//! Row/scope existence (does the named `affordance_template_id` resolve? is
//! `place_graph_id` a real graph in this bundle?) is deliberately NOT
//! checked here — cross-row resolution is the validator's job (Class 10b's
//! `validate_movement_default_binding_coverage` /
//! `validate_movement_mode_composition`, `crates/spellkave-validate/src/refs.rs`).

use serde_json::Value;

use super::support::{
    optional_string_field, patched_tables, remove_rows_by_fields, resolved_bundle_id, row,
    row_array, s, string_field, string_field_or, PatchResult,
};

pub fn add_movement_default(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let place_graph_id = string_field_or(payload, "place_graph_id", "");
    let mode_id = string_field_or(payload, "mode_id", "");
    let affordance_template_id = string_field(payload, "affordance_template_id")?;
    let bindings_json = optional_string_field(payload, "bindings_json");

    let mut rows = remove_rows_by_fields(
        row_array(manifest, "movement_defaults"),
        &[
            ("bundle_id", bundle_id.as_str()),
            ("place_graph_id", place_graph_id.as_str()),
            ("mode_id", mode_id.as_str()),
            ("affordance_template_id", affordance_template_id.as_str()),
        ],
    );
    let mut fields = vec![
        ("bundle_id", s(bundle_id)),
        ("place_graph_id", s(place_graph_id)),
        ("mode_id", s(mode_id)),
        ("affordance_template_id", s(affordance_template_id)),
    ];
    if let Some(bindings_json) = bindings_json {
        fields.push(("bindings_json", s(bindings_json)));
    }
    rows.push(row(fields));
    Ok(patched_tables([("movement_defaults", rows)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest_with_bundle(bundle_id: &str) -> Value {
        json!({
            "schema_version": "1.0.0",
            "bundles": [{ "bundle_id": bundle_id }],
        })
    }

    #[test]
    fn defaults_place_graph_id_and_mode_id_to_empty_bundle_scope() {
        let manifest = manifest_with_bundle("srd");
        let out = add_movement_default(
            &manifest,
            &json!({ "affordance_template_id": "srd/affordance/step" }),
        )
        .expect("op should succeed");
        let rows = out["movement_defaults"].as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["bundle_id"], json!("srd"));
        assert_eq!(rows[0]["place_graph_id"], json!(""));
        assert_eq!(rows[0]["mode_id"], json!(""));
        assert_eq!(
            rows[0]["affordance_template_id"],
            json!("srd/affordance/step")
        );
        assert!(rows[0].get("bindings_json").is_none());
    }

    #[test]
    fn upserts_by_scope_mode_template_and_adds_siblings() {
        let manifest = manifest_with_bundle("srd");
        let out = add_movement_default(
            &manifest,
            &json!({
                "affordance_template_id": "srd/affordance/step",
                "bindings_json": "{\"movement_reach_resource\":\"srd/speed-walk\"}",
            }),
        )
        .expect("op should succeed");

        // Re-authoring the SAME scope+mode+template replaces bindings_json.
        let manifest2 = json!({
            "schema_version": "1.0.0",
            "bundles": [{ "bundle_id": "srd" }],
            "movement_defaults": out["movement_defaults"].clone(),
        });
        let out2 = add_movement_default(
            &manifest2,
            &json!({
                "affordance_template_id": "srd/affordance/step",
                "bindings_json": "{\"movement_reach_resource\":\"srd/speed-walk-2\"}",
            }),
        )
        .expect("op should succeed");
        let rows2 = out2["movement_defaults"].as_array().expect("array");
        assert_eq!(rows2.len(), 1);
        assert_eq!(
            rows2[0]["bindings_json"],
            json!("{\"movement_reach_resource\":\"srd/speed-walk-2\"}")
        );

        // A distinct mode_id under the same scope is a sibling row (OR).
        let manifest3 = json!({
            "schema_version": "1.0.0",
            "bundles": [{ "bundle_id": "srd" }],
            "movement_defaults": out2["movement_defaults"].clone(),
        });
        let out3 = add_movement_default(
            &manifest3,
            &json!({
                "mode_id": "fly",
                "affordance_template_id": "srd/affordance/fly",
                "bindings_json": "{}",
            }),
        )
        .expect("op should succeed");
        let rows3 = out3["movement_defaults"].as_array().expect("array");
        assert_eq!(rows3.len(), 2);
    }

    #[test]
    fn requires_affordance_template_id() {
        let manifest = manifest_with_bundle("srd");
        let err = add_movement_default(&manifest, &json!({}))
            .expect_err("missing affordance_template_id should error");
        assert!(err.contains("affordance_template_id"));
    }
}
