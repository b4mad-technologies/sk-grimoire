//! Bounded runtime content/hash facets. These do not establish an observed
//! target, current assignment, read grant, authority or publication activation.
use crate::canonical_document_hash;
use serde_json::Value;

pub(crate) fn valid_hash(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn envelope(input: &str) -> Result<Value, Vec<String>> {
    if input.len() > crate::npc_documents::MAX_DOCUMENT_BYTES {
        return Err(vec![
            "dm-document: exceeds 256 KiB including envelope".into()
        ]);
    }
    let value = crate::dm_document_authority::unique_json(input)?;
    let mut content = value.clone();
    let fields = content
        .as_object_mut()
        .ok_or_else(|| vec!["dm-document: expected object".into()])?;
    fields.remove("content_hash");
    fields.remove("publication_id");
    let expected =
        canonical_document_hash(&content).map_err(|error| vec![format!("dm-document: {error}")])?;
    if value["content_hash"].as_str() != Some(expected.as_str()) {
        return Err(vec!["dm-document: content hash mismatch".into()]);
    }
    if !value["publication_id"]
        .as_str()
        .and_then(|v| v.strip_prefix("source:"))
        .is_some_and(valid_hash)
    {
        return Err(vec!["dm-document: invalid publication identity".into()]);
    }
    Ok(value)
}

/// Exact private presentation content plus consistency envelope. The caller
/// must separately tie the observed metadata target to its authenticated body.
pub fn validate_presentation_document_json(input: &str) -> Vec<String> {
    let mut value = match envelope(input) {
        Ok(value) => value,
        Err(errors) => return errors,
    };
    let fields = value.as_object_mut().expect("envelope checked object");
    fields.remove("content_hash");
    fields.remove("publication_id");
    crate::dm_presentation::validate_presentation_json(&value.to_string())
}

/// Strict runtime duties omit all Job recipes and locators. This facet checks
/// finite policies/domains and the envelope; it grants no dispatch authority
/// and does not substitute for canonical refs or observed publication links.
pub fn validate_duty_document_json(input: &str) -> Vec<String> {
    let mut value = match envelope(input) {
        Ok(value) => value,
        Err(errors) => return errors,
    };
    let document: crate::dm_duties::DutyDocument = match serde_json::from_value(value.clone()) {
        Ok(document) => document,
        Err(error) => return vec![format!("dm-document: invalid duty shape: {error}")],
    };
    if !valid_hash(&document.catalog_content_hash)
        || !valid_hash(&document.presentation_content_hash)
        || !document
            .catalog_publication_id
            .strip_prefix("source:")
            .is_some_and(valid_hash)
        || document.catalog_namespace.trim().is_empty()
        || document.catalog_namespace.len() > crate::dm_duties::MAX_REF_BYTES
    {
        return vec!["dm-document: invalid declared document link".into()];
    }
    let fields = value.as_object_mut().expect("envelope checked object");
    for field in [
        "content_hash",
        "publication_id",
        "catalog_namespace",
        "catalog_content_hash",
        "catalog_publication_id",
        "presentation_content_hash",
    ] {
        fields.remove(field);
    }
    crate::dm_duties::validate_runtime_component(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seal(mut content: Value) -> String {
        content["content_hash"] = serde_json::json!(canonical_document_hash(&content).unwrap());
        content["publication_id"] = serde_json::json!(format!("source:sha256:{}", "0".repeat(64)));
        content.to_string()
    }
    fn duties() -> Value {
        let mut value: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/dm-duty-source-v1.generated.json"
        ))
        .unwrap();
        for kit in value["creation"]["kits"].as_array_mut().unwrap() {
            kit.as_object_mut().unwrap().remove("items");
        }
        value["catalog_namespace"] = serde_json::json!("fixture.catalog");
        for field in ["catalog_content_hash", "presentation_content_hash"] {
            value[field] = serde_json::json!(format!("sha256:{}", "0".repeat(64)));
        }
        value["catalog_publication_id"] =
            serde_json::json!(format!("source:sha256:{}", "1".repeat(64)));
        value
    }
    // K2: runtime documents use the real component with Job recipes removed;
    // even correctly rehashed content must retain finite domains and policies.
    #[test]
    fn runtime_duties_share_source_policies_and_forbid_job_fields() {
        assert!(validate_duty_document_json(&seal(duties())).is_empty());
        for mutation in 0..6 {
            let mut value = duties();
            match mutation {
                0 => value["creation"]["kits"][0]["items"] = serde_json::json!([]),
                1 => value["dialog"]["slot_count"] = serde_json::json!(0),
                2 => {
                    value["quest_prerequisites"][0]["requires_rewarded_quests"] =
                        serde_json::json!([])
                }
                3 => value["creation"]["training"]["skill"]["action_id"] = Value::Null,
                4 => value["watches"]["quest_loot_enabled"] = serde_json::json!(true),
                _ => value["catalog_content_hash"] = serde_json::json!("sha256:wrong"),
            }
            assert!(
                !validate_duty_document_json(&seal(value)).is_empty(),
                "mutation {mutation}"
            );
        }
    }
    // K2: an authorized writer can rehash changed prose. No one of the seven
    // forbidden selectors is admitted at any of the presentation paths.
    #[test]
    fn runtime_envelope_refuses_tampering_duplicates_and_oversize() {
        let valid = seal(duties());
        let mut value: Value = serde_json::from_str(&valid).unwrap();
        value["content_hash"] = serde_json::json!(format!("sha256:{}", "f".repeat(64)));
        assert!(!validate_duty_document_json(&value.to_string()).is_empty());
        let mut value: Value = serde_json::from_str(&valid).unwrap();
        value["publication_id"] = serde_json::json!("arbitrary");
        assert!(!validate_duty_document_json(&value.to_string()).is_empty());
        let duplicate = valid.replacen(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
            1,
        );
        assert_ne!(duplicate, valid);
        assert!(!validate_duty_document_json(&duplicate).is_empty());
        let oversized = format!(
            "{}{}",
            valid,
            " ".repeat(crate::npc_documents::MAX_DOCUMENT_BYTES)
        );
        assert!(validate_duty_document_json(&oversized)[0].contains("256 KiB"));
    }
}
