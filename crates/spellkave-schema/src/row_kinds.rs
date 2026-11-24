// Manifest row-kind -> single-column natural-key field (#1968 R7, ADR-0087
// slice 2). Ported VERBATIM from `@spellkave/world-author`'s
// `row-kinds.ts` — same entries, same key-field strings, same header
// provenance below — into the single Rust source of truth. `row-kinds.ts`
// becomes a thin wrapper reading this table through
// `spellkave-authoring-wasm`'s `row_kind_natural_key_json` export (see that
// crate's `lib.rs`); no consumer of `ROW_KIND_NATURAL_KEY` (shared tool runtime's
// workspace idempotency merge, `world-author`'s `AuthorTarget`-facing local
// driver) observes a difference.
//
// Relocated here from `spellkave-authoring` (`tag-vocabulary-declaration`
// change, Task 3.2, 2026-07-27): `spellkave-validate`'s natural-key
// cross-check for `TagPrefixDeclaration.natural_key_field` needs this
// registry too, and `spellkave-authoring` depends on `spellkave-validate`
// (not the reverse) — this crate is the lowest common dependency both
// already share, so it is the single-source-of-truth home for either
// direction. `spellkave-authoring` re-exports this module unchanged
// (`pub use spellkave_schema::row_kinds;` in its `lib.rs`) so no existing
// `spellkave_authoring::ROW_KIND_NATURAL_KEY`-shaped call site moves.
//
// Junction rows with composite keys are listed in `row_identity` instead.
/// Manifest row-kind -> ordered composite-key fields (finding #9, srd-cc
/// battle-test ledger, 2026-07-26): the twelve uniform `*_tag` satellite
/// kinds, each `{<parent_field>, tag, bundle_id, source}`
/// (`crates/spellkave-schema/src/lib.rs`'s `*TagRow` structs — verified
/// field-by-field, every one of them exactly `{parent_id_field, tag,
/// bundle_id, source}`, never invented). These are ABSENT from
/// `ROW_KIND_NATURAL_KEY` above because a single tag value is not a row's
/// identity (many tag rows share the same parent) — the natural key is the
/// ordered pair `(parent_field, "tag")` used by the row query interface.
///
/// sk-bwd adds ONE non-`*_tag` entry, `tag_capability_grant`
/// (`{tag, capability_id, bundle_id}` — verified field-by-field like the
/// twelve above). It is the same uniform two-discriminant shape, with the
/// tag as the PARENT rather than the discriminant, so its ordered pair is
/// `("tag", "capability_id")` — matching the `"{tag}:{capability_id}"`
/// retraction key.
///
/// `condition_capability_extent_grants` uses the ordered four-reference key
/// `(condition_definition_id, capability_id, extent_kind_ref, extent_ref)`.
/// These fields match `ConditionCapabilityExtentGrantRow` and the server's
/// authored satellite identity; bundle ownership is separate from identity.
///
/// Other satellite/junction kinds named in the header note above
/// (`entity_definition_initial_*`, `spatial_niche_attachments`,
/// `actor_capabilities`, `consequence_frame_writes`, `object_state`) are
/// NOT uniform `{parent, discriminant}` pairs and are deliberately left out
/// of this table — each would need its own field-by-field verification
/// against its own manifest row struct before being added here.
pub const ROW_KIND_COMPOSITE_KEY: &[(&str, &[&str])] = &[
    ("entity_definition_tag", &["entity_def_id", "tag"]),
    ("object_state_definition_tag", &["state_id", "tag"]),
    ("capability_definition_tag", &["capability_id", "tag"]),
    ("niche_definition_tag", &["niche_id", "tag"]),
    ("outcome_template_tag", &["outcome_template_id", "tag"]),
    ("action_definition_tag", &["action_id", "tag"]),
    (
        "resource_definition_tag",
        &["resource_definition_id", "tag"],
    ),
    ("roll_table_tag", &["roll_table_id", "tag"]),
    ("roll_table_entry_tag", &["roll_table_entry_id", "tag"]),
    ("modifier_template_tag", &["modifier_template_id", "tag"]),
    (
        "condition_definition_tag",
        &["condition_definition_id", "tag"],
    ),
    ("entity_tag", &["entity_key", "tag"]),
    // sk-bwd: the IDENTITY->capability bridge. NOT a `*_tag` satellite (its
    // parent IS the tag), but the same uniform `{discriminant_a,
    // discriminant_b}` shape — `TagCapabilityGrantRow` is exactly
    // `{tag, capability_id, bundle_id}`, verified field-by-field against
    // `crates/spellkave-schema/src/lib.rs`, so a caller can enumerate and
    // upsert it by key generically like every kind above.
    ("tag_capability_grant", &["tag", "capability_id"]),
    // Same four-ref identity as ConditionCapabilityExtentGrantRow and the server satellite.
    (
        "condition_capability_extent_grants",
        &[
            "condition_definition_id",
            "capability_id",
            "extent_kind_ref",
            "extent_ref",
        ],
    ),
];

