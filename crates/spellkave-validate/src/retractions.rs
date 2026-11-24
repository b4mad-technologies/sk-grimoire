//! Authored deletion-tombstone validation (ADR-0085 D-6, `[[retractions]]`).
//!
//! `r_apply_manifest_delta` (server) is upsert-only: an authored key simply
//! absent from a delta says nothing about deletion intent. A delta MAY carry
//! `[[retractions]]` tuples of `(row_kind, authored_key)` that the server
//! resolves against (live world ∪ delta) and, on commit, deletes the full row
//! closure for.
//!
//! This module validates only the two things a single manifest can see without
//! live-world context: `authored_key` is non-empty, and `row_kind` is one this
//! ADR's implementation actually executes. It does **not** attempt the "does
//! this key exist, and is it still referenced elsewhere" check — that closure
//! can only be resolved against live world state (ADR-0085, ADR-0044).

use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;

/// `retractions[].authored_key` is empty.
pub const MF_VAL_RETRACTION_KEY_EMPTY: &str = "MF-VAL-292";
/// `retractions[].row_kind` is not one of [`SUPPORTED_RETRACTION_ROW_KINDS`].
/// ADR-0085 G-1: the per-row-kind cascade table is enumeration work, not fixed
/// by the ADR; only kinds the server can actually execute are accepted here so
/// a delta never stages a tombstone that silently no-ops at commit.
pub const MF_VAL_RETRACTION_ROW_KIND: &str = "MF-VAL-293";

/// Row kinds that support retraction in a manifest. Retracting an entity
/// definition satellite uses a colon-joined authored key such as
/// `"{entity_def_id}:{secondary_field}"`. A frame-write retraction names the
/// template and clears its ordered writes.
pub const SUPPORTED_RETRACTION_ROW_KINDS: &[&str] = &[
    "containment_policy_definition",
    "entity_definition_containment_policy",
    "entity_containment_policy",
    "place_seal",
    "place_seals",
    "role_binding",
    "role_bindings",
    "actor_capability",
    "actor_capabilities",
    "role_definition",
    "role_aggregation",
    "entity",
    "outcome_graph",
    "outcome_graph_node",
    "outcome_graph_edge",
    "consequence_template",
    "roll_table",
    "roll_table_entry",
    "entity_definition",
    "entity_definition_initial_state",
    "entity_definition_initial_capability",
    "entity_definition_initial_condition",
    // sk-84i.3 (parent sk-84i): two more composite-keyed
    // `entity_definition_initial_*` satellites, same shape as the trio
    // above — `authored_key` is `"{entity_def_id}:{contained_entity_def_id}"`
    // / `"{entity_def_id}:{slot_id}"` respectively.
    "entity_definition_initial_containment",
    "entity_definition_initial_equipment",
    "consequence_template_frame_write",
    // sk-84i.2: thirteenth kind — composite-keyed
    // (`"{entity_def_id}:{slot}"`) like the three `entity_definition_
    // initial_*` satellites above; the item DSL-source slot table
    // `<subject>.source(...)` (widened G-1) reads.
    "entity_definition_item_dsl_source",
    // sk-bwd: the IDENTITY→capability bridge. Composite-keyed
    // `"{tag}:{capability_id}"` uses a tag as the anchor.
    "tag_capability_grant",
    "condition_capability_extent_grants",
    // Composite-keyed `"{entity_def_id}:{modifier_template_id}"`, same shape
    // as the satellites above.
    "entity_definition_equip_modifier",
];

pub fn validate_retractions(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for (i, row) in manifest.retractions.iter().enumerate() {
        if row.authored_key.trim().is_empty() {
            diags.push(fatal(
                MF_VAL_RETRACTION_KEY_EMPTY,
                &format!("retractions[{i}].authored_key"),
                "retractions[].authored_key must not be empty".to_owned(),
            ));
        }
        if !SUPPORTED_RETRACTION_ROW_KINDS.contains(&row.row_kind.as_str()) {
            diags.push(fatal(
                MF_VAL_RETRACTION_ROW_KIND,
                &format!("retractions[{i}].row_kind"),
                format!(
                    "retractions[].row_kind '{}' is not a supported retraction kind. \
                     Supported: [{}] (ADR-0085 G-1 — the remaining row-kind cascades are \
                     unimplemented enumeration work, not a validator gap).",
                    row.row_kind,
                    SUPPORTED_RETRACTION_ROW_KINDS.join(", "),
                ),
            ));
        }
    }
}

