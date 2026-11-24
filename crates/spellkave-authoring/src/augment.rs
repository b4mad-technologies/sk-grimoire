//! Natural-key merge-by-target — the "HOW" half of `authoring-verbs.ts`'s
//! `augmentForValidation` (#1968 R7 slice 5.2, ADR-0087).
//!
//! `augmentForValidation` itself STAYS in TypeScript: WHICH row kinds to
//! augment (`VALIDATION_AUGMENT_KINDS`/`PLACE_AUGMENT_KINDS_LOCAL_ONLY`),
//! WHEN (`target.kind === "local"` gating), and same-bundle filtering for the
//! place kinds are policy decisions about the calling context this crate has
//! no access to — `target.lookup(kind)` is I/O (a `local` target reads
//! sibling TOML off disk, a `live` target queries the connected world; this
//! crate does neither, per the crate-root doc comment's "no fs access").
//!
//! What DOES belong here is manifest semantics: given a row kind's
//! ALREADY-authored (workspace) rows and a candidate set already resolved
//! from the target (and, for the place kinds, already same-bundle-filtered
//! by the TS caller), which candidate rows are genuinely NEW by natural key.
//! Reuses `ROW_KIND_NATURAL_KEY` (section 2) for the key field — the SAME
//! table `augmentForValidation` read directly before this slice.
//!
//! `dropTargetDeferredDiagnostics` (the sibling function `augmentForValidation`'s
//! own doc comment cross-references) has NO natural-key merge logic at all —
//! it filters diagnostics by code/path, pure WHEN-policy — so it is not
//! ported here; it stays TypeScript-only.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::patch::support::PatchResult;
use crate::row_kinds::ROW_KIND_NATURAL_KEY;

fn natural_key_field(kind: &str) -> PatchResult<&'static str> {
    ROW_KIND_NATURAL_KEY
        .iter()
        .find(|(row_kind, _)| *row_kind == kind)
        .map(|(_, field)| *field)
        .ok_or_else(|| format!("augment: unknown row kind for natural-key lookup: {kind}"))
}

fn rows_array(json: &str, label: &str) -> PatchResult<Vec<Value>> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("augment {label} JSON parse failed: {err}"))?;
    match value {
        Value::Array(rows) => Ok(rows),
        other => Err(format!("augment {label} must be a JSON array, got {other}")),
    }
}

/// The candidate (target-resolved) rows for ONE row kind whose natural key
/// (`ROW_KIND_NATURAL_KEY[kind]`) is NOT already present among `existing`
/// (the workspace's own rows) — the exact "extra" set `augmentForValidation`'s
/// per-kind loop appends. Workspace rows always win: a same-key candidate is
/// dropped, never merged field-by-field, mirroring `augmentForValidation`'s
/// own doc comment ("a workspace row ALWAYS wins over a same-key target
/// row — re-authoring a row the target already has is intentional shadowing,
/// not a collision"). `existing_json`/`candidate_json` are each a JSON array
/// of row objects; the return value is the filtered candidate array as JSON.
pub fn extra_rows_by_natural_key_json(
    kind: &str,
    existing_json: &str,
    candidate_json: &str,
) -> PatchResult<String> {
    let key_field = natural_key_field(kind)?;
    let existing = rows_array(existing_json, "existing rows")?;
    let candidates = rows_array(candidate_json, "candidate rows")?;

    let existing_keys: BTreeSet<&str> = existing
        .iter()
        .filter_map(|row| row.get(key_field).and_then(Value::as_str))
        .collect();

    let extra: Vec<Value> = candidates
        .into_iter()
        .filter(|row| match row.get(key_field).and_then(Value::as_str) {
            Some(key) => !existing_keys.contains(key),
            None => true,
        })
        .collect();

    serde_json::to_string(&Value::Array(extra))
        .map_err(|err| format!("augment extra-rows JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn drops_a_candidate_whose_key_already_exists_in_workspace() {
        let existing = json!([{ "resource_definition_id": "srd/hp" }]).to_string();
        let candidates = json!([
            { "resource_definition_id": "srd/hp", "display_name": "SHADOWED" },
            { "resource_definition_id": "srd/mp", "display_name": "New" },
        ])
        .to_string();
        let out = extra_rows_by_natural_key_json("resource_definitions", &existing, &candidates)
            .expect("known row kind should succeed");
        let extra: Value = serde_json::from_str(&out).expect("valid JSON array");
        let rows = extra.as_array().expect("array");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["resource_definition_id"], json!("srd/mp"));
    }

    #[test]
    fn empty_candidates_yield_empty_extra() {
        let out = extra_rows_by_natural_key_json("entity_definitions", "[]", "[]")
            .expect("empty inputs should succeed");
        assert_eq!(out, "[]");
    }

    #[test]
    fn unknown_row_kind_errors() {
        let err = extra_rows_by_natural_key_json("not_a_real_kind", "[]", "[]")
            .expect_err("unknown kind should error");
        assert!(err.contains("not_a_real_kind"));
    }

    #[test]
    fn every_place_augment_kind_is_a_known_natural_key() {
        // Mirrors the TS side's own guard (`biome-ignore lint/style/noNonNullAssertion`
        // comment in `authoring-verbs.ts`'s `augmentForValidation`): every kind
        // TS ever calls this export with must resolve here, never fall through
        // to the "unknown row kind" error path.
        for kind in [
            "resource_definitions",
            "entity_definitions",
            "object_state_definitions",
            "capability_definitions",
            "place_graphs",
            "place_nodes",
        ] {
            assert!(
                natural_key_field(kind).is_ok(),
                "{kind} should have a known natural-key field"
            );
        }
    }
}
