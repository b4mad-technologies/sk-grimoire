//! Opt-in Job carrier facets. No publication, identity subject or activation.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn validate_dm_artifact_shape_json(input: &str) -> String {
    serde_json::to_string(&spellkave_validate::dm_artifact::validate_artifact_shape_json(input))
        .expect("DM artifact diagnostics serialize")
}

#[wasm_bindgen]
pub fn validate_dm_presentation_document_json(input: &str) -> String {
    serde_json::to_string(
        &spellkave_validate::dm_document_envelope::validate_presentation_document_json(input),
    )
    .expect("DM document diagnostics serialize")
}

#[wasm_bindgen]
pub fn validate_dm_duty_document_json(input: &str) -> String {
    serde_json::to_string(
        &spellkave_validate::dm_document_envelope::validate_duty_document_json(input),
    )
    .expect("DM document diagnostics serialize")
}

#[wasm_bindgen]
pub fn validate_dm_source_shape_json(input: &str) -> String {
    serde_json::to_string(
        &spellkave_validate::dm_source::parse_source_json(input)
            .err()
            .unwrap_or_default(),
    )
    .expect("DM source diagnostics serialize")
}

#[wasm_bindgen]
pub fn validate_dm_source_references_json(input: &str, worlds: &str) -> String {
    serde_json::to_string(
        &spellkave_validate::dm_source::validate_source_references_json(input, worlds),
    )
    .expect("DM reference diagnostics serialize")
}

#[wasm_bindgen]
pub fn derive_dm_document_authority_json(input: &str, worlds: &str) -> Result<String, JsValue> {
    derive_dm_document_authority_json_core(input, worlds).map_err(|error| JsValue::from_str(&error))
}

pub fn derive_dm_document_authority_json_core(input: &str, worlds: &str) -> Result<String, String> {
    let admitted = spellkave_validate::dm_document_authority::admit_read_authority(input, worlds)
        .map_err(|errors| errors.join("; "))?;
    serde_json::to_string(&serde_json::json!({
        "principal_plan": admitted.principal_plan(),
        "manifest": admitted.definition_rows(),
    }))
    .map_err(|error| error.to_string())
}
