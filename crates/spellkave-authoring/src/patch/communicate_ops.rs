//! Slice 3.6 (#1968 R7, ADR-0087 task 3.6): the one remaining patch op,
//! `add-communicate-affordance`. Ported verbatim from `patch.ts`: append-only
//! (no upsert at all, unlike every other op in section 3) into
//! `outcome_templates`.

use serde_json::Value;

use super::support::{
    patched_tables, resolved_bundle_id, row, row_array, s, string_field, PatchResult,
};

/// Pushes a communication-speech-act outcome template. `outcome_bool` is
/// always `true` (a communicate affordance's outcome is a boolean speech
/// act, never a rolled/graded one); `consequence_template_id` defaults to
/// `""` when omitted.
pub fn add_communicate_affordance(manifest: &Value, payload: &Value) -> PatchResult<Value> {
    let bundle_id = resolved_bundle_id(manifest, payload);
    let outcome_template_id = string_field(payload, "outcome_template_id")?;
    let display_name = string_field(payload, "display_name")?;
    let speech_act_id = string_field(payload, "speech_act_id")?;
    let consequence_template_id = payload
        .get("consequence_template_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();

    let mut rows = row_array(manifest, "outcome_templates");
    rows.push(row([
        ("outcome_template_id", s(outcome_template_id)),
        ("bundle_id", s(bundle_id)),
        ("display_name", s(display_name)),
        ("outcome_bool", Value::Bool(true)),
        ("communication_speech_act_id", s(speech_act_id)),
        ("consequence_template_id", s(consequence_template_id)),
    ]));
    Ok(patched_tables([("outcome_templates", rows)]))
}