/// `ROW_KIND_COMPOSITE_KEY` as a JSON object string (`{"<row_kind>":
/// ["<field>", ...], ...}`) — the wasm-exported counterpart of
/// `row_kind_natural_key_json`, same shape discipline (JSON round-trips,
/// this crate is the single source of truth).
pub fn row_kind_composite_key_json() -> String {
    let mut map = serde_json::Map::with_capacity(ROW_KIND_COMPOSITE_KEY.len());
    for (kind, fields) in ROW_KIND_COMPOSITE_KEY {
        map.insert(
            (*kind).to_owned(),
            serde_json::Value::Array(
                fields
                    .iter()
                    .map(|field| serde_json::Value::String((*field).to_owned()))
                    .collect(),
            ),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_else(|_| "{}".to_owned())
}

pub const ROW_KIND_NATURAL_KEY: &[(&str, &str)] = &[
    ("containment_policy_definition", "policy_id"),
    ("entity_definition_containment_policy", "entity_def_id"),
    ("entity_containment_policy", "entity_key"),
    ("bundles", "bundle_id"),
    ("tag_value_definitions", "tag"),
    ("coordinate_kind_definitions", "coordinate_kind_id"),
    ("substrate_definitions", "substrate_id"),
    ("area_shape_definitions", "shape_id"),
    ("location_definitions", "location_def_id"),
    ("spatial_state_definitions", "state_id"),
    ("entity_definitions", "entity_def_id"),
    ("entities", "entity_key"),
    ("actors", "entity_key"),
    ("object_state_definitions", "state_id"),
    ("capability_definitions", "capability_id"),
    ("role_definitions", "role_id"),
    ("role_bindings", "binding_id"),
    ("extent_kind_definitions", "kind_id"),
    ("distance_metric_definitions", "metric_id"),
    ("niche_definitions", "niche_id"),
    ("outcome_templates", "outcome_template_id"),
    ("outcome_graphs", "outcome_graph_id"),
    ("outcome_graph_nodes", "outcome_graph_node_id"),
    ("outcome_graph_edges", "outcome_graph_edge_id"),
    ("economy_channel_definitions", "id"),
    ("turn_scheduler", "id"),
    ("session_definitions", "session_def_id"),
    ("combat_grid_blueprints", "blueprint_id"),
    ("effect_definitions", "effect_def_id"),
    ("consequence_templates", "template_id"),
    ("action_definitions", "action_id"),
    ("resource_definitions", "resource_definition_id"),
    ("autonomy_jobs", "job_id"),
    ("trace_kind_definitions", "kind_id"),
    ("dsl_macro_definitions", "macro_id"),
    ("roll_tables", "roll_table_id"),
    ("roll_table_entries", "roll_table_entry_id"),
    ("modifier_templates", "modifier_template_id"),
    ("condition_definitions", "condition_definition_id"),
    ("place_graphs", "place_graph_id"),
    ("place_seals", "place_graph_id"),
    ("place_nodes", "node_id"),
    ("place_edges", "edge_key"),
    ("affordance_templates", "template_id"),
    // ADR-0142 D-1: each carries an independent authored id, verified
    // field-by-field against `crates/spellkave-schema/src/lib.rs`'s
    // `PromptDefinitionRow`/`AudienceKindDefinitionRow`.
    ("prompt_definitions", "prompt_def_id"),
    ("audience_kind_definitions", "audience_kind_id"),
    ("equipment_slot_definitions", "slot_id"),
    ("modifier_target_definitions", "target_id"),
    ("stacking_discipline_definitions", "discipline_id"),
    ("condition_remove_policy_definitions", "policy_id"),
    ("map_layers", "map_layer_id"),
    ("anchor_kind_definitions", "kind_id"),
    ("boundary_definitions", "id"),
    ("item_definitions", "item_def_id"),
];

/// `ROW_KIND_NATURAL_KEY` as a JSON object string (`{"<row_kind>":
/// "<field>", ...}`), the exact shape `row-kinds.ts`'s
/// `Readonly<Record<string, string>>` expects. Lives in this crate (not the
/// wasm crate) so a native Rust caller gets the same serialization a wasm
/// caller does, with no duplicate formatting logic.
pub fn row_kind_natural_key_json() -> String {
    let mut map = serde_json::Map::with_capacity(ROW_KIND_NATURAL_KEY.len());
    for (kind, field) in ROW_KIND_NATURAL_KEY {
        map.insert(
            (*kind).to_owned(),
            serde_json::Value::String((*field).to_owned()),
        );
    }
    serde_json::to_string(&serde_json::Value::Object(map)).unwrap_or_else(|_| "{}".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_row_kind_is_unique() {
        let mut seen = BTreeSet::new();
        for (kind, _) in ROW_KIND_NATURAL_KEY {
            assert!(
                seen.insert(*kind),
                "duplicate row kind {kind} in ROW_KIND_NATURAL_KEY"
            );
        }
    }

    #[test]
    fn json_round_trips_every_entry() {
        let json = row_kind_natural_key_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON object");
        let obj = value.as_object().expect("top-level object");
        assert_eq!(obj.len(), ROW_KIND_NATURAL_KEY.len());
        for (kind, field) in ROW_KIND_NATURAL_KEY {
            assert_eq!(
                obj.get(*kind).and_then(|v| v.as_str()),
                Some(*field),
                "row kind {kind} should map to natural key field {field}"
            );
        }
    }

    #[test]
    fn every_composite_row_kind_is_unique() {
        let mut seen = BTreeSet::new();
        for (kind, _) in ROW_KIND_COMPOSITE_KEY {
            assert!(
                seen.insert(*kind),
                "duplicate row kind {kind} in ROW_KIND_COMPOSITE_KEY"
            );
        }
    }

    #[test]
    fn no_row_kind_has_both_a_natural_and_a_composite_key() {
        for (kind, _) in ROW_KIND_COMPOSITE_KEY {
            assert!(
                !ROW_KIND_NATURAL_KEY.iter().any(|(nk, _)| nk == kind),
                "{kind} appears in both ROW_KIND_NATURAL_KEY and ROW_KIND_COMPOSITE_KEY"
            );
        }
    }

    #[test]
    fn composite_json_round_trips_every_entry() {
        let json = row_kind_composite_key_json();
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON object");
        let obj = value.as_object().expect("top-level object");
        assert_eq!(obj.len(), ROW_KIND_COMPOSITE_KEY.len());
        for (kind, fields) in ROW_KIND_COMPOSITE_KEY {
            let got = obj
                .get(*kind)
                .and_then(|v| v.as_array())
                .expect("composite key value should be an array");
            let got_fields: Vec<&str> = got.iter().filter_map(|v| v.as_str()).collect();
            assert_eq!(&got_fields, fields, "row kind {kind} composite key fields");
        }
    }

    // K4: fence the exact authored identity shapes exported to generic consumers.
    #[test]
    fn composite_keys_match_declared_row_shapes() {
        // Every `*_tag` satellite is `(parent_field, "tag")`. sk-bwd's
        // `tag_capability_grant` is the ONE entry where the tag is the
        // parent rather than the discriminant, so it reads
        // `("tag", "capability_id")` — the field-count and
        // one-of-them-is-`tag` uniformity this table's doc comment claims
        // remains strict for the existing tag rows. The condition extent
        // satellite has its separately verified four-reference identity;
        // assert its complete field list rather than accepting any length.
        for (kind, fields) in ROW_KIND_COMPOSITE_KEY {
            if *kind == "condition_capability_extent_grants" {
                assert_eq!(
                    *fields,
                    [
                        "condition_definition_id",
                        "capability_id",
                        "extent_kind_ref",
                        "extent_ref",
                    ],
                    "condition extent grants use the exact ordered four-reference identity"
                );
                continue;
            }
            assert_eq!(fields.len(), 2, "{kind} composite key should have 2 fields");
            assert!(
                fields.contains(&"tag"),
                "{kind} composite key should include \"tag\""
            );
            if *kind != "tag_capability_grant" {
                assert_eq!(
                    fields[1], "tag",
                    "{kind} composite key's 2nd field should be \"tag\""
                );
            }
        }
    }
}
