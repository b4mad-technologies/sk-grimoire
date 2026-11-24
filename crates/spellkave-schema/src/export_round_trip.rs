//! The manifest fields `r_export_manifest` does not round-trip (ADR-0086
//! asymmetry classes), in one place (sk-jay0.7.174.15).
//!
//! Readers: `spellkave-authoring`'s `diff_apply` masks them before `sk diff -k`
//! reports drift. Consumers can ignore these fields by default through
//! `export_not_round_tripped_json`.

use serde_json::Value;

/// Row kind -> fields a live export can differ in from the authored row
/// without any drift:
///
/// - VALIDATE-ONLY fields the export fills with schema defaults
///   (`predicate_bool`, `accepts_global_storm`, `stratification_ok`,
///   `outcome_bool`);
/// - ENRICHED-AT-APPLY fields it exports as the live computed value
///   (`locality_class`, `reads_occupancy`, `spends_channel`, `bindings_json`,
///   resolved `visibility`);
/// - snapshot fields (`elevation_class`, `expires_at_tick`, runtime
///   `place_node_id`, `object_state.bundle_id`).
///
/// `"*"` applies to every kind.
pub const EXPORT_NOT_ROUND_TRIPPED: &[(&str, &[&str])] = &[
    ("*", &["elevation_class", "visibility", "expires_at_tick"]),
    (
        "affordance_templates",
        &[
            "predicate_bool",
            "accepts_global_storm",
            "stratification_ok",
            "locality_class",
            "reads_occupancy",
            "spends_channel",
        ],
    ),
    ("outcome_templates", &["outcome_bool"]),
    // The exporter supplies defaults for these authored graph links.
    (
        "place_graphs",
        &[
            "blueprint_id",
            "parent_graph_id",
            "anchor_node_id",
            "is_ephemeral_instance",
        ],
    ),
    ("place_edge_affordance", &["bindings_json"]),
    ("entities", &["place_node_id"]),
    ("actor_spatial_locations", &["place_node_id"]),
    ("object_state", &["bundle_id"]),
];

pub const EXPORT_COVERAGE_GAP: &[&str] = &[
    // reconstruct() leaves these manifest arrays at their defaults.
    "map_layers",
    "movement_defaults",
    "metadata_namespaces",
    "metadata_attachments",
    "substrate_definitions",
    "coordinate_kind_definitions",
    "anchor_kind_definitions",
    "extent_kind_definitions",
    // ADR-0134: same not-yet-reconstructed posture as its vocabulary siblings.
    "distance_metric_definitions",
    // ADR-0142: same not-yet-reconstructed posture as the two vocabulary
    // siblings above.
    "audience_kind_definitions",
    "prompt_definitions",
    "boundary_definitions",
    "location_definitions",
    "place_node_traces",
    "location_traces",
    "entity_definition_tag",
    "entity_definition_initial_state",
    "entity_definition_initial_capability",
    "entity_definition_initial_condition",
    "entity_definition_equip_modifier",
    // sk-84i.2: same not-yet-reconstructed posture as the three siblings above.
    "entity_definition_item_dsl_source",
    // sk-84i.3 (canon-warden finding A-7, PR #2154): the two def-level
    // containment/equipment seed tables — same gap class as their three
    // `entity_definition_initial_*` siblings above.
    "entity_definition_initial_containment",
    "entity_definition_initial_equipment",
    "object_state_definitions",
    "object_state_definition_tag",
    "capability_definition_tag",
    "niche_definition_tag",
    "outcome_template_tag",
    "outcome_graphs",
    "outcome_graph_nodes",
    "outcome_graph_edges",
    "effect_definitions",
    "consequence_templates",
    "consequence_frame_writes",
    "action_definitions",
    "action_definition_tag",
    "resource_definitions",
    "resource_definition_tag",
    "roll_tables",
    "roll_table_tag",
    "roll_table_entries",
    "roll_table_entry_tag",
    "modifier_target_definitions",
    "stacking_discipline_definitions",
    "condition_remove_policy_definitions",
    "modifier_templates",
    "modifier_template_tag",
    "condition_definitions",
    "condition_definition_tag",
    "target_conditions",
    // sk-0m4.42: `entity_tag`, `entity_containment`, `object_state`, and
    // `actor_equipment_slots` now reconstruct (see `reconstruct`). Live
    // equipment rows live on `entity_equipment_slot` — do not grep the
    // server table for the manifest's plural field name.
    "entity_resource_bound",
    "actor_identities",
    "equipment_slot_definitions",
    "item_definitions",
    "spatial_niche_attachments",
    "spatial_state_definitions",
    "spatial_environment_state",
    "area_shape_definitions",
    "economy_channel_definitions",
    "turn_scheduler",
    "session_definitions",
    "combat_grid_blueprints",
    "autonomy_jobs",
    "trace_kind_definitions",
    "retractions",
];

/// Whether `field` of a `kind` row is in [`EXPORT_NOT_ROUND_TRIPPED`].
pub fn export_not_round_tripped(kind: &str, field: &str) -> bool {
    EXPORT_NOT_ROUND_TRIPPED
        .iter()
        .any(|(k, fields)| (*k == "*" || *k == kind) && fields.contains(&field))
}

/// [`EXPORT_NOT_ROUND_TRIPPED`] as a JSON object string,
/// `{"<kind or *>": ["<field>", ...], ...}`, for the wasm export.
pub fn export_not_round_tripped_json() -> String {
    let map: serde_json::Map<String, Value> = EXPORT_NOT_ROUND_TRIPPED
        .iter()
        .map(|(kind, fields)| {
            (
                (*kind).to_owned(),
                Value::Array(
                    fields
                        .iter()
                        .map(|f| Value::String((*f).to_owned()))
                        .collect(),
                ),
            )
        })
        .collect();
    Value::Object(map).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_fields_apply_to_every_kind() {
        assert!(export_not_round_tripped(
            "capability_definitions",
            "visibility"
        ));
        assert!(export_not_round_tripped(
            "affordance_templates",
            "locality_class"
        ));
        assert!(!export_not_round_tripped(
            "outcome_templates",
            "locality_class"
        ));
    }

    #[test]
    fn every_kind_is_a_manifest_field() {
        let schema = serde_json::to_value(crate::manifest_json_schema()).unwrap();
        let properties = schema["properties"]
            .as_object()
            .expect("manifest properties");
        for (kind, _) in EXPORT_NOT_ROUND_TRIPPED {
            assert!(
                *kind == "*" || properties.contains_key(*kind),
                "'{kind}' is not a manifest row kind"
            );
        }
    }

    #[test]
    fn json_round_trips_the_table() {
        let parsed: serde_json::Map<String, Value> =
            serde_json::from_str(&export_not_round_tripped_json()).unwrap();
        assert_eq!(parsed.len(), EXPORT_NOT_ROUND_TRIPPED.len());
        assert_eq!(
            parsed["outcome_templates"],
            serde_json::json!(["outcome_bool"])
        );
    }
}