fn fatal(code: &str, path: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        message,
        path: path.to_owned(),
        span: None,
        hint: None,
        details: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::RetractionRow;

    fn manifest_with(rows: Vec<RetractionRow>) -> Manifest {
        Manifest {
            retractions: rows,
            ..Manifest::default()
        }
    }

    fn codes(diags: &[Diagnostic]) -> Vec<String> {
        diags.iter().map(|d| d.code.clone()).collect()
    }

    #[test]
    fn empty_authored_key_is_fatal() {
        let manifest = manifest_with(vec![RetractionRow {
            row_kind: "entity".into(),
            authored_key: "".into(),
        }]);
        let mut diags = Vec::new();
        validate_retractions(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_RETRACTION_KEY_EMPTY.to_owned()));
    }

    #[test]
    fn unsupported_row_kind_is_fatal() {
        let manifest = manifest_with(vec![RetractionRow {
            row_kind: "affordance_template".into(),
            authored_key: "testpack/foo".into(),
        }]);
        let mut diags = Vec::new();
        validate_retractions(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_RETRACTION_ROW_KIND.to_owned()));
    }

    #[test]
    fn entity_retraction_with_a_key_is_accepted() {
        let manifest = manifest_with(vec![RetractionRow {
            row_kind: "entity".into(),
            authored_key: "testpack:testpack/guard_1".into(),
        }]);
        let mut diags = Vec::new();
        validate_retractions(&manifest, &mut diags);
        assert!(diags.is_empty(), "valid entity retraction: {diags:?}");
    }

    /// #1968 B-9 (g1-satellite-cascade-upsert D-5): one acceptance test per
    /// newly supported kind, mirroring `entity_retraction_with_a_key_is_
    /// accepted` above.
    #[test]
    fn each_new_g1_cascade_kind_with_a_key_is_accepted() {
        for kind in [
            "outcome_graph",
            "outcome_graph_node",
            "outcome_graph_edge",
            "consequence_template",
            "roll_table",
            "roll_table_entry",
            "entity_definition",
        ] {
            let manifest = manifest_with(vec![RetractionRow {
                row_kind: kind.into(),
                authored_key: "testpack/some_key".into(),
            }]);
            let mut diags = Vec::new();
            validate_retractions(&manifest, &mut diags);
            assert!(diags.is_empty(), "valid '{kind}' retraction: {diags:?}");
        }
    }

    /// Tranche B (task §3): the three composite-keyed
    /// `entity_definition_initial_*` kinds, whose `authored_key` is the
    /// The authored key is an `"{entity_def_id}:{secondary_field}"` pair.
    #[test]
    fn each_new_entity_definition_initial_kind_with_a_composite_key_is_accepted() {
        for kind in [
            "entity_definition_initial_state",
            "entity_definition_initial_capability",
            "entity_definition_initial_condition",
            "entity_definition_initial_containment",
            "entity_definition_initial_equipment",
        ] {
            let manifest = manifest_with(vec![RetractionRow {
                row_kind: kind.into(),
                authored_key: "testpack/some_def:testpack/some_secondary".into(),
            }]);
            let mut diags = Vec::new();
            validate_retractions(&manifest, &mut diags);
            assert!(diags.is_empty(), "valid '{kind}' retraction: {diags:?}");
        }
    }

    /// Tranche C (task §5): the twelfth and final kind,
    /// `consequence_template_frame_write` — its `authored_key` names the
    /// PARENT `template_id` (Class β has no per-row identity), the
    /// template-scoped clear-to-zero tombstone.
    #[test]
    fn consequence_template_frame_write_retraction_with_a_template_id_is_accepted() {
        let manifest = manifest_with(vec![RetractionRow {
            row_kind: "consequence_template_frame_write".into(),
            authored_key: "testpack/some_template".into(),
        }]);
        let mut diags = Vec::new();
        validate_retractions(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "valid 'consequence_template_frame_write' retraction: {diags:?}"
        );
    }
}
