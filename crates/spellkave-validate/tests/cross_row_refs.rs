//! Authors receive diagnostics for missing or incompatible referenced rows.
//! Uses in-memory manifests and committed fixtures through Cargo.
//! Spec: srd-conformance-probe Direction C / issue #259.

use spellkave_schema::EntityDefinitionTagRow;
use spellkave_schema::{
    ActionDefinitionRow, ActionDefinitionTagRow, ActorCapabilityRow, ActorEquipmentSlotRow,
    AffordanceTemplateRow, BundleRow, CapabilityDefinitionTagRow, ConditionDefinitionRow,
    ConditionHookRow, ConsequenceFrameWriteRow, ConsequenceTemplateRow,
    EconomyChannelDefinitionRow, EntityKindRow, EntityRow, EntityTagRow,
    EquipmentSlotDefinitionRow, ItemDefinitionRow, Manifest, ModifierTemplateRow,
    NicheDefinitionTagRow, OutcomeTemplateRow, OutcomeTemplateTagRow, PlaceEdgeAffordanceRow,
    PlaceGraphRow, ResourceDefinitionRow, RollTableEntryRow, RollTableRow, RollTableTagRow,
    SchemaVersion,
};
use spellkave_schema::{
    CapabilityDefinitionRow, EntityDefinitionEquipModifierRow,
    EntityDefinitionInitialCapabilityRow, EntityDefinitionInitialConditionRow,
    EntityDefinitionInitialStateRow, EntityDefinitionItemDslSourceRow, EntityDefinitionRow,
    ObjectStateDefinitionRow,
};
use spellkave_validate::MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION;
use std::collections::{BTreeMap, BTreeSet};

use spellkave_validate::{
    validate_manifest, validate_manifest_with_world, LiveVocabulary, Severity,
    MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF, MF_VAL_ACTOR_EQUIP_SLOT_DEF_REF,
    MF_VAL_AFFORDANCE_BINDING_COVERAGE, MF_VAL_AFFORDANCE_KNOWS_ENTITY_BINDING_REF,
    MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR, MF_VAL_CONDITION_CAPABILITY_REF,
    MF_VAL_CONDITION_IMPLIES_REF, MF_VAL_CROSS_BUNDLE_SCOPE, MF_VAL_DSL_FORMULA_STAT_REF,
    MF_VAL_DSL_ROLL_TABLE_REF, MF_VAL_DSL_TEMPLATE_ID_LITERAL, MF_VAL_ENTITY_TAG_ENTITY_REF,
    MF_VAL_EQUIP_SLOT_DUP, MF_VAL_FK_CLOSURE, MF_VAL_FRAME_WRITE_CHANNEL_REF,
    MF_VAL_FRAME_WRITE_DSL_PARSE, MF_VAL_FRAME_WRITE_DSL_TYPE, MF_VAL_FRAME_WRITE_RESOURCE_REF,
    MF_VAL_HOOK_GRAFTED_REF, MF_VAL_ITEM_DEF_SLOT_REF, MF_VAL_MANIP_ACTION_ACTION_REF,
    MF_VAL_MANIP_ACTION_ON_SUCCESS, MF_VAL_MANIP_ACTION_TAG_SHAPE, MF_VAL_MANIP_ACTION_TAG_TYPO,
    MF_VAL_MANIP_ACTION_TEMPLATE_ACTION_REF, MF_VAL_ROLL_TABLE_DICE_SMUGGLING,
    MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF, MF_VAL_SELF_REACTION_RESOURCE_REF,
    MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR, MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG,
};
use spellkave_validate::{
    MF_VAL_ACTION_REQUIRED_CAPABILITY_REF, MF_VAL_CAPABILITY_DEFINITION_DUP,
    MF_VAL_SPELL_CAPABILITY_CATALOG_CONVENTION,
};
use spellkave_validate::{
    MF_VAL_DEF_SEED_CAPABILITY_REF, MF_VAL_DEF_SEED_CONDITION_REF, MF_VAL_DEF_SEED_ENTITY_REF,
    MF_VAL_DEF_SEED_MODIFIER_TEMPLATE_REF, MF_VAL_DEF_SEED_STATE_REF,
    MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF,
};
// ADR-0101 D-1/D-5 (movement-defaults-modes-terrain-cost).
use spellkave_schema::MovementDefaultRow;
use spellkave_validate::{
    MF_VAL_MOVEMENT_DEFAULT_BINDING_COVERAGE, MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT,
    MF_VAL_MOVEMENT_MODE_TEMPLATE_REPEATED,
};
// manifest-natural-keys 1.3.
use spellkave_validate::MF_VAL_FRAME_WRITE_ORDINAL_DUP;

fn base_manifest() -> Manifest {
    let mut m = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        ..Manifest::default()
    };
    m.bundles.push(BundleRow {
        bundle_id: "core".to_owned(),
        display_name: "core".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m
}

fn place_graph(id: &str, bundle: &str) -> PlaceGraphRow {
    PlaceGraphRow {
        place_graph_id: id.to_owned(),
        bundle_id: bundle.to_owned(),
        substrate_id: String::new(),
        display_name: id.to_owned(),
        origin_cell_res10: 0,
        origin_cell_res7: 0,
        origin_cell_res4: 0,
        description: String::new(),
        bearing: 0.0,
        metres_per_unit: 1.0,
        blueprint_id: String::new(),
        parent_graph_id: String::new(),
        anchor_node_id: String::new(),
        is_ephemeral_instance: false,
    }
}

fn actor_capability(extent_ref: &str) -> ActorCapabilityRow {
    ActorCapabilityRow {
        actor_entity_key: "hero".to_owned(),
        capability_id: "core/see".to_owned(),
        source: "test".to_owned(),
        source_args_hash: String::new(),
        bundle_id: "core".to_owned(),
        visibility: Some("public".to_owned()),
        extent_ref: extent_ref.to_owned(),
        extent_kind_ref: String::new(),
        elevation_class: String::new(),
    }
}

#[test]
fn actor_capability_extent_ref_resolved_admits_clean() {
    let mut m = base_manifest();
    m.place_graphs.push(place_graph("core/tavern", "core"));
    m.actor_capabilities.push(actor_capability("core/tavern"));
    assert!(validate_manifest(&m)
        .iter()
        .all(|d| d.path != "actor_capabilities.extent_ref"));
}

#[test]
fn actor_capability_extent_ref_missing_is_fatal() {
    let mut m = base_manifest();
    m.place_graphs.push(place_graph("core/tavern", "core"));
    m.actor_capabilities.push(actor_capability("core/typo"));
    let diags: Vec<_> = validate_manifest(&m)
        .into_iter()
        .filter(|d| d.path == "actor_capabilities.extent_ref")
        .collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, MF_VAL_FK_CLOSURE);
    assert_eq!(diags[0].severity, Severity::Fatal);
    assert_eq!(
        diags[0].message,
        "extent_ref 'core/typo' does not resolve to a declared place graph"
    );
}

#[test]
fn actor_capability_empty_extent_ref_admits_clean() {
    let mut m = base_manifest();
    m.actor_capabilities.push(actor_capability(""));
    assert!(validate_manifest(&m)
        .iter()
        .all(|d| d.path != "actor_capabilities.extent_ref"));
}

#[test]
fn actor_capability_party_extent_ref_is_open_id() {
    let mut m = base_manifest();
    let mut row = actor_capability("phandelver/party");
    row.extent_kind_ref = "party".to_owned();
    m.actor_capabilities.push(row);
    assert!(validate_manifest(&m)
        .iter()
        .all(|d| d.path != "actor_capabilities.extent_ref"));
}

#[test]
fn bundle_extent_uses_declared_bundle_and_rejects_unknown_bundle() {
    for (bundle, valid) in [("core", true), ("unknown", false), ("", true)] {
        let mut manifest = base_manifest();
        let mut row = actor_capability(bundle);
        row.extent_kind_ref = "bundle".into();
        manifest.actor_capabilities.push(row);
        let invalid = validate_manifest(&manifest)
            .iter()
            .any(|d| d.path == "actor_capabilities.extent_ref");
        assert_eq!(invalid, !valid);
    }
    let mut manifest = base_manifest();
    let mut row = actor_capability("already-published");
    row.extent_kind_ref = "bundle".into();
    manifest.actor_capabilities.push(row);
    let live = LiveVocabulary {
        bundles: BTreeSet::from(["already-published".to_owned()]),
        ..Default::default()
    };
    assert!(
        validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live))
            .iter()
            .all(|d| d.path != "actor_capabilities.extent_ref")
    );
}

#[test]
fn extent_kind_definition_unknown_shape_is_fatal() {
    let mut m = base_manifest();
    m.extent_kind_definitions
        .push(spellkave_schema::ExtentKindDefinitionRow {
            kind_id: "homebrew/weird".to_owned(),
            resolution_shape: "H3Cell".to_owned(),
            bundle_id: "core".to_owned(),
            description: "deferred shape".to_owned(),
        });
    let diags: Vec<_> = validate_manifest(&m)
        .into_iter()
        .filter(|d| d.path == "extent_kind_definitions.resolution_shape")
        .collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, MF_VAL_FK_CLOSURE);
}

/// ADR-0134 D-1: `metric_shape` is the closed kernel formula set.
#[test]
fn distance_metric_definition_unknown_shape_is_fatal() {
    let mut m = base_manifest();
    m.distance_metric_definitions
        .push(spellkave_schema::DistanceMetricDefinitionRow {
            metric_id: "homebrew/metric/hex".to_owned(),
            metric_shape: "Hexagonal".to_owned(),
            bundle_id: "core".to_owned(),
            description: "not a kernel formula".to_owned(),
        });
    let diags: Vec<_> = validate_manifest(&m)
        .into_iter()
        .filter(|d| d.path == "distance_metric_definitions.metric_shape")
        .collect();
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, MF_VAL_FK_CLOSURE);
}

fn repo_fixture(path: &str) -> Manifest {
    let manifest_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("cargo test must supply the current package directory"),
    );
    let repo = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("validate crate should live under repo/crates");
    let source = std::fs::read_to_string(repo.join(path)).expect("fixture should be readable");
    spellkave_format::parse_manifest(&source)
        .expect("fixture should parse as Manifest")
        .manifest
}

fn manipulation_manifest(tags: &str) -> Manifest {
    let mut m = base_manifest();
    m.outcome_templates.push(OutcomeTemplateRow {
        outcome_template_id: "core/manipulate/outcome".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Manipulate Outcome".to_owned(),
        outcome_bool: true,
        outcome_expression_source: String::new(),
        manipulation_action_id: String::new(),
        communication_speech_act_id: String::new(),
        movement_descriptor: String::new(),
        sense_query_kind: String::new(),
        sense_visibility: spellkave_schema::VisibilityClass::Local,
        consequence_template_id: String::new(),
        discovery_validated: false,
        description: String::new(),
    });
    // The inline `outcome_templates.tags` CSV was removed (CR-4); the
    // deprecated-descriptor lint now reads the relational `[[outcome_template_tag]]`
    // section, so author the test's descriptor tags as relational rows.
    for tag in tags.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        m.outcome_template_tag.push(OutcomeTemplateTagRow {
            outcome_template_id: "core/manipulate/outcome".to_owned(),
            tag: tag.to_owned(),
            bundle_id: "core".to_owned(),
            source: "manifest".to_owned(),
        });
    }
    m.affordance_templates.push(AffordanceTemplateRow {
        template_id: "core/manipulate/affordance".to_owned(),
        bundle_id: "core".to_owned(),
        niche_id: "core/manipulate".to_owned(),
        capability_id: "core/manipulate".to_owned(),
        display_name: "Manipulate".to_owned(),
        predicate_bool: true,
        predicate_source: String::new(),
        accepts_global_storm: false,
        locality_class: String::new(),
        reads_occupancy: false,
        description: String::new(),
        stratification_ok: true,
        outcome_template_id: "core/manipulate/outcome".to_owned(),
        range_m: 0.0,
        spends_channel: None,
        recharges_on: None,
        composes: None,

        movement_cost_expression_source: String::new(),
    });
    m.action_definitions.push(ActionDefinitionRow {
        action_id: "core/manipulate/affordance".to_owned(),
        bundle_id: "core".to_owned(),
        required_capability: "core/manipulate".to_owned(),
        on_success: "core/manipulate/success".to_owned(),
        on_failure: String::new(),
        resolution_mode: Default::default(),
        resolution_predicate_source: String::new(),
        resolution_scope: String::new(),
        non_steering: false,
        display_name: "Manipulate Affordance".to_owned(),
        cost_time_ms: 250,
        session_def_id: String::new(),
        resumable: false,
        reaction_eligibility: None,
        reaction_window_ms: None,
        reaction_trigger: None,
        self_eligible: false,
        self_only: false,
        description: String::new(),
        spends_channel: None,
    });
    m.action_definitions.push(ActionDefinitionRow {
        action_id: "core/manipulate".to_owned(),
        bundle_id: "core".to_owned(),
        required_capability: "core/manipulate".to_owned(),
        on_success: "core/manipulate/success".to_owned(),
        on_failure: String::new(),
        resolution_mode: Default::default(),
        resolution_predicate_source: String::new(),
        resolution_scope: String::new(),
        non_steering: false,
        display_name: "Manipulate".to_owned(),
        cost_time_ms: 250,
        session_def_id: String::new(),
        resumable: false,
        reaction_eligibility: None,
        reaction_window_ms: None,
        reaction_trigger: None,
        self_eligible: false,
        self_only: false,
        description: String::new(),
        spends_channel: None,
    });
    m
}

/// A legal int roll_table — non-identity values so MF-VAL-317 does not fire.
/// Used wherever tests only need a resolvable `roll_table('<id>')` target.
fn sample_int_table(bundle: &str) -> (RollTableRow, Vec<RollTableEntryRow>) {
    let id = format!("{bundle}/severity");
    let table = RollTableRow {
        roll_table_id: id.clone(),
        bundle_id: bundle.to_owned(),
        display_name: "severity".to_owned(),
        result_kind: "int".to_owned(),
        description: String::new(),
    };
    // Weighted non-identity (10/20/30) — not a smuggled die.
    let entries = [(10_i64, 6_u32), (20, 3), (30, 1)]
        .into_iter()
        .enumerate()
        .map(|(i, (value, weight))| RollTableEntryRow {
            roll_table_entry_id: format!("{id}:{}", i + 1),
            roll_table_id: id.clone(),
            authored_order: (i as u32) + 1,
            weight,
            result_ref: value.to_string(),
        })
        .collect();
    (table, entries)
}

/// Identity 1..N weight-1 int table — the MF-VAL-317 reject shape.
fn identity_die_table(bundle: &str, n: u32) -> (RollTableRow, Vec<RollTableEntryRow>) {
    let id = format!("{bundle}/d{n}");
    let table = RollTableRow {
        roll_table_id: id.clone(),
        bundle_id: bundle.to_owned(),
        display_name: format!("d{n}"),
        result_kind: "int".to_owned(),
        description: String::new(),
    };
    let entries = (1..=n)
        .map(|i| RollTableEntryRow {
            roll_table_entry_id: format!("{id}:{i}"),
            roll_table_id: id.clone(),
            authored_order: i,
            weight: 1,
            result_ref: i.to_string(),
        })
        .collect();
    (table, entries)
}

fn codes_for(diags: &[spellkave_validate::Diagnostic], code: &str) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.code == code)
        .map(|d| d.message.clone())
        .collect()
}

fn fatal_codes(diags: &[spellkave_validate::Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.severity == Severity::Fatal)
        .map(|d| d.code.clone())
        .collect()
}

#[test]
fn arbitration_outcome_graph_authoring_fixture_validates() {
    // Archived in the implementing PR (#681): acceptance fixtures move with the
    // change to archive/ on archive — this permanent regression test follows them.
    let manifest = repo_fixture(
        "openspec/changes/archive/2026-06-14-arbitration-outcome-graph-authoring/fixtures/authored-outcome-graph.toml",
    );
    let diags = validate_manifest(&manifest);
    assert_eq!(fatal_codes(&diags), Vec::<String>::new(), "{diags:?}");
}

#[test]
fn contested_grab_arbitration_fixture_validates() {
    let manifest = repo_fixture(
        "openspec/changes/archive/2026-06-14-arbitration-outcome-graph-authoring/fixtures/contested-grab.toml",
    );
    let diags = validate_manifest(&manifest);
    assert_eq!(fatal_codes(&diags), Vec::<String>::new(), "{diags:?}");
}

// ──────────────────────────────────────────────────────────────────────────
// Class 1 — roll_table_entry.result_ref
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn class1_passing_outcome_graph_resolves() {
    let mut m = base_manifest();
    m.outcome_templates.push(OutcomeTemplateRow {
        outcome_template_id: "core/red".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Red".to_owned(),
        outcome_bool: false,
        outcome_expression_source: String::new(),
        manipulation_action_id: String::new(),
        communication_speech_act_id: String::new(),
        movement_descriptor: String::new(),
        sense_query_kind: String::new(),
        sense_visibility: spellkave_schema::VisibilityClass::Local,
        consequence_template_id: String::new(),
        discovery_validated: false,
        description: String::new(),
    });
    m.roll_tables.push(RollTableRow {
        roll_table_id: "core/colors".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Colors".to_owned(),
        result_kind: "outcome_graph".to_owned(),
        description: String::new(),
    });
    m.roll_table_entries.push(RollTableEntryRow {
        roll_table_entry_id: "core/colors:1".to_owned(),
        roll_table_id: "core/colors".to_owned(),
        authored_order: 1,
        weight: 1,
        result_ref: "core/red".to_owned(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF).is_empty(),
        "expected no MF-VAL-210; got: {diags:?}"
    );
}

#[test]
fn class1_failing_outcome_graph_dangling_ref_is_fatal() {
    // Canonical regression: the F-typo probe shape.
    let mut m = base_manifest();
    m.outcome_templates.push(OutcomeTemplateRow {
        outcome_template_id: "core/layer_violet".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Violet".to_owned(),
        outcome_bool: false,
        outcome_expression_source: String::new(),
        manipulation_action_id: String::new(),
        communication_speech_act_id: String::new(),
        movement_descriptor: String::new(),
        sense_query_kind: String::new(),
        sense_visibility: spellkave_schema::VisibilityClass::Local,
        consequence_template_id: String::new(),
        discovery_validated: false,
        description: String::new(),
    });
    m.roll_tables.push(RollTableRow {
        roll_table_id: "core/prismatic".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Prismatic".to_owned(),
        result_kind: "outcome_graph".to_owned(),
        description: String::new(),
    });
    m.roll_table_entries.push(RollTableEntryRow {
        roll_table_entry_id: "core/prismatic:7".to_owned(),
        roll_table_id: "core/prismatic".to_owned(),
        authored_order: 7,
        weight: 1,
        result_ref: "core/layer_voilet".to_owned(), // <-- typo (matches F probe)
    });
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF);
    assert_eq!(
        msgs.len(),
        1,
        "expected exactly one MF-VAL-210; got: {diags:?}"
    );
    assert!(msgs[0].contains("layer_voilet"));
    assert!(diags
        .iter()
        .find(|d| d.code == MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF)
        .map(|d| matches!(d.severity, Severity::Fatal))
        .unwrap());
}

#[test]
fn class1_failing_int_kind_bad_parse_is_fatal() {
    let mut m = base_manifest();
    m.roll_tables.push(RollTableRow {
        roll_table_id: "core/d20".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "d20".to_owned(),
        result_kind: "int".to_owned(),
        description: String::new(),
    });
    m.roll_table_entries.push(RollTableEntryRow {
        roll_table_entry_id: "core/d20:bad".to_owned(),
        roll_table_id: "core/d20".to_owned(),
        authored_order: 1,
        weight: 1,
        result_ref: "not_a_number".to_owned(),
    });
    let diags = validate_manifest(&m);
    assert!(!codes_for(&diags, MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF).is_empty());
}

#[test]
fn class1_reserved_result_kind_is_not_flagged() {
    let mut m = base_manifest();
    m.roll_tables.push(RollTableRow {
        roll_table_id: "core/future".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Future".to_owned(),
        result_kind: "future_namespace".to_owned(),
        description: String::new(),
    });
    m.roll_table_entries.push(RollTableEntryRow {
        roll_table_entry_id: "core/future:1".to_owned(),
        roll_table_id: "core/future".to_owned(),
        authored_order: 1,
        weight: 1,
        result_ref: "anything".to_owned(),
    });
    let diags = validate_manifest(&m);
    assert!(codes_for(&diags, MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF).is_empty());
}

#[test]
fn dice_smuggling_identity_1_to_n_weight_1_is_fatal() {
    let mut m = base_manifest();
    let (table, entries) = identity_die_table("core", 6);
    m.roll_tables.push(table);
    m.roll_table_entries.extend(entries);
    let diags = validate_manifest(&m);
    assert_eq!(
        codes_for(&diags, MF_VAL_ROLL_TABLE_DICE_SMUGGLING).len(),
        1,
        "identity 1..6 weight-1 int table must Fatal once; got: {diags:?}"
    );
    assert!(diags
        .iter()
        .filter(|d| d.code == MF_VAL_ROLL_TABLE_DICE_SMUGGLING)
        .all(|d| d.severity == Severity::Fatal));
}

#[test]
fn dice_smuggling_single_entry_constant_table_is_clean() {
    // {result_ref "1", weight 1} alone is a constant lookup (the archived
    // flat-1 damage profiles), not a smuggled '1d1' — a die has >= 2 faces.
    let mut m = base_manifest();
    let (table, entries) = identity_die_table("core", 1);
    m.roll_tables.push(table);
    m.roll_table_entries.extend(entries);
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_ROLL_TABLE_DICE_SMUGGLING).is_empty(),
        "single-entry constant table must not trip MF-VAL-317"
    );
}

#[test]
fn dice_smuggling_weighted_non_identity_is_clean() {
    let mut m = base_manifest();
    let (table, entries) = sample_int_table("core");
    m.roll_tables.push(table);
    m.roll_table_entries.extend(entries);
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_ROLL_TABLE_DICE_SMUGGLING).is_empty(),
        "weighted non-identity int table must not trip MF-VAL-317; got: {diags:?}"
    );
}

#[test]
fn dice_smuggling_uniform_but_not_starting_at_one_is_clean() {
    // 2..4 weight-1 is uniform but not identity 1..N.
    let mut m = base_manifest();
    m.roll_tables.push(RollTableRow {
        roll_table_id: "core/offset".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "offset".to_owned(),
        result_kind: "int".to_owned(),
        description: String::new(),
    });
    for (i, v) in [2_i64, 3, 4].into_iter().enumerate() {
        m.roll_table_entries.push(RollTableEntryRow {
            roll_table_entry_id: format!("core/offset:{}", i + 1),
            roll_table_id: "core/offset".to_owned(),
            authored_order: (i as u32) + 1,
            weight: 1,
            result_ref: v.to_string(),
        });
    }
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_ROLL_TABLE_DICE_SMUGGLING).is_empty(),
        "2..4 weight-1 must not trip dice-smuggling"
    );
}

#[test]
fn dice_smuggling_die_tagged_identity_table_is_clean() {
    // Substrate core/d20 pattern: intentional named die for roll_table() consumers.
    let mut m = base_manifest();
    let (table, entries) = identity_die_table("core", 20);
    let id = table.roll_table_id.clone();
    m.roll_tables.push(table);
    m.roll_table_entries.extend(entries);
    m.roll_table_tag.push(RollTableTagRow {
        roll_table_id: id,
        tag: "die".to_owned(),
        bundle_id: "core".to_owned(),
        source: "manifest".to_owned(),
    });
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_ROLL_TABLE_DICE_SMUGGLING).is_empty(),
        "die-tagged identity tables (core/d20) must remain legal"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Class 2 — DSL roll_table('<id>') refs
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn class2_passing_roll_table_call_resolves() {
    let mut m = base_manifest();
    let (table, entries) = sample_int_table("core");
    m.roll_tables.push(table);
    m.roll_table_entries.extend(entries);
    m.modifier_templates.push(ModifierTemplateRow {
        modifier_template_id: "core/m1".to_owned(),
        bundle_id: "core".to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: "m1".to_owned(),
        predicate_source: String::new(),
        modifier_expression_source:
            "let a = roll_table('core/severity'); let b = roll_table('core/severity'); min(a, b)"
                .to_owned(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_DSL_ROLL_TABLE_REF).is_empty(),
        "expected no MF-VAL-211; got: {diags:?}"
    );
}

#[test]
fn class2_failing_roll_table_call_dangling_ref_is_fatal() {
    let mut m = base_manifest();
    m.modifier_templates.push(ModifierTemplateRow {
        modifier_template_id: "core/m_bad".to_owned(),
        bundle_id: "core".to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: "m".to_owned(),
        predicate_source: String::new(),
        modifier_expression_source:
            "let a = roll_table('core/never_authored'); let b = roll_table('core/d20_also_missing'); min(a, b)".to_owned(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_DSL_ROLL_TABLE_REF);
    assert_eq!(msgs.len(), 2, "expected two MF-VAL-211; got: {diags:?}");
    assert!(msgs.iter().any(|m| m.contains("core/never_authored")));
    assert!(msgs.iter().any(|m| m.contains("core/d20_also_missing")));
}

#[test]
fn class2_roll_table_call_resolves_via_live_vocabulary() {
    // sk-3h5: a delta that only *calls* roll_table('<id>') must see an
    // already-published table through LiveVocabulary, same existence-only
    // shape as economy_channels / modifier_templates.
    let mut m = base_manifest();
    m.modifier_templates.push(ModifierTemplateRow {
        modifier_template_id: "core/m_live".to_owned(),
        bundle_id: "core".to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: "m".to_owned(),
        predicate_source: String::new(),
        modifier_expression_source:
            "let a = roll_table('core/severity'); let b = roll_table('core/severity'); min(a, b)"
                .to_owned(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    });
    let live = LiveVocabulary {
        economy_channels: BTreeMap::new(),
        modifier_templates: BTreeMap::new(),
        equipment_slots: BTreeMap::new(),
        roll_tables: BTreeMap::from([("core/severity".to_owned(), "core".to_owned())]),
        actions: BTreeMap::new(),
        ..Default::default()
    };
    let diags = validate_manifest_with_world(&m, &BTreeSet::new(), Some(&live));
    assert!(
        codes_for(&diags, MF_VAL_DSL_ROLL_TABLE_REF).is_empty(),
        "a live-published roll_table must resolve in-validator; got: {diags:?}"
    );
}

#[test]
fn class2_runtime_roll_table_call_degrades_to_warning_not_fatal() {
    // Option C (G3 / #1254): a runtime `roll_table(actor.wielded_item.text('…'))`
    // cannot be statically resolved → MF-VAL-211 degrades to a Warning (it fails
    // closed at eval, ADR-0035), NOT a Fatal. The literal form still fatals (see
    // class2_failing_roll_table_call_dangling_ref_is_fatal) — static validation
    // is preserved for literals.
    let mut m = base_manifest();
    m.modifier_templates.push(ModifierTemplateRow {
        modifier_template_id: "core/m_runtime".to_owned(),
        bundle_id: "core".to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: "m".to_owned(),
        predicate_source: String::new(),
        modifier_expression_source:
            "let a = roll_table(actor.wielded_item.text('core/table_pointer')); let b = roll_table(actor.wielded_item.text('core/table_pointer')); min(a, b)".to_owned(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    let roll: Vec<&spellkave_validate::Diagnostic> = diags
        .iter()
        .filter(|d| d.code == MF_VAL_DSL_ROLL_TABLE_REF)
        .collect();
    assert!(
        !roll.is_empty(),
        "expected an MF-VAL-211 for the runtime roll_table; got: {diags:?}"
    );
    assert!(
        roll.iter().all(|d| d.severity == Severity::Warning),
        "every runtime roll_table MF-VAL-211 must be a Warning, not Fatal; got: {diags:?}"
    );
    assert!(roll.iter().all(|d| d.message.contains("runtime")));
}

// ──────────────────────────────────────────────────────────────────────────
// Class 3 — DSL $event.detail.template_id == '<literal>'
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn class3_passing_template_id_compare_resolves_to_outcome_template() {
    let mut m = base_manifest();
    m.outcome_templates.push(OutcomeTemplateRow {
        outcome_template_id: "core/known_template".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Known".to_owned(),
        outcome_bool: false,
        outcome_expression_source: String::new(),
        manipulation_action_id: String::new(),
        communication_speech_act_id: String::new(),
        movement_descriptor: String::new(),
        sense_query_kind: String::new(),
        sense_visibility: spellkave_schema::VisibilityClass::Local,
        consequence_template_id: String::new(),
        discovery_validated: false,
        description: String::new(),
    });
    m.condition_definitions.push(ConditionDefinitionRow {
        condition_definition_id: "core/c".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "C".to_owned(),
        stacking_rule: "core/stacking/single".to_owned(),
        remove_time_policy: "core/remove_policy/independent_lifetime".to_owned(),
        gate_predicate_source: "$event.detail.template_id == 'core/known_template'".to_owned(),
        modifier_template_id: String::new(),
        hooks: vec![],
        observable: true,
        grants_capability_while_active: Vec::new(),
        suppresses_capability_while_active: Vec::new(),
        implies: Vec::new(),
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_DSL_TEMPLATE_ID_LITERAL).is_empty(),
        "no warning expected for resolved literal; got: {diags:?}"
    );
}

#[test]
fn class3_dangling_template_id_literal_is_warning_not_fatal() {
    let mut m = base_manifest();
    m.condition_definitions.push(ConditionDefinitionRow {
        condition_definition_id: "core/c".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "C".to_owned(),
        stacking_rule: "core/stacking/single".to_owned(),
        remove_time_policy: "core/remove_policy/independent_lifetime".to_owned(),
        gate_predicate_source: "$event.detail.template_id == 'core/no_such_template'".to_owned(),
        modifier_template_id: String::new(),
        hooks: vec![],
        observable: true,
        grants_capability_while_active: Vec::new(),
        suppresses_capability_while_active: Vec::new(),
        implies: Vec::new(),
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    let warns: Vec<_> = diags
        .iter()
        .filter(|d| d.code == MF_VAL_DSL_TEMPLATE_ID_LITERAL)
        .collect();
    assert_eq!(warns.len(), 1, "expected one MF-VAL-212; got: {diags:?}");
    assert!(matches!(warns[0].severity, Severity::Warning));
}

// ──────────────────────────────────────────────────────────────────────────
// Class 4 — grafted condition_hook AST refs
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn class4_grafted_hook_expression_source_with_dangling_roll_table_is_fatal() {
    let mut m = base_manifest();
    m.modifier_templates.push(ModifierTemplateRow {
        modifier_template_id: "core/m".to_owned(),
        bundle_id: "core".to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: "m".to_owned(),
        predicate_source: String::new(),
        modifier_expression_source: String::new(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    });
    m.condition_definitions.push(ConditionDefinitionRow {
        condition_definition_id: "core/c".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "C".to_owned(),
        stacking_rule: "core/stacking/single".to_owned(),
        remove_time_policy: "core/remove_policy/independent_lifetime".to_owned(),
        // Gate predicate: a literal `true`-shape comparison so the
        // outer condition does not also raise a parse error.
        gate_predicate_source: "1 == 1".to_owned(),
        modifier_template_id: "core/m".to_owned(),
        hooks: vec![ConditionHookRow {
            event_kind: "Layer2.ApplyConsequence".to_owned(),
            detail_filter: None,
            detail_filter_source: None,
            expression: None,
            // Grafted expression — dangling roll_table call.
            expression_source: Some(
                "let a = roll_table('core/missing_table'); let b = roll_table('core/missing_table'); min(a, b)"
                    .to_owned(),
            ),
        }],
        observable: true,
        grants_capability_while_active: Vec::new(),
        suppresses_capability_while_active: Vec::new(),
        implies: Vec::new(),
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_HOOK_GRAFTED_REF);
    assert!(
        !msgs.is_empty(),
        "expected at least one MF-VAL-213; got: {diags:?}"
    );
    assert!(msgs.iter().any(|m| m.contains("core/missing_table")));
}

// ──────────────────────────────────────────────────────────────────────────
// Class 5 — cross-bundle scope
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn class5_srd_references_core_without_depends_on_is_fatal() {
    // Author an srd bundle (no `depends_on = ["core"]` — but in our impl
    // "core" is always implicitly in scope). To exercise the cross-bundle
    // failure cleanly, author *two* peer add-on bundles ("foo", "bar") and
    // have foo reference a bar-owned roll_table without declaring the
    // dependency.
    let mut m = base_manifest();
    m.bundles.push(BundleRow {
        bundle_id: "foo".to_owned(),
        display_name: "foo".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m.bundles.push(BundleRow {
        bundle_id: "bar".to_owned(),
        display_name: "bar".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });

    // bar authors the roll_table.
    let (bar_table, bar_entries) = sample_int_table("bar");
    m.roll_tables.push(bar_table);
    m.roll_table_entries.extend(bar_entries);

    // foo authors a modifier that calls bar's roll_table without depending on bar.
    m.modifier_templates.push(ModifierTemplateRow {
        modifier_template_id: "foo/m".to_owned(),
        bundle_id: "foo".to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: "m".to_owned(),
        predicate_source: String::new(),
        modifier_expression_source:
            "let a = roll_table('bar/severity'); let b = roll_table('bar/severity'); min(a, b)"
                .to_owned(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_CROSS_BUNDLE_SCOPE);
    assert!(
        !msgs.is_empty(),
        "expected at least one MF-VAL-214; got: {diags:?}"
    );
    assert!(msgs.iter().all(|m| m.contains("bar/severity")));
    assert!(msgs.iter().all(|m| m.contains("foo")));
}

#[test]
fn class5_passing_with_explicit_depends_on() {
    let mut m = base_manifest();
    m.bundles.push(BundleRow {
        bundle_id: "foo".to_owned(),
        display_name: "foo".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec!["bar".to_owned()],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m.bundles.push(BundleRow {
        bundle_id: "bar".to_owned(),
        display_name: "bar".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    let (bar_table, bar_entries) = sample_int_table("bar");
    m.roll_tables.push(bar_table);
    m.roll_table_entries.extend(bar_entries);
    m.modifier_templates.push(ModifierTemplateRow {
        modifier_template_id: "foo/m".to_owned(),
        bundle_id: "foo".to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: "m".to_owned(),
        predicate_source: String::new(),
        modifier_expression_source:
            "let a = roll_table('bar/severity'); let b = roll_table('bar/severity'); min(a, b)"
                .to_owned(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_CROSS_BUNDLE_SCOPE).is_empty(),
        "expected no MF-VAL-214; got: {diags:?}"
    );
}

#[test]
fn equipment_slot_id_ref_rejects_unresolved_item_definition_slot() {
    let mut m = base_manifest();
    m.item_definitions.push(ItemDefinitionRow {
        item_def_id: "core/item/horn_blade".to_owned(),
        bundle_id: "core".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        slot_compatibility_predicate: String::new(),
        description: String::new(),
    });

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_ITEM_DEF_SLOT_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-215; got: {diags:?}");
    assert!(msgs[0].contains("homebrew/horn-sheath"));
}

#[test]
fn equipment_slot_id_ref_accepts_existing_slot_definition() {
    let mut m = base_manifest();
    m.equipment_slot_definitions
        .push(EquipmentSlotDefinitionRow {
            slot_id: "homebrew/horn-sheath".to_owned(),
            bundle_id: "core".to_owned(),
            label: "Horn Sheath".to_owned(),
            default_compatibility_predicate: String::new(),
            description: String::new(),
        });
    m.item_definitions.push(ItemDefinitionRow {
        item_def_id: "core/item/horn_blade".to_owned(),
        bundle_id: "core".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        slot_compatibility_predicate: String::new(),
        description: String::new(),
    });

    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_ITEM_DEF_SLOT_REF).is_empty(),
        "expected no MF-VAL-215; got: {diags:?}"
    );
}

#[test]
fn actor_equipment_slot_item_key_ref_rejects_unresolved_item() {
    let mut m = base_manifest();
    m.actor_equipment_slots.push(ActorEquipmentSlotRow {
        actor_entity_key: "core/player".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        item_key: "homebrew/item/horn_blade".to_owned(),
    });

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-216; got: {diags:?}");
    assert!(msgs[0].contains("homebrew/item/horn_blade"));
}

#[test]
fn actor_equipment_slot_item_key_ref_accepts_existing_item_entity() {
    let mut m = base_manifest();
    m.entities.push(EntityRow {
        entity_key: "core/player".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: String::new(),
        name: "Player".to_owned(),
        kind: EntityKindRow::Player,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.entities.push(EntityRow {
        entity_key: "homebrew/item/horn_blade".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: "homebrew/def/horn_blade".to_owned(),
        name: "Horn Blade".to_owned(),
        kind: EntityKindRow::Item,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.actor_equipment_slots.push(ActorEquipmentSlotRow {
        actor_entity_key: "core/player".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        item_key: "homebrew/item/horn_blade".to_owned(),
    });

    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF).is_empty(),
        "expected no MF-VAL-216; got: {diags:?}"
    );
}

#[test]
fn actor_equipment_slot_slot_id_ref_rejects_unresolved_slot_definition() {
    let mut m = base_manifest();
    m.entities.push(EntityRow {
        entity_key: "homebrew/item/horn_blade".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: "homebrew/def/horn_blade".to_owned(),
        name: "Horn Blade".to_owned(),
        kind: EntityKindRow::Item,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.actor_equipment_slots.push(ActorEquipmentSlotRow {
        actor_entity_key: "core/player".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        item_key: "homebrew/item/horn_blade".to_owned(),
    });

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_ACTOR_EQUIP_SLOT_DEF_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-319; got: {diags:?}");
    assert!(msgs[0].contains("homebrew/horn-sheath"));
}

#[test]
fn actor_equipment_slot_slot_id_ref_accepts_existing_slot_definition() {
    let mut m = base_manifest();
    m.equipment_slot_definitions
        .push(EquipmentSlotDefinitionRow {
            slot_id: "homebrew/horn-sheath".to_owned(),
            bundle_id: "core".to_owned(),
            label: "Horn Sheath".to_owned(),
            default_compatibility_predicate: String::new(),
            description: String::new(),
        });
    m.entities.push(EntityRow {
        entity_key: "homebrew/item/horn_blade".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: "homebrew/def/horn_blade".to_owned(),
        name: "Horn Blade".to_owned(),
        kind: EntityKindRow::Item,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.actor_equipment_slots.push(ActorEquipmentSlotRow {
        actor_entity_key: "core/player".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        item_key: "homebrew/item/horn_blade".to_owned(),
    });

    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_ACTOR_EQUIP_SLOT_DEF_REF).is_empty(),
        "expected no MF-VAL-319; got: {diags:?}"
    );
}

#[test]
fn actor_equipment_slot_duplicate_actor_slot_is_rejected() {
    let mut m = base_manifest();
    m.entities.push(EntityRow {
        entity_key: "core/player".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: String::new(),
        name: "Player".to_owned(),
        kind: EntityKindRow::Player,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.entities.push(EntityRow {
        entity_key: "homebrew/item/horn_blade-1".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: "homebrew/def/horn_blade".to_owned(),
        name: "Horn Blade 1".to_owned(),
        kind: EntityKindRow::Item,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.entities.push(EntityRow {
        entity_key: "homebrew/item/horn_blade-2".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: "homebrew/def/horn_blade".to_owned(),
        name: "Horn Blade 2".to_owned(),
        kind: EntityKindRow::Item,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.actor_equipment_slots.push(ActorEquipmentSlotRow {
        actor_entity_key: "core/player".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        item_key: "homebrew/item/horn_blade-1".to_owned(),
    });
    m.actor_equipment_slots.push(ActorEquipmentSlotRow {
        actor_entity_key: "core/player".to_owned(),
        slot_id: "homebrew/horn-sheath".to_owned(),
        item_key: "homebrew/item/horn_blade-2".to_owned(),
    });

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_EQUIP_SLOT_DUP);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-219; got: {diags:?}");
    assert!(msgs[0].contains("core/player"));
    assert!(msgs[0].contains("homebrew/horn-sheath"));
}

// ──────────────────────────────────────────────────────────────────────────
// Class 6 — consequence_frame_write ModifyResource.resource ref (MF-VAL-217),
// including the recursive walk into a ForEachActorInRadius body.
// Added by Direction H / Phase 14.b of the SRD probe. (The old MF-VAL-218
// center_id sentinel was retired with the field per ADR-0042 D-2.)
// ──────────────────────────────────────────────────────────────────────────

fn frame_write_template(bundle: &str, template_id: &str) -> ConsequenceTemplateRow {
    ConsequenceTemplateRow {
        template_id: template_id.to_owned(),
        bundle_id: bundle.to_owned(),
        effect_refs_csv: String::new(),
        traces_json: "[]".to_owned(),
        description: String::new(),
    }
}

fn hp_resource(bundle: &str) -> ResourceDefinitionRow {
    ResourceDefinitionRow {
        resource_definition_id: format!("{bundle}/hp"),
        bundle_id: bundle.to_owned(),
        display_name: "Hit Points".to_owned(),
        value_kind: "int".to_owned(),
        min_value: 0,
        max_value: 999,
        recovery_rule: "long_rest".to_owned(),
        initial_value: None,
        description: String::new(),
    }
}

fn self_reaction_action(bundle: &str, action_id: &str, on_success: &str) -> ActionDefinitionRow {
    ActionDefinitionRow {
        action_id: action_id.to_owned(),
        bundle_id: bundle.to_owned(),
        required_capability: String::new(),
        on_success: on_success.to_owned(),
        on_failure: String::new(),
        resolution_mode: Default::default(),
        resolution_predicate_source: String::new(),
        resolution_scope: String::new(),
        non_steering: false,
        display_name: "Legendary Resistance".to_owned(),
        cost_time_ms: 0,
        session_def_id: String::new(),
        resumable: false,
        reaction_eligibility: Some("true".to_owned()),
        reaction_window_ms: Some(1000),
        reaction_trigger: Some("true".to_owned()),
        self_eligible: true,
        self_only: false,
        description: String::new(),
        spends_channel: None,
    }
}

#[test]
fn class6_passing_modify_resource_resource_resolves() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/heal/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/heal/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "5" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_FRAME_WRITE_RESOURCE_REF).is_empty(),
        "expected no MF-VAL-217; got: {diags:?}"
    );
}

// manifest-natural-keys 1.3 (design D3): consequence_frame_writes' storage
// key is (template_id, ordinal); two blocks of one template sharing an
// ordinal must be a fatal, named diagnostic at authoring time, not a
// commit-time primary-key collision.
// ──────────────────────────────────────────────────────────────────────────

fn heal_frame_write(template_id: &str, ordinal: u32) -> ConsequenceFrameWriteRow {
    ConsequenceFrameWriteRow {
        template_id: template_id.to_owned(),
        ordinal,
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "5" }
        }),
    }
}

#[test]
fn duplicate_frame_write_ordinal_in_one_template_is_fatal() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "demo/hit/success"));
    m.consequence_frame_writes
        .push(heal_frame_write("demo/hit/success", 0));
    m.consequence_frame_writes
        .push(heal_frame_write("demo/hit/success", 0));
    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_FRAME_WRITE_ORDINAL_DUP);
    assert_eq!(
        codes.len(),
        1,
        "expected exactly one MF-VAL-335; got: {diags:?}"
    );
    assert!(codes[0].contains("demo/hit/success"), "{}", codes[0]);
    assert!(codes[0].contains('0'), "{}", codes[0]);
}

#[test]
fn distinct_ordinals_in_one_template_do_not_conflict() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "demo/hit/success"));
    m.consequence_frame_writes
        .push(heal_frame_write("demo/hit/success", 0));
    m.consequence_frame_writes
        .push(heal_frame_write("demo/hit/success", 1));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_FRAME_WRITE_ORDINAL_DUP).is_empty(),
        "expected no MF-VAL-335; got: {diags:?}"
    );
}

#[test]
fn the_same_ordinal_on_two_different_templates_does_not_conflict() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "demo/hit/success"));
    m.consequence_templates
        .push(frame_write_template("core", "demo/other/success"));
    m.consequence_frame_writes
        .push(heal_frame_write("demo/hit/success", 0));
    m.consequence_frame_writes
        .push(heal_frame_write("demo/other/success", 0));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_FRAME_WRITE_ORDINAL_DUP).is_empty(),
        "expected no MF-VAL-335; got: {diags:?}"
    );
}

#[test]
fn class6_failing_modify_resource_resource_dangles() {
    let mut m = base_manifest();
    // No resource_definitions row authored.
    m.consequence_templates
        .push(frame_write_template("core", "core/burn/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/burn/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "hp", "delta_source": "-3" }
        }),
    });
    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_FRAME_WRITE_RESOURCE_REF);
    assert_eq!(
        codes.len(),
        1,
        "expected exactly one MF-VAL-217; got: {diags:?}"
    );
    assert!(
        codes[0].contains("'hp'"),
        "expected diagnostic to name the dangling resource; got: {}",
        codes[0]
    );
    assert!(diags
        .iter()
        .any(|d| d.code == MF_VAL_FRAME_WRITE_RESOURCE_REF && d.severity == Severity::Fatal));
}

#[test]
fn class6_nested_inside_for_each_actor_in_radius_is_walked() {
    let mut m = base_manifest();
    // ForEachActorInRadius wraps ModifyResource with a dangling resource.
    m.consequence_templates
        .push(frame_write_template("core", "core/aoe/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/aoe/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ForEachActorInRadius": {
                "center_ref": "ActorSpatialRef",
                "shape_id": "core/area/node_ball",
                "shape_params_json": "{\"ball_radius\": 0}",
                "predicate_source": "",
                "body": [
                    { "ModifyResource": { "actor": "Actor", "resource": "phantom", "delta_source": "-1" } }
                ]
            }
        }),
    });
    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_FRAME_WRITE_RESOURCE_REF);
    assert_eq!(
        codes.len(),
        1,
        "expected one MF-VAL-217 from nested body; got: {diags:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Class 6 sibling (sk-ct8.4 / finding #86) — consequence_frame_write
// ModifyChannel.channel ref (MF-VAL-312).
// ──────────────────────────────────────────────────────────────────────────

fn action_class_channel(bundle: &str) -> EconomyChannelDefinitionRow {
    EconomyChannelDefinitionRow {
        id: format!("{bundle}/action_class/primary"),
        bundle_id: bundle.to_owned(),
        pool_ms_per_round: 1_000,
        resets_on: "turn_start".to_owned(),
        off_turn_eligible: false,
        description: String::new(),
    }
}

#[test]
fn class6_passing_modify_channel_channel_resolves() {
    let mut m = base_manifest();
    m.economy_channel_definitions
        .push(action_class_channel("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/action_surge/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/action_surge/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyChannel": { "actor": "Actor", "channel": "core/action_class/primary" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_FRAME_WRITE_CHANNEL_REF).is_empty(),
        "expected no MF-VAL-312; got: {diags:?}"
    );
}

#[test]
fn class6_failing_modify_channel_channel_dangles() {
    let mut m = base_manifest();
    // No economy_channel_definitions row authored.
    m.consequence_templates
        .push(frame_write_template("core", "core/burn_action/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/burn_action/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyChannel": { "actor": "Actor", "channel": "ghost_channel", "delta_source": "-500" }
        }),
    });
    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_FRAME_WRITE_CHANNEL_REF);
    assert_eq!(
        codes.len(),
        1,
        "expected exactly one MF-VAL-312; got: {diags:?}"
    );
    assert!(
        codes[0].contains("'ghost_channel'"),
        "expected diagnostic to name the dangling channel; got: {}",
        codes[0]
    );
    assert!(diags
        .iter()
        .any(|d| d.code == MF_VAL_FRAME_WRITE_CHANNEL_REF && d.severity == Severity::Fatal));
}

/// Empty/omitted `delta_source` is the authored default (+1 full pool) — it
/// must NOT be sent through the DSL Int parser (which would reject an empty
/// string).
#[test]
fn class6_modify_channel_empty_delta_source_is_not_parsed_as_dsl() {
    let mut m = base_manifest();
    m.economy_channel_definitions
        .push(action_class_channel("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/action_surge/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/action_surge/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyChannel": { "actor": "Actor", "channel": "core/action_class/primary", "delta_source": "" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_FRAME_WRITE_DSL_PARSE).is_empty(),
        "empty delta_source must not be sent through the DSL parser; got: {diags:?}"
    );
}

/// A malformed non-empty `delta_source` still fails loudly (MF-VAL-256),
/// same posture as `ModifyResource.delta_source`.
#[test]
fn class6_modify_channel_malformed_delta_source_is_fatal() {
    let mut m = base_manifest();
    m.economy_channel_definitions
        .push(action_class_channel("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/action_surge/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/action_surge/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyChannel": { "actor": "Actor", "channel": "core/action_class/primary", "delta_source": "((" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        !codes_for(&diags, MF_VAL_FRAME_WRITE_DSL_PARSE).is_empty(),
        "malformed delta_source must fail MF-VAL-256; got: {diags:?}"
    );
}

#[test]
fn class6_modify_channel_cross_bundle_scope_is_fatal() {
    // "core" is always implicitly in every bundle's reach (BundleScope::build),
    // so the cross-bundle probe needs two NON-core bundles — same shape the
    // roll_table cross-bundle-scope test above uses ("foo"/"bar").
    let mut m = base_manifest();
    m.bundles.push(BundleRow {
        bundle_id: "foo".to_owned(),
        display_name: "foo".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![], // does NOT depend on "bar"
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m.bundles.push(BundleRow {
        bundle_id: "bar".to_owned(),
        display_name: "bar".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    // bar authors the channel.
    m.economy_channel_definitions
        .push(action_class_channel("bar"));
    // foo's consequence references bar's channel without depending on bar.
    m.consequence_templates
        .push(frame_write_template("foo", "foo/action_surge/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "foo/action_surge/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyChannel": { "actor": "Actor", "channel": "bar/action_class/primary" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        !codes_for(&diags, MF_VAL_CROSS_BUNDLE_SCOPE).is_empty(),
        "expected MF-VAL-214 for a channel outside the owning bundle's depends_on closure; got: {diags:?}"
    );
}

#[test]
fn class6_modify_channel_nested_inside_for_each_actor_in_radius_is_walked() {
    let mut m = base_manifest();
    m.consequence_templates
        .push(frame_write_template("core", "core/aoe_surge/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/aoe_surge/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ForEachActorInRadius": {
                "center_ref": "ActorSpatialRef",
                "shape_id": "core/area/node_ball",
                "shape_params_json": "{\"ball_radius\": 0}",
                "predicate_source": "",
                "body": [
                    { "ModifyChannel": { "actor": "Actor", "channel": "phantom_channel" } }
                ]
            }
        }),
    });
    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_FRAME_WRITE_CHANNEL_REF);
    assert_eq!(
        codes.len(),
        1,
        "expected one MF-VAL-312 from nested body; got: {diags:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Reference tests for a manifest that declares one bundle and depends on another.
// ──────────────────────────────────────────────────────────────────────────

fn srd_only_manifest() -> Manifest {
    let mut m = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        ..Manifest::default()
    };
    m.bundles.push(BundleRow {
        bundle_id: "srd".to_owned(),
        display_name: "srd".to_owned(),
        author: "t".to_owned(),
        version: "1".to_owned(),
        depends_on: vec!["core".to_owned()],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m
}

fn srd_channel_grant_manifest() -> Manifest {
    let mut m = srd_only_manifest();
    m.consequence_templates
        .push(frame_write_template("srd", "srd/action_surge/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "srd/action_surge/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyChannel": { "actor": "Actor", "channel": "core/action_class/primary" }
        }),
    });
    m
}

#[test]
fn class6_modify_channel_cross_bundle_trust_when_closure_incomplete_offline() {
    // sk-zqc RULED behavior: per-file validate of `srd` alone (`core`'s
    // `[[bundles]]` row absent) — the file is demonstrably correct and
    // nothing is being admitted anywhere at this moment, so this SHALL pass
    // with a hint, not Fatal.
    let diags = validate_manifest(&srd_channel_grant_manifest());
    assert!(
        !diags.iter().any(|d| d.severity == Severity::Fatal),
        "a per-file validate whose frame-write ref points outside the file must not Fatal; got {diags:?}"
    );
    let info = diags
        .iter()
        .find(|d| d.code == MF_VAL_FRAME_WRITE_CHANNEL_REF)
        .expect("expected an MF-VAL-312 Info diagnostic recording the trust");
    assert_eq!(info.severity, Severity::Info);
    assert!(
        info.hint.is_some(),
        "trusted cross-bundle channel ref must carry a hint; got {info:?}"
    );
}

#[test]
fn class6_modify_resource_cross_bundle_trust_when_closure_incomplete_offline() {
    let mut m = srd_only_manifest();
    m.consequence_templates
        .push(frame_write_template("srd", "srd/burn/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "srd/burn/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "-3" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        !diags.iter().any(|d| d.severity == Severity::Fatal),
        "a per-file validate whose frame-write ref points outside the file must not Fatal; got {diags:?}"
    );
    let info = diags
        .iter()
        .find(|d| d.code == MF_VAL_FRAME_WRITE_RESOURCE_REF)
        .expect("expected an MF-VAL-217 Info diagnostic recording the trust");
    assert_eq!(info.severity, Severity::Info);
    assert!(info.hint.is_some());
}

#[test]
fn class6_modify_channel_resolves_via_live_vocabulary() {
    // sk-pm7 RULED fix: the SAME closure-incomplete shape as the trust test
    // above, but the server's existence-only live index now sees the
    // already-published `core/action_class/primary` channel — it resolves
    // DETERMINISTICALLY (no diagnostic at all), the false-fatal this bead
    // reports is gone.
    let live = LiveVocabulary {
        economy_channels: BTreeMap::from([(
            "core/action_class/primary".to_owned(),
            "core".to_owned(),
        )]),
        modifier_templates: BTreeMap::new(),
        equipment_slots: BTreeMap::new(),
        roll_tables: BTreeMap::new(),
        actions: BTreeMap::new(),
        ..Default::default()
    };
    let diags =
        validate_manifest_with_world(&srd_channel_grant_manifest(), &BTreeSet::new(), Some(&live));
    assert!(
        !diags
            .iter()
            .any(|d| d.code == MF_VAL_FRAME_WRITE_CHANNEL_REF),
        "a live-published channel must resolve in-validator with no diagnostic at all; got {diags:?}"
    );
}

#[test]
fn class6_modify_channel_resolves_via_against_manifest_vocabulary() {
    // sk-vjh.2: `--against <bundle>` fills the same LiveVocabulary from disk.
    let mut against = Manifest {
        schema_version: SchemaVersion::new(1, 0, 0),
        ..Manifest::default()
    };
    against.bundles.push(BundleRow {
        bundle_id: "core".to_owned(),
        display_name: "core".to_owned(),
        author: "t".to_owned(),
        version: "1".to_owned(),
        depends_on: Vec::new(),
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    against
        .economy_channel_definitions
        .push(action_class_channel("core"));
    let live = LiveVocabulary::from_manifest(&against);
    let diags =
        validate_manifest_with_world(&srd_channel_grant_manifest(), &BTreeSet::new(), Some(&live));
    assert!(
        !diags
            .iter()
            .any(|d| d.code == MF_VAL_FRAME_WRITE_CHANNEL_REF),
        "--against a bundle that declares the channel must resolve with no MF-VAL-312; got {diags:?}"
    );
}

#[test]
fn class6_modify_channel_wrong_order_admission_stays_fatal() {
    // sk-pm7 RULING: admission into a live world stays STRICT. The SAME
    // closure-incomplete shape as the two tests above, but the live world
    // genuinely does not have the channel — `validate_manifest_with_world`
    // must NOT fall back to sk-zqc's offline leniency just because the
    // manifest's own `depends_on` closure looks incomplete: a real admission
    // The bundle still lacks the definition, even with an empty live index.
    let live = LiveVocabulary::default();
    let diags =
        validate_manifest_with_world(&srd_channel_grant_manifest(), &BTreeSet::new(), Some(&live));
    assert!(
        diags
            .iter()
            .any(|d| d.code == MF_VAL_FRAME_WRITE_CHANNEL_REF && d.severity == Severity::Fatal),
        "a genuinely-missing channel must stay Fatal at live admission even with an incomplete depends_on closure; got {diags:?}"
    );
}

#[test]
fn class11_equip_modifier_resolves_via_live_vocabulary() {
    // sk-pm7's sibling MF-VAL-310 fix: `entity_definition_equip_modifier.
    // modifier_template_id` against an already-published dependency's
    // `modifier_template`, via the SAME existence-only index.
    let mut m = srd_only_manifest();
    m.entity_definitions.push(EntityDefinitionRow {
        entity_def_id: "srd/regdar".to_owned(),
        bundle_id: "srd".to_owned(),
        display_name: "Regdar".to_owned(),
        lineage_id: String::new(),
        affordance_seed_id: String::new(),
        on_consume_consequence_id: String::new(),
        on_destroy_consequence_id: String::new(),
        on_equip_consequence_id: String::new(),
        on_unequip_consequence_id: String::new(),
        grants_capability_while_equipped: String::new(),
        grants_capability_while_present_in_location: String::new(),
        slot_compatibility_predicate: String::new(),
        stackable: false,
        description: String::new(),
    });
    m.entity_definition_equip_modifier
        .push(EntityDefinitionEquipModifierRow {
            entity_def_id: "srd/regdar".to_owned(),
            modifier_template_id: "core/modifier/strength".to_owned(),
            bundle_id: "srd".to_owned(),
        });
    let live = LiveVocabulary {
        economy_channels: BTreeMap::new(),
        modifier_templates: BTreeMap::from([(
            "core/modifier/strength".to_owned(),
            "core".to_owned(),
        )]),
        equipment_slots: BTreeMap::new(),
        roll_tables: BTreeMap::new(),
        actions: BTreeMap::new(),
        ..Default::default()
    };
    let diags = validate_manifest_with_world(&m, &BTreeSet::new(), Some(&live));
    assert!(
        !diags
            .iter()
            .any(|d| d.code == MF_VAL_DEF_SEED_MODIFIER_TEMPLATE_REF),
        "a live-published modifier_template must resolve in-validator; got {diags:?}"
    );
}

#[test]
fn self_reaction_budget_resource_resolves() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.action_definitions.push(self_reaction_action(
        "core",
        "core/legendary_resistance",
        "core/legendary_resistance/success",
    ));
    m.consequence_templates.push(frame_write_template(
        "core",
        "core/legendary_resistance/success",
    ));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/legendary_resistance/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "-1" }
        }),
    });

    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_SELF_REACTION_RESOURCE_REF).is_empty(),
        "expected no MF-VAL-263; got: {diags:?}"
    );
}

#[test]
fn self_reaction_channel_budget_resolves_without_resource_write() {
    let mut m = base_manifest();
    let mut action = self_reaction_action("core", "core/dodge", "core/dodge/success");
    action.spends_channel = Some("core/economy_channel/primary".to_owned());
    m.action_definitions.push(action);
    m.consequence_templates
        .push(frame_write_template("core", "core/dodge/success"));

    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_SELF_REACTION_RESOURCE_REF).is_empty(),
        "expected no MF-VAL-263 for a channel-budgeted action; got: {diags:?}"
    );
}

#[test]
fn self_reaction_budget_resource_nested_in_for_each_actor_in_radius_resolves() {
    // #1216: a self_eligible self-anchored AoE (e.g. srd/cast_acid_splash) whose
    // only ModifyResource lives inside the ForEachActorInRadius body must satisfy
    // MF-VAL-263. The budget walker recurses into the JSON array body, mirroring
    // the MF-VAL-217 walker (`class6_nested_inside_for_each_actor_in_radius_is_walked`).
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.action_definitions.push(self_reaction_action(
        "core",
        "core/acid_splash",
        "core/acid_splash/success",
    ));
    m.consequence_templates
        .push(frame_write_template("core", "core/acid_splash/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/acid_splash/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ForEachActorInRadius": {
                "center_ref": "ActorSpatialRef",
                "shape_id": "core/area/sphere",
                "shape_params_json": "{\"radius_m\": 5.0}",
                "predicate_source": "roll_under(d20, 13)",
                "body": [
                    { "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "0 - d6" } }
                ]
            }
        }),
    });

    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_SELF_REACTION_RESOURCE_REF).is_empty(),
        "expected no MF-VAL-263 (nested-in-AoE budget resolves); got: {diags:?}"
    );
}

#[test]
fn self_reaction_budget_resource_required() {
    let mut m = base_manifest();
    m.action_definitions.push(self_reaction_action(
        "core",
        "core/legendary_resistance",
        "core/legendary_resistance/success",
    ));
    m.consequence_templates.push(frame_write_template(
        "core",
        "core/legendary_resistance/success",
    ));

    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_SELF_REACTION_RESOURCE_REF);
    assert_eq!(
        codes.len(),
        1,
        "expected one MF-VAL-263 for missing budget write; got: {diags:?}"
    );
    assert!(codes[0].contains("has no ModifyResource budget resource"));
}

#[test]
fn self_reaction_budget_resource_must_be_in_scope() {
    let mut m = base_manifest();
    m.bundles.push(BundleRow {
        bundle_id: "other".to_owned(),
        display_name: "other".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m.resource_definitions.push(hp_resource("other"));
    m.action_definitions.push(self_reaction_action(
        "core",
        "core/legendary_resistance",
        "core/legendary_resistance/success",
    ));
    m.consequence_templates.push(frame_write_template(
        "core",
        "core/legendary_resistance/success",
    ));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/legendary_resistance/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "other/hp", "delta_source": "-1" }
        }),
    });

    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_SELF_REACTION_RESOURCE_REF);
    assert_eq!(
        codes.len(),
        1,
        "expected one MF-VAL-263 for out-of-scope budget; got: {diags:?}"
    );
    assert!(codes[0].contains("outside owning bundle"));
}

#[test]
fn self_reaction_driver_fixture_validates() {
    let manifest = repo_fixture(
        "openspec/changes/archive/2026-07-07-self-reaction-resolution-flip/fixtures/legendary-resistance.toml",
    );
    let diags = validate_manifest(&manifest);
    assert!(
        !diags.iter().any(|diag| diag.severity == Severity::Fatal),
        "Legendary Resistance fixture should validate; got: {diags:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Self-reaction budget — MF-VAL-263 must
// only gate self-eligible REACTIONS (reaction_eligibility/reaction_trigger
// wired into the §20 producer, ADR-0050), not every self_eligible action. A
// plain self-targeted action with neither axis set (e.g. the SRD's genuinely
// free "Dropping Prone") has no reaction window and no re-trigger loophole,
// so it must be authorable without a fabricated budget.
// ──────────────────────────────────────────────────────────────────────────

fn free_self_action(bundle: &str, action_id: &str, on_success: &str) -> ActionDefinitionRow {
    ActionDefinitionRow {
        action_id: action_id.to_owned(),
        bundle_id: bundle.to_owned(),
        required_capability: String::new(),
        on_success: on_success.to_owned(),
        on_failure: String::new(),
        resolution_mode: Default::default(),
        resolution_predicate_source: String::new(),
        resolution_scope: String::new(),
        non_steering: false,
        display_name: "Drop Prone".to_owned(),
        cost_time_ms: 0,
        session_def_id: String::new(),
        resumable: false,
        reaction_eligibility: None,
        reaction_window_ms: None,
        reaction_trigger: None,
        self_eligible: true,
        self_only: false,
        description: String::new(),
        spends_channel: None,
    }
}

/// RED-FIRST: before the Slice A fix, this shape (mirroring the exact
/// Dropping-Prone action — self_eligible, no `spends_channel`, no
/// `ModifyResource` budget write, applies a condition to self) failed
/// generation hard with MF-VAL-263 (finding #73). It must now validate.
#[test]
fn drop_prone_shape_free_self_action_validates() {
    let mut m = base_manifest();
    m.condition_definitions.push(ConditionDefinitionRow {
        condition_definition_id: "core/condition/prone".to_owned(),
        bundle_id: "core".to_owned(),
        display_name: "Prone".to_owned(),
        stacking_rule: "unique".to_owned(),
        remove_time_policy: "manual".to_owned(),
        gate_predicate_source: String::new(),
        modifier_template_id: String::new(),
        hooks: Vec::new(),
        observable: true,
        grants_capability_while_active: Vec::new(),
        suppresses_capability_while_active: Vec::new(),
        implies: Vec::new(),
        description: String::new(),
    });
    m.action_definitions.push(free_self_action(
        "core",
        "core/drop_prone",
        "core/drop_prone/success",
    ));
    m.consequence_templates
        .push(frame_write_template("core", "core/drop_prone/success"));
    // No ModifyResource anywhere in the on_success chain — just a self-target
    // condition apply, exactly the SRD's "without using an action or any of
    // your Speed" rule.
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/drop_prone/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ApplyStatus": {
                "target_kind": "entity",
                "target_ref": "Actor",
                "condition_definition_id": "core/condition/prone"
            }
        }),
    });

    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_SELF_REACTION_RESOURCE_REF).is_empty(),
        "a genuinely free self-targeted non-reaction action must not require \
         an MF-VAL-263 budget; got: {diags:?}"
    );
}
/// A self-eligible reaction without a finite budget must fail validation.
#[test]
fn self_eligible_action_with_reaction_eligibility_but_no_reaction_trigger_still_requires_budget() {
    let mut m = base_manifest();
    let mut action = self_reaction_action("core", "core/half_wired", "core/half_wired/success");
    action.reaction_trigger = None; // only reaction_eligibility set
    m.action_definitions.push(action);
    m.consequence_templates
        .push(frame_write_template("core", "core/half_wired/success"));

    let diags = validate_manifest(&m);
    let codes = codes_for(&diags, MF_VAL_SELF_REACTION_RESOURCE_REF);
    assert_eq!(
        codes.len(),
        1,
        "a self-eligible action with EITHER reaction axis set must still \
         require a budget; got: {diags:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// ADR-0045 COND-4 — frame_write DSL admission: delta_source (Int) +
// predicate_source (Bool) are parsed + type-checked at manifest admission.
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn adr0045_delta_source_dice_expression_admits() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/mm/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/mm/success".to_owned(),
        ordinal: 0,
        // Magic Missile: -(1d4) - 1, a rolled Int delta.
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "-d4 - 1" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_FRAME_WRITE_DSL_PARSE).is_empty()
            && codes_for(&diags, MF_VAL_FRAME_WRITE_DSL_TYPE).is_empty(),
        "a valid dice delta_source must admit; got: {diags:?}"
    );
}

#[test]
fn adr0045_delta_source_bool_where_int_is_fatal() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/bad/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/bad/success".to_owned(),
        ordinal: 0,
        // `true` parses, but its root is Bool where an Int delta is required.
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "true" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == MF_VAL_FRAME_WRITE_DSL_TYPE && d.severity == Severity::Fatal),
        "Bool-where-Int delta_source must be a fatal MF-VAL-253; got: {diags:?}"
    );
}

#[test]
fn adr0045_delta_source_parse_error_is_fatal() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/parse/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/parse/success".to_owned(),
        ordinal: 0,
        // Unbalanced parens — a parse failure, not a runtime-only Err.
        frame_write: serde_json::json!({
            "ModifyResource": { "actor": "Actor", "resource": "core/hp", "delta_source": "roll('1d6'" }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == MF_VAL_FRAME_WRITE_DSL_PARSE && d.severity == Severity::Fatal),
        "a parse error in delta_source must be a fatal MF-VAL-252; got: {diags:?}"
    );
}

#[test]
fn adr0045_predicate_source_save_admits() {
    let mut m = base_manifest();
    m.consequence_templates
        .push(frame_write_template("core", "core/aoe2/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/aoe2/success".to_owned(),
        ordinal: 0,
        // A DEX save: d20 + dex modifier ≥ DC. A valid per-actor Bool predicate.
        frame_write: serde_json::json!({
            "ForEachActorInRadius": {
                "center_ref": "ActorSpatialRef",
                "shape_id": "core/area/node_ball",
                "shape_params_json": "{\"ball_radius\": 0}",
                "predicate_source": "d20 + actor.stat('dex') >= 13",
                "body": []
            }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_FRAME_WRITE_DSL_PARSE).is_empty()
            && codes_for(&diags, MF_VAL_FRAME_WRITE_DSL_TYPE).is_empty(),
        "a valid save predicate_source must admit; got: {diags:?}"
    );
}

#[test]
fn adr0045_predicate_source_parse_error_is_fatal() {
    let mut m = base_manifest();
    m.consequence_templates
        .push(frame_write_template("core", "core/aoe3/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/aoe3/success".to_owned(),
        ordinal: 0,
        // A bare Int is not a predicate (no comparison) — a parse failure.
        frame_write: serde_json::json!({
            "ForEachActorInRadius": {
                "center_ref": "ActorSpatialRef",
                "shape_id": "core/area/node_ball",
                "shape_params_json": "{\"ball_radius\": 0}",
                "predicate_source": "d20",
                "body": []
            }
        }),
    });
    let diags = validate_manifest(&m);
    assert!(
        diags
            .iter()
            .any(|d| d.code == MF_VAL_FRAME_WRITE_DSL_PARSE && d.severity == Severity::Fatal),
        "a non-predicate predicate_source must be a fatal MF-VAL-252; got: {diags:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// issue #209 traps — manipulation_action validator coverage
//   trap #1 alternative: require template-id action_definition row
//   trap #2: outcome tags key/value/conflict lint + action_definition FK
//   trap #3: referenced action_definition.on_success must be non-empty
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn issue209_trap1_missing_tag_resolved_action_definition_is_fatal() {
    // MF-VAL-223 checks the typed `manipulation_action_id` against
    // `action_definitions`, not the affordance's `template_id`. The
    // pre-fix self-compare (refs.rs before commit 54515f24)
    // compared affordance.template_id against an action_id-keyed index
    // and consequently fired for the wrong scenario. This test removes
    // the action_definition whose `action_id` matches the typed binding —
    // the affordance still has its template_id-named action_definition
    // row, but the descriptor resolves to a missing one. That is the load-bearing
    // FK that MF-VAL-223 protects.
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/manipulate".to_owned();
    m.action_definitions
        .retain(|row| row.action_id != "core/manipulate");

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_MANIP_ACTION_TEMPLATE_ACTION_REF);
    assert_eq!(
        msgs.len(),
        1,
        "expected one MF-VAL-223 for typed manipulation_action_id missing from action_definitions; got: {diags:?}"
    );
    assert!(
        msgs[0].contains("core/manipulate/affordance"),
        "diagnostic must name the affordance template_id; got: {}",
        msgs[0]
    );
    assert!(
        msgs[0].contains("core/manipulate"),
        "diagnostic must name the unresolved action_id; got: {}",
        msgs[0]
    );
}

#[test]
fn issue209_trap2_unknown_action_id_value_is_fatal() {
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/typoed_action".to_owned();
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_MANIP_ACTION_ACTION_REF);
    assert_eq!(
        msgs.len(),
        1,
        "expected one MF-VAL-221 for dangling action_id; got: {diags:?}"
    );
    assert!(msgs[0].contains("core/typoed_action"));
}

#[test]
fn typed_manipulation_action_id_value_is_validated() {
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/typoed_action".to_owned();

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_MANIP_ACTION_ACTION_REF);
    assert_eq!(
        msgs.len(),
        1,
        "expected typed manipulation_action_id FK diagnostic; got: {diags:?}"
    );
    assert!(msgs[0].contains("core/typoed_action"));
    assert_eq!(
        diags
            .iter()
            .find(|diag| diag.code == MF_VAL_MANIP_ACTION_ACTION_REF)
            .map(|diag| diag.path.as_str()),
        Some("outcome_templates.manipulation_action_id")
    );
}

#[test]
fn deprecated_manipulation_descriptor_tag_is_fatal() {
    let m = manipulation_manifest("manipulation.action_id=core/manipulate");
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG);
    assert_eq!(
        msgs.len(),
        1,
        "expected one deprecated descriptor diagnostic; got: {diags:?}"
    );
    assert!(msgs[0].contains("manipulation_action_id"));
}

#[test]
fn issue209_trap2_typoed_key_warns_and_missing_binding_fails_closed() {
    let m = manipulation_manifest("manipuilation.action_id=core/manipulate");
    let diags = validate_manifest(&m);

    let warns = codes_for(&diags, MF_VAL_MANIP_ACTION_TAG_TYPO);
    assert_eq!(
        warns.len(),
        1,
        "expected one MF-VAL-224 typo warning; got: {diags:?}"
    );
    assert!(warns[0].contains("manipuilation.action_id"));

    let fatals = codes_for(&diags, MF_VAL_MANIP_ACTION_TAG_SHAPE);
    assert!(
        !fatals.is_empty(),
        "expected MF-VAL-220 fail-closed missing-binding error; got: {diags:?}"
    );
}

#[test]
fn issue209_trap2_conflicting_values_for_same_key_are_fatal() {
    let m = manipulation_manifest("action_id=core/manipulate,action_id=core/other_action");
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG);
    assert_eq!(
        msgs.len(),
        1,
        "expected deprecated descriptor diagnostic for legacy action_id tag; got: {diags:?}"
    );
    assert!(msgs[0].contains("manipulation_action_id"));
}

#[test]
fn communication_mode_tag_is_deprecated_descriptor_fatal() {
    let mut m = manipulation_manifest("communication.mode=deterministic");
    m.outcome_templates[0].communication_speech_act_id = "core/assert".to_owned();

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG);
    assert_eq!(
        msgs.len(),
        1,
        "expected deprecated descriptor diagnostic for communication.mode tag; got: {diags:?}"
    );
    assert!(msgs[0].contains("communication.mode"));
    assert!(msgs[0].contains("action_definitions.resolution_mode"));
}

#[test]
fn communication_action_missing_speech_act_descriptor_warns() {
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/manipulate".to_owned();
    m.action_definitions
        .iter_mut()
        .find(|row| row.action_id == "core/manipulate")
        .expect("base manipulation action exists")
        .required_capability = "core/communicate".to_owned();

    let diags = validate_manifest(&m);
    let warnings: Vec<_> = diags
        .iter()
        .filter(|d| d.code == MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR)
        .collect();
    assert_eq!(
        warnings.len(),
        1,
        "expected one missing communication_speech_act_id warning; got: {diags:?}"
    );
    assert!(matches!(warnings[0].severity, Severity::Warning));
    assert!(warnings[0].message.contains("communication_speech_act_id"));
}

#[test]
fn communication_affordance_missing_speech_act_descriptor_warns_without_action_binding() {
    let mut m = manipulation_manifest("");
    m.affordance_templates[0].capability_id = "core/communicate".to_owned();

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR);
    assert_eq!(
        msgs.len(),
        1,
        "expected communication affordance to warn without action binding; got: {diags:?}"
    );
    assert!(msgs[0].contains("communication_speech_act_id"));
}

#[test]
fn communication_scoped_descriptor_warning_deduplicates_per_outcome() {
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/manipulate".to_owned();
    m.affordance_templates[0].capability_id = "core/communicate".to_owned();
    m.action_definitions
        .iter_mut()
        .find(|row| row.action_id == "core/manipulate")
        .expect("base manipulation action exists")
        .required_capability = "core/communicate".to_owned();

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR);
    assert_eq!(
        msgs.len(),
        1,
        "expected one deduplicated communication descriptor warning; got: {diags:?}"
    );
}

#[test]
fn non_communication_action_missing_speech_act_descriptor_is_quiet() {
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/manipulate".to_owned();

    let diags = validate_manifest(&m);
    assert!(
        diags
            .iter()
            .all(|d| d.code != MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR),
        "non-communication action should not emit #646 warning; got: {diags:?}"
    );
}

#[test]
fn communication_action_tag_missing_speech_act_descriptor_warns() {
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/manipulate".to_owned();
    m.action_definition_tag.push(ActionDefinitionTagRow {
        action_id: "core/manipulate".to_owned(),
        tag: "communication".to_owned(),
        bundle_id: "core".to_owned(),
        source: "manifest".to_owned(),
    });

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR);
    assert_eq!(
        msgs.len(),
        1,
        "expected action tag to mark communication scope; got: {diags:?}"
    );
}

// --- C13 / MF-VAL-229: sense-descriptor nudge (the sibling of #646/MF-VAL-228).
// A perception-scoped affordance whose outcome omits the typed `sense_query_kind`
// validates + applies but r_sense skips it (empty query id) -> silently
// unperceivable. Detection is tag-driven (`perception`), not core-id-keyed.

#[test]
fn perception_scoped_affordance_missing_sense_query_kind_warns() {
    let mut m = manipulation_manifest("");
    // Mark the affordance's niche as perception-scoped (core vocab shape).
    m.niche_definition_tag.push(NicheDefinitionTagRow {
        niche_id: "core/manipulate".to_owned(),
        tag: "perception".to_owned(),
        bundle_id: "core".to_owned(),
        source: "manifest".to_owned(),
    });

    let diags = validate_manifest(&m);
    let warnings: Vec<_> = diags
        .iter()
        .filter(|d| d.code == MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR)
        .collect();
    assert_eq!(
        warnings.len(),
        1,
        "expected one missing sense_query_kind warning; got: {diags:?}"
    );
    assert!(matches!(warnings[0].severity, Severity::Warning));
    assert!(warnings[0].message.contains("sense_query_kind"));
}

#[test]
fn perception_scoped_affordance_with_sense_query_kind_is_quiet() {
    let mut m = manipulation_manifest("");
    m.niche_definition_tag.push(NicheDefinitionTagRow {
        niche_id: "core/manipulate".to_owned(),
        tag: "perception".to_owned(),
        bundle_id: "core".to_owned(),
        source: "manifest".to_owned(),
    });
    // The authored sense outcome carries the typed query kind -> r_sense selects it.
    m.outcome_templates[0].sense_query_kind = "look".to_owned();

    let diags = validate_manifest(&m);
    assert!(
        diags
            .iter()
            .all(|d| d.code != MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR),
        "a perception affordance WITH sense_query_kind must not warn; got: {diags:?}"
    );
}

#[test]
fn non_perception_affordance_missing_sense_query_kind_is_quiet() {
    // The default manipulate affordance is not perception-scoped (no `perception`
    // tag) -> MF-VAL-229 must stay silent so every manipulate outcome is not warned.
    let m = manipulation_manifest("");

    let diags = validate_manifest(&m);
    assert!(
        diags
            .iter()
            .all(|d| d.code != MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR),
        "non-perception affordance should not emit the sense nudge; got: {diags:?}"
    );
}

#[test]
fn modder_perception_tag_marks_sense_scope_warns() {
    // Modder-neutrality (canon-warden ruling): a NON-core capability tagged
    // `perception` triggers the nudge -> detection is by tag, not by `core/sense`.
    let mut m = manipulation_manifest("");
    m.affordance_templates[0].capability_id = "mybundle/perceive".to_owned();
    m.capability_definition_tag
        .push(CapabilityDefinitionTagRow {
            capability_id: "mybundle/perceive".to_owned(),
            tag: "perception".to_owned(),
            bundle_id: "mybundle".to_owned(),
            source: "manifest".to_owned(),
        });

    let diags = validate_manifest(&m);
    let warnings = codes_for(&diags, MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR);
    assert_eq!(
        warnings.len(),
        1,
        "a modder-authored perception capability must mark sense scope; got: {diags:?}"
    );
}

#[test]
fn issue209_trap3_empty_on_success_for_dispatchable_action_is_fatal() {
    let mut m = manipulation_manifest("");
    m.outcome_templates[0].manipulation_action_id = "core/manipulate".to_owned();
    let action = m
        .action_definitions
        .iter_mut()
        .find(|row| row.action_id == "core/manipulate")
        .expect("base manipulation manifest should author core/manipulate action");
    action.on_success.clear();

    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_MANIP_ACTION_ON_SUCCESS);
    assert_eq!(
        msgs.len(),
        1,
        "expected one MF-VAL-222 for empty on_success; got: {diags:?}"
    );
    assert!(msgs[0].contains("core/manipulate"));
    assert!(msgs[0].contains("no-op consequence_template"));
}

// ──────────────────────────────────────────────────────────────────────────
// Class 8 — condition_definition.implies
//   Surfaced by SRD-probe Direction R (Phase 24) as finding R-A.
// ──────────────────────────────────────────────────────────────────────────

fn condition_def(id: &str, bundle: &str, implies: &[&str]) -> ConditionDefinitionRow {
    ConditionDefinitionRow {
        condition_definition_id: id.to_owned(),
        bundle_id: bundle.to_owned(),
        display_name: id.to_owned(),
        stacking_rule: "core/stacking/single".to_owned(),
        remove_time_policy: "core/remove_policy/independent_lifetime".to_owned(),
        gate_predicate_source: String::new(),
        modifier_template_id: String::new(),
        hooks: vec![],
        observable: true,
        grants_capability_while_active: Vec::new(),
        suppresses_capability_while_active: Vec::new(),
        implies: implies.iter().map(|s| (*s).to_owned()).collect(),
        description: String::new(),
    }
}

#[test]
fn class8_implies_resolved_same_bundle_admits_clean() {
    let mut m = base_manifest();
    m.condition_definitions
        .push(condition_def("core/incapacitated", "core", &[]));
    m.condition_definitions.push(condition_def(
        "core/paralyzed",
        "core",
        &["core/incapacitated"],
    ));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_CONDITION_IMPLIES_REF).is_empty(),
        "expected no MF-VAL-226; got: {diags:?}"
    );
}

#[test]
fn class8_implies_resolved_via_core_dependency_admits_clean() {
    // SRD-canonical shape: srd/paralyzed implies core/condition/incapacitated.
    // core is in every bundle's implicit scope.
    let mut m = base_manifest();
    m.bundles.push(BundleRow {
        bundle_id: "srd".to_owned(),
        display_name: "srd".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec!["core".to_owned()],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m.condition_definitions
        .push(condition_def("core/condition/incapacitated", "core", &[]));
    m.condition_definitions.push(condition_def(
        "srd/paralyzed",
        "srd",
        &["core/condition/incapacitated"],
    ));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_CONDITION_IMPLIES_REF).is_empty(),
        "expected no MF-VAL-226; got: {diags:?}"
    );
    assert!(
        codes_for(&diags, MF_VAL_CROSS_BUNDLE_SCOPE).is_empty(),
        "expected no MF-VAL-214 (core is in implicit scope); got: {diags:?}"
    );
}

#[test]
fn class8_implies_unresolved_id_is_fatal() {
    // The R-A reproduction: a bogus implied id passes static lint today
    // and only fails at apply time inside apply_implied_cascade.
    let mut m = base_manifest();
    m.condition_definitions.push(condition_def(
        "srd/paralyzed",
        "core",
        &["core/condition/nonexistent"],
    ));
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_CONDITION_IMPLIES_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-226; got: {diags:?}");
    assert!(msgs[0].contains("srd/paralyzed"));
    assert!(msgs[0].contains("core/condition/nonexistent"));
    let fatal = diags
        .iter()
        .find(|d| d.code == MF_VAL_CONDITION_IMPLIES_REF)
        .expect("MF-VAL-226 must be emitted");
    assert!(matches!(fatal.severity, Severity::Fatal));
}

// ──────────────────────────────────────────────────────────────────────────
// Class 14 — condition_definition capability grants / suppressions (sk-syo)
//   Surfaced by the canon-warden pass on sk-syo: both fields were enforced at
//   fold time but unchecked statically, so a typo'd capability id validated
//   and seeded cleanly and then silently matched nothing — the same
//   "validates clean, does nothing" failure the bead exists to kill.
// ──────────────────────────────────────────────────────────────────────────

fn condition_with_capabilities(
    id: &str,
    bundle: &str,
    grants: &[&str],
    suppresses: &[&str],
) -> ConditionDefinitionRow {
    let mut row = condition_def(id, bundle, &[]);
    row.grants_capability_while_active = grants.iter().map(|s| (*s).to_owned()).collect();
    row.suppresses_capability_while_active = suppresses.iter().map(|s| (*s).to_owned()).collect();
    row
}

#[test]
fn class14_resolved_capability_refs_admit_clean() {
    let mut m = base_manifest();
    m.capability_definitions.push(cap_def("core/speak", "core"));
    m.capability_definitions
        .push(cap_def("core/fireball", "core"));
    m.condition_definitions.push(condition_with_capabilities(
        "core/condition/incapacitated",
        "core",
        &["core/fireball"],
        &["core/speak"],
    ));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_CONDITION_CAPABILITY_REF).is_empty(),
        "expected no MF-VAL-316; got: {diags:?}"
    );
}

#[test]
fn class14_empty_lists_are_the_ungated_default_and_admit_clean() {
    let mut m = base_manifest();
    m.condition_definitions
        .push(condition_with_capabilities("core/plain", "core", &[], &[]));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_CONDITION_CAPABILITY_REF).is_empty(),
        "every condition ships both lists empty; that must stay silent: {diags:?}"
    );
}

#[test]
fn class14_unresolved_suppression_id_is_fatal() {
    let mut m = base_manifest();
    m.condition_definitions.push(condition_with_capabilities(
        "core/condition/incapacitated",
        "core",
        &[],
        &["core/speeek"],
    ));
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_CONDITION_CAPABILITY_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-316; got: {diags:?}");
    assert!(msgs[0].contains("core/condition/incapacitated"));
    assert!(msgs[0].contains("core/speeek"));
    assert!(
        msgs[0].contains("suppresses_capability_while_active"),
        "the message must name WHICH of the two lists is wrong: {msgs:?}"
    );
    let fatal = diags
        .iter()
        .find(|d| d.code == MF_VAL_CONDITION_CAPABILITY_REF)
        .expect("MF-VAL-316 must be emitted");
    assert!(matches!(fatal.severity, Severity::Fatal));
}

#[test]
fn class14_unresolved_grant_id_is_fatal_and_named_separately() {
    let mut m = base_manifest();
    m.condition_definitions.push(condition_with_capabilities(
        "core/ring/attuned",
        "core",
        &["core/fireballl"],
        &[],
    ));
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_CONDITION_CAPABILITY_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-316; got: {diags:?}");
    assert!(msgs[0].contains("grants_capability_while_active"));
}

#[test]
fn class14_both_lists_are_walked_not_just_the_first() {
    // Guards the loop actually covering BOTH fields — a check that silently
    // walked only `grants` would still pass every test above except this one.
    let mut m = base_manifest();
    m.condition_definitions.push(condition_with_capabilities(
        "core/broken",
        "core",
        &["core/typo_a"],
        &["core/typo_b"],
    ));
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_CONDITION_CAPABILITY_REF);
    assert_eq!(msgs.len(), 2, "expected one per list; got: {diags:?}");
}

#[test]
fn class8_implies_multiple_unresolved_emit_one_diagnostic_each() {
    let mut m = base_manifest();
    m.condition_definitions
        .push(condition_def("core/known", "core", &[]));
    m.condition_definitions.push(condition_def(
        "core/exhaustion_3",
        "core",
        &["core/known", "core/typo_a", "core/typo_b"],
    ));
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_CONDITION_IMPLIES_REF);
    assert_eq!(msgs.len(), 2, "expected two diagnostics; got: {diags:?}");
}

#[test]
fn class8_implies_across_bundle_without_depends_on_is_cross_bundle_scope_violation() {
    let mut m = base_manifest();
    m.bundles.push(BundleRow {
        bundle_id: "bundle_a".to_owned(),
        display_name: "a".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m.bundles.push(BundleRow {
        bundle_id: "bundle_b".to_owned(),
        display_name: "b".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec![],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m.condition_definitions
        .push(condition_def("bundle_b/foo", "bundle_b", &[]));
    m.condition_definitions
        .push(condition_def("bundle_a/bar", "bundle_a", &["bundle_b/foo"]));
    let diags = validate_manifest(&m);
    // Resolution succeeds (the id is authored), but cross-bundle scope fails.
    assert!(
        codes_for(&diags, MF_VAL_CONDITION_IMPLIES_REF).is_empty(),
        "expected no MF-VAL-226 when the id resolves; got: {diags:?}"
    );
    let scope_msgs = codes_for(&diags, MF_VAL_CROSS_BUNDLE_SCOPE);
    assert!(
        scope_msgs
            .iter()
            .any(|m| m.contains("bundle_a/bar") && m.contains("bundle_b")),
        "expected MF-VAL-214 mentioning bundle_a/bar implying into bundle_b; got: {diags:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Class 9 — entity_tag.entity_key resolves to an authored instance (CR-4/CR-7)
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn entity_tag_unknown_entity_key_is_fatal() {
    let mut m = base_manifest();
    m.entities.push(EntityRow {
        entity_key: "core/known".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: String::new(),
        name: "Known".to_owned(),
        kind: EntityKindRow::Npc,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    // A tag on the authored entity validates clean…
    m.entity_tag.push(EntityTagRow {
        entity_key: "core/known".to_owned(),
        tag: "demo.party".to_owned(),
        bundle_id: "core".to_owned(),
        source: "manifest".to_owned(),
    });
    // …but a tag pointed at a key no authored entity carries is Fatal.
    m.entity_tag.push(EntityTagRow {
        entity_key: "core/ghost".to_owned(),
        tag: "spooky".to_owned(),
        bundle_id: "core".to_owned(),
        source: "manifest".to_owned(),
    });
    let diags = validate_manifest(&m);
    let hits = codes_for(&diags, MF_VAL_ENTITY_TAG_ENTITY_REF);
    assert_eq!(
        hits.len(),
        1,
        "exactly the unknown key is rejected: {hits:?}"
    );
    assert!(hits[0].contains("core/ghost"));
    assert!(!hits.iter().any(|msg| msg.contains("core/known")));
}

// ──────────────────────────────────────────────────────────────────────────
// Class 10 — place_edge_affordance knows_entity binding resolves (CR-18 / V2)
// ──────────────────────────────────────────────────────────────────────────

/// An affordance template whose predicate gates on `$bindings.castle` via the
/// given op source. Defaults to a `knows_entity` gate (the entity-ref case).
fn binding_gated_template(template_id: &str, predicate_source: &str) -> AffordanceTemplateRow {
    AffordanceTemplateRow {
        template_id: template_id.to_owned(),
        bundle_id: "core".to_owned(),
        niche_id: "core/passable_portal".to_owned(),
        capability_id: "core/walk".to_owned(),
        display_name: "March".to_owned(),
        predicate_bool: false,
        predicate_source: predicate_source.to_owned(),
        accepts_global_storm: false,
        locality_class: String::new(),
        reads_occupancy: false,
        description: String::new(),
        stratification_ok: true,
        outcome_template_id: "core/move_outcome".to_owned(),
        range_m: 0.0,
        spends_channel: None,
        recharges_on: None,
        composes: None,

        movement_cost_expression_source: String::new(),
    }
}

#[test]
fn knows_entity_binding_unknown_key_is_fatal() {
    let mut m = base_manifest();
    m.entities.push(EntityRow {
        entity_key: "core/castle".to_owned(),
        bundle_id: "core".to_owned(),
        entity_def_id: String::new(),
        name: "Castle".to_owned(),
        kind: EntityKindRow::Place,
        description: String::new(),
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
    });
    m.affordance_templates.push(binding_gated_template(
        "core/march",
        "actor.knows_entity($bindings.castle)",
    ));
    // Binding the gate to the authored entity validates clean…
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/known".to_owned(),
        affordance_template_id: "core/march".to_owned(),
        bindings_json: r#"{"castle":"core/castle"}"#.to_owned(),

        mode_id: String::new(),
    });
    // …but a binding naming a key no authored entity carries is Fatal — the V2
    // wall: it would apply clean and leave the gate silently false forever.
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/ghost".to_owned(),
        affordance_template_id: "core/march".to_owned(),
        bindings_json: r#"{"castle":"core/nowhere"}"#.to_owned(),

        mode_id: String::new(),
    });
    let diags = validate_manifest(&m);
    let hits = codes_for(&diags, MF_VAL_AFFORDANCE_KNOWS_ENTITY_BINDING_REF);
    assert_eq!(
        hits.len(),
        1,
        "exactly the unresolvable key is rejected: {hits:?}"
    );
    assert!(hits[0].contains("core/nowhere"));
    assert!(!hits.iter().any(|msg| msg.contains("core/castle")));
}

#[test]
fn knows_entity_binding_literal_numeric_id_passes() {
    let mut m = base_manifest();
    m.affordance_templates.push(binding_gated_template(
        "core/march",
        "actor.knows_entity($bindings.castle)",
    ));
    // A literal numeric entity id is an explicit authored choice, not a key —
    // the check passes it through (apply does too).
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/literal".to_owned(),
        affordance_template_id: "core/march".to_owned(),
        bindings_json: r#"{"castle":"40"}"#.to_owned(),

        mode_id: String::new(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_AFFORDANCE_KNOWS_ENTITY_BINDING_REF).is_empty(),
        "a literal numeric entity id passes the knowledge-binding check; got: {diags:?}"
    );
}

#[test]
fn has_item_binding_is_not_treated_as_entity_ref() {
    let mut m = base_manifest();
    // has_item's binding value is a def-id string compared as-is — NOT an entity
    // key. The predicate-aware classifier must leave it untouched, never flag it
    // as an unresolvable entity (the warden's predicate-awareness requirement).
    m.affordance_templates.push(binding_gated_template(
        "core/locked",
        "actor.has_item($bindings.required_key)",
    ));
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/locked".to_owned(),
        affordance_template_id: "core/locked".to_owned(),
        bindings_json: r#"{"required_key":"core/item/cellar_key"}"#.to_owned(),

        mode_id: String::new(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_AFFORDANCE_KNOWS_ENTITY_BINDING_REF).is_empty(),
        "has_item bindings carry def-id strings, not entity keys — never flagged: {diags:?}"
    );
}

#[test]
fn structured_binding_subject_missing_from_bindings_json_is_fatal() {
    let mut m = base_manifest();
    m.affordance_templates.push(binding_gated_template(
        "core/group_escort",
        r#"bindings.party.stat("phandelver/escort_stage") >= 2"#,
    ));
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/group".to_owned(),
        affordance_template_id: "core/group_escort".to_owned(),
        bindings_json: r#"{"wrong_key":"phandelver/party"}"#.to_owned(),

        mode_id: String::new(),
    });
    let diags = validate_manifest(&m);
    let hits = codes_for(&diags, MF_VAL_AFFORDANCE_BINDING_COVERAGE);
    assert_eq!(
        hits.len(),
        1,
        "missing structured binding subject should be fatal: {diags:?}"
    );
    assert!(hits[0].contains("party"));
}

#[test]
fn structured_binding_subject_covered_by_bindings_json_passes() {
    let mut m = base_manifest();
    m.affordance_templates.push(binding_gated_template(
        "core/group_escort",
        r#"bindings.party.stat("phandelver/escort_stage") >= 2"#,
    ));
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/group".to_owned(),
        affordance_template_id: "core/group_escort".to_owned(),
        bindings_json: r#"{"party":"phandelver/party"}"#.to_owned(),

        mode_id: String::new(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_AFFORDANCE_BINDING_COVERAGE).is_empty(),
        "covered structured binding subject should pass coverage: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// Class 10b — movement defaults + modes (ADR-0101 D-1/D-5, authored-rows-only)
// ---------------------------------------------------------------------------

fn movement_default_row(
    bundle_id: &str,
    place_graph_id: &str,
    mode_id: &str,
    affordance_template_id: &str,
    bindings_json: &str,
) -> MovementDefaultRow {
    MovementDefaultRow {
        bundle_id: bundle_id.to_owned(),
        place_graph_id: place_graph_id.to_owned(),
        mode_id: mode_id.to_owned(),
        affordance_template_id: affordance_template_id.to_owned(),
        bindings_json: bindings_json.to_owned(),
    }
}

#[test]
fn movement_default_missing_binding_is_fatal() {
    let mut m = base_manifest();
    m.affordance_templates.push(binding_gated_template(
        "core/locked",
        "actor.has_item($bindings.required_key)",
    ));
    m.movement_defaults
        .push(movement_default_row("core", "", "", "core/locked", "{}"));
    let diags = validate_manifest(&m);
    let hits = codes_for(&diags, MF_VAL_MOVEMENT_DEFAULT_BINDING_COVERAGE);
    assert_eq!(hits.len(), 1, "missing binding should be fatal: {diags:?}");
    assert!(hits[0].contains("required_key"));
}

#[test]
fn movement_default_covered_binding_passes() {
    let mut m = base_manifest();
    m.affordance_templates.push(binding_gated_template(
        "core/locked",
        "actor.has_item($bindings.required_key)",
    ));
    m.movement_defaults.push(movement_default_row(
        "core",
        "",
        "",
        "core/locked",
        r#"{"required_key":"core/item/cellar_key"}"#,
    ));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_MOVEMENT_DEFAULT_BINDING_COVERAGE).is_empty(),
        "covered binding should pass: {diags:?}"
    );
}

#[test]
fn two_place_edge_affordance_rows_in_one_mode_sharing_a_key_is_fatal() {
    let mut m = base_manifest();
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/ford".to_owned(),
        affordance_template_id: "core/affordance/step".to_owned(),
        bindings_json: r#"{"movement_reach_resource":"core/speed-walk"}"#.to_owned(),
        mode_id: String::new(),
    });
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/ford".to_owned(),
        affordance_template_id: "core/affordance/swim".to_owned(),
        bindings_json: r#"{"movement_reach_resource":"core/speed-swim"}"#.to_owned(),
        mode_id: String::new(),
    });
    let diags = validate_manifest(&m);
    let hits = codes_for(&diags, MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT);
    assert_eq!(hits.len(), 1, "shared key in one mode is fatal: {diags:?}");
    assert!(hits[0].contains("movement_reach_resource"));
}

#[test]
fn two_place_edge_affordance_rows_in_different_modes_sharing_a_key_is_fine() {
    let mut m = base_manifest();
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/ford".to_owned(),
        affordance_template_id: "core/affordance/step".to_owned(),
        bindings_json: r#"{"movement_reach_resource":"core/speed-walk"}"#.to_owned(),
        mode_id: String::new(),
    });
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/ford".to_owned(),
        affordance_template_id: "core/affordance/swim".to_owned(),
        bindings_json: r#"{"movement_reach_resource":"core/speed-swim"}"#.to_owned(),
        mode_id: "swim".to_owned(),
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT).is_empty(),
        "different modes never conflict: {diags:?}"
    );
}

#[test]
fn two_movement_default_rows_in_one_scope_sharing_a_key_is_fatal() {
    let mut m = base_manifest();
    m.movement_defaults.push(movement_default_row(
        "core",
        "",
        "",
        "core/affordance/step",
        r#"{"movement_reach_resource":"core/speed-walk"}"#,
    ));
    m.movement_defaults.push(movement_default_row(
        "core",
        "",
        "",
        "core/affordance/swim",
        r#"{"movement_reach_resource":"core/speed-swim"}"#,
    ));
    let diags = validate_manifest(&m);
    let hits = codes_for(&diags, MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT);
    assert_eq!(
        hits.len(),
        1,
        "shared key in one default scope is fatal: {diags:?}"
    );
}

#[test]
fn same_template_on_one_edge_under_two_modes_is_a_warning() {
    let mut m = base_manifest();
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/ford".to_owned(),
        affordance_template_id: "core/affordance/step".to_owned(),
        bindings_json: "{}".to_owned(),
        mode_id: String::new(),
    });
    m.place_edge_affordance.push(PlaceEdgeAffordanceRow {
        edge_key: "core/edge/ford".to_owned(),
        affordance_template_id: "core/affordance/step".to_owned(),
        bindings_json: "{}".to_owned(),
        mode_id: "wade".to_owned(),
    });
    let diags = validate_manifest(&m);
    let hits: Vec<_> = diags
        .iter()
        .filter(|d| d.code == MF_VAL_MOVEMENT_MODE_TEMPLATE_REPEATED)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "same template in two modes warns once: {diags:?}"
    );
    assert_eq!(hits[0].severity, Severity::Warning);
}

// ---------------------------------------------------------------------------
// Class 11 — definition-level instance seeds (#728 / #729 + condition fossil)
// ---------------------------------------------------------------------------

fn entity_def(id: &str, bundle: &str) -> EntityDefinitionRow {
    EntityDefinitionRow {
        entity_def_id: id.to_owned(),
        bundle_id: bundle.to_owned(),
        display_name: id.to_owned(),
        lineage_id: String::new(),
        affordance_seed_id: String::new(),
        on_consume_consequence_id: String::new(),
        on_destroy_consequence_id: String::new(),
        on_equip_consequence_id: String::new(),
        on_unequip_consequence_id: String::new(),
        grants_capability_while_equipped: String::new(),
        grants_capability_while_present_in_location: String::new(),
        slot_compatibility_predicate: String::new(),
        stackable: false,
        description: String::new(),
    }
}

fn seed_state_manifest() -> Manifest {
    let mut m = base_manifest();
    m.entity_definitions.push(entity_def("core/ghoul", "core"));
    m.object_state_definitions.push(ObjectStateDefinitionRow {
        state_id: "core/hp".to_owned(),
        bundle_id: "core".to_owned(),
        target_kind: "entity".to_owned(),
        display_name: "HP".to_owned(),
        default_visibility: "Public".to_owned(),
        description: String::new(),
    });
    m
}

#[test]
fn class11_initial_state_resolved_admits_clean() {
    let mut m = seed_state_manifest();
    m.entity_definition_initial_state
        .push(EntityDefinitionInitialStateRow {
            entity_def_id: "core/ghoul".to_owned(),
            state_id: "core/hp".to_owned(),
            value_bool: true,
            value_int: Some(22),
            value_text: None,
            bundle_id: "core".to_owned(),
        });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_DEF_SEED_ENTITY_REF).is_empty()
            && codes_for(&diags, MF_VAL_DEF_SEED_STATE_REF).is_empty(),
        "resolved state seed must admit clean: {diags:?}"
    );
}

#[test]
fn class11_initial_state_missing_state_id_is_fatal() {
    let mut m = seed_state_manifest();
    m.entity_definition_initial_state
        .push(EntityDefinitionInitialStateRow {
            entity_def_id: "core/ghoul".to_owned(),
            state_id: "core/typo".to_owned(),
            value_bool: true,
            value_int: Some(22),
            value_text: None,
            bundle_id: "core".to_owned(),
        });
    assert_eq!(
        codes_for(&validate_manifest(&m), MF_VAL_DEF_SEED_STATE_REF).len(),
        1
    );
}

#[test]
fn class11_initial_state_missing_entity_def_is_fatal() {
    let mut m = seed_state_manifest();
    m.entity_definition_initial_state
        .push(EntityDefinitionInitialStateRow {
            entity_def_id: "core/missing".to_owned(),
            state_id: "core/hp".to_owned(),
            value_bool: true,
            value_int: Some(1),
            value_text: None,
            bundle_id: "core".to_owned(),
        });
    assert_eq!(
        codes_for(&validate_manifest(&m), MF_VAL_DEF_SEED_ENTITY_REF).len(),
        1
    );
}

#[test]
fn class11_initial_capability_missing_is_fatal() {
    let mut m = base_manifest();
    m.entity_definitions
        .push(entity_def("core/hobgoblin", "core"));
    m.entity_definition_initial_capability
        .push(EntityDefinitionInitialCapabilityRow {
            extent_kind_ref: String::new(),
            extent_ref: String::new(),
            entity_def_id: "core/hobgoblin".to_owned(),
            capability_id: "core/typo".to_owned(),
            bundle_id: "core".to_owned(),
        });
    assert_eq!(
        codes_for(&validate_manifest(&m), MF_VAL_DEF_SEED_CAPABILITY_REF).len(),
        1
    );
}

#[test]
fn class11_initial_capability_resolved_admits_clean() {
    let mut m = base_manifest();
    m.entity_definitions
        .push(entity_def("core/hobgoblin", "core"));
    m.capability_definitions.push(CapabilityDefinitionRow {
        capability_id: "core/multiattack".to_owned(),
        bundle_id: "core".to_owned(),
        actor_kind: String::new(),
        display_name: "Multiattack".to_owned(),
        description: String::new(),
        default_visibility: "public".to_owned(),
    });
    m.entity_definition_initial_capability
        .push(EntityDefinitionInitialCapabilityRow {
            extent_kind_ref: String::new(),
            extent_ref: String::new(),
            entity_def_id: "core/hobgoblin".to_owned(),
            capability_id: "core/multiattack".to_owned(),
            bundle_id: "core".to_owned(),
        });
    assert!(codes_for(&validate_manifest(&m), MF_VAL_DEF_SEED_CAPABILITY_REF).is_empty());
}

#[test]
fn class11_initial_condition_missing_is_fatal() {
    let mut m = base_manifest();
    m.entity_definitions.push(entity_def("core/torch", "core"));
    m.entity_definition_initial_condition
        .push(EntityDefinitionInitialConditionRow {
            entity_def_id: "core/torch".to_owned(),
            condition_definition_id: "core/typo".to_owned(),
            duration_ticks: 0,
            bundle_id: "core".to_owned(),
        });
    assert_eq!(
        codes_for(&validate_manifest(&m), MF_VAL_DEF_SEED_CONDITION_REF).len(),
        1
    );
}

// ---------------------------------------------------------------------------
// sk-ct8.3 / finding #37 (BL-19), canon-warden fix #3 — the two row/column
// kinds the wear/equip-gating slice added: `entity_definition_equip_modifier`
// (Part B link) and `entity_definitions.{on_equip,on_unequip}_consequence_id`
// (Part A hooks).
// ---------------------------------------------------------------------------

fn resolution_check_modifier(bundle: &str, modifier_template_id: &str) -> ModifierTemplateRow {
    ModifierTemplateRow {
        modifier_template_id: modifier_template_id.to_owned(),
        bundle_id: bundle.to_owned(),
        applies_to: "ResolutionCheck".to_owned(),
        display_name: modifier_template_id.to_owned(),
        predicate_source: String::new(),
        modifier_expression_source: "1".to_owned(),
        priority: 0,
        delta_kind: "Plus".to_owned(),
        replace_template_id: None,
        description: String::new(),
    }
}

#[test]
fn class11_equip_modifier_missing_entity_def_is_fatal() {
    let mut m = base_manifest();
    m.modifier_templates
        .push(resolution_check_modifier("core", "core/armor_bonus"));
    m.entity_definition_equip_modifier
        .push(EntityDefinitionEquipModifierRow {
            entity_def_id: "core/missing_armor".to_owned(),
            modifier_template_id: "core/armor_bonus".to_owned(),
            bundle_id: "core".to_owned(),
        });
    assert_eq!(
        codes_for(&validate_manifest(&m), MF_VAL_DEF_SEED_ENTITY_REF).len(),
        1
    );
}

#[test]
fn class11_equip_modifier_missing_modifier_template_is_fatal() {
    let mut m = base_manifest();
    m.entity_definitions.push(entity_def("core/armor", "core"));
    m.entity_definition_equip_modifier
        .push(EntityDefinitionEquipModifierRow {
            entity_def_id: "core/armor".to_owned(),
            modifier_template_id: "core/typo".to_owned(),
            bundle_id: "core".to_owned(),
        });
    assert_eq!(
        codes_for(
            &validate_manifest(&m),
            MF_VAL_DEF_SEED_MODIFIER_TEMPLATE_REF
        )
        .len(),
        1
    );
}

#[test]
fn class11_equip_modifier_resolved_admits_clean() {
    let mut m = base_manifest();
    m.entity_definitions.push(entity_def("core/armor", "core"));
    m.modifier_templates
        .push(resolution_check_modifier("core", "core/armor_bonus"));
    m.entity_definition_equip_modifier
        .push(EntityDefinitionEquipModifierRow {
            entity_def_id: "core/armor".to_owned(),
            modifier_template_id: "core/armor_bonus".to_owned(),
            bundle_id: "core".to_owned(),
        });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_DEF_SEED_ENTITY_REF).is_empty()
            && codes_for(&diags, MF_VAL_DEF_SEED_MODIFIER_TEMPLATE_REF).is_empty(),
        "resolved equip-modifier link must admit clean: {diags:?}"
    );
}

#[test]
fn class11_on_equip_consequence_missing_is_fatal() {
    let mut m = base_manifest();
    m.entity_definitions.push(EntityDefinitionRow {
        on_equip_consequence_id: "core/typo".to_owned(),
        ..entity_def("core/cursed_ring", "core")
    });
    assert_eq!(
        codes_for(
            &validate_manifest(&m),
            MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF
        )
        .len(),
        1
    );
}

#[test]
fn class11_on_unequip_consequence_missing_is_fatal() {
    let mut m = base_manifest();
    m.entity_definitions.push(EntityDefinitionRow {
        on_unequip_consequence_id: "core/typo".to_owned(),
        ..entity_def("core/cursed_ring", "core")
    });
    assert_eq!(
        codes_for(
            &validate_manifest(&m),
            MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF
        )
        .len(),
        1
    );
}

#[test]
fn class11_on_equip_and_on_unequip_resolved_admits_clean() {
    let mut m = base_manifest();
    m.consequence_templates
        .push(frame_write_template("core", "core/cursed_ring/on_equip"));
    m.consequence_templates
        .push(frame_write_template("core", "core/cursed_ring/on_unequip"));
    m.entity_definitions.push(EntityDefinitionRow {
        on_equip_consequence_id: "core/cursed_ring/on_equip".to_owned(),
        on_unequip_consequence_id: "core/cursed_ring/on_unequip".to_owned(),
        ..entity_def("core/cursed_ring", "core")
    });
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF).is_empty(),
        "resolved on_equip/on_unequip hooks must admit clean: {diags:?}"
    );
}

#[test]
fn class11_empty_equip_hooks_are_not_checked() {
    // `entity_def` leaves both hook columns empty ("no hook" — the runtime's
    // own convention). Absence must never be flagged as a dangling ref.
    let mut m = base_manifest();
    m.entity_definitions
        .push(entity_def("core/plain_shirt", "core"));
    let diags = validate_manifest(&m);
    assert!(
        codes_for(&diags, MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF).is_empty(),
        "empty on_equip/on_unequip columns must never be flagged: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// Class 12 — action_definition.required_capability + srd/spell catalog binding (#835)
// ---------------------------------------------------------------------------

fn cap_def(id: &str, bundle: &str) -> CapabilityDefinitionRow {
    CapabilityDefinitionRow {
        capability_id: id.to_owned(),
        bundle_id: bundle.to_owned(),
        actor_kind: String::new(),
        display_name: id.to_owned(),
        description: String::new(),
        default_visibility: "public".to_owned(),
    }
}

fn gated_action(action_id: &str, bundle: &str, required_capability: &str) -> ActionDefinitionRow {
    let mut row = self_reaction_action(bundle, action_id, "");
    row.required_capability = required_capability.to_owned();
    row.reaction_eligibility = None;
    row.reaction_window_ms = None;
    row.reaction_trigger = None;
    row.self_eligible = false;
    row
}

#[test]
fn class12_required_capability_unresolved_is_fatal() {
    let mut m = base_manifest();
    m.action_definitions
        .push(gated_action("srd/cast_ghost", "core", "srd/spell/ghost"));
    assert_eq!(
        codes_for(
            &validate_manifest(&m),
            MF_VAL_ACTION_REQUIRED_CAPABILITY_REF
        )
        .len(),
        1
    );
}

#[test]
fn class12_required_capability_resolved_admits_clean() {
    let mut m = base_manifest();
    m.capability_definitions
        .push(cap_def("srd/spell/fireball", "core"));
    m.action_definitions.push(gated_action(
        "srd/cast_fireball",
        "core",
        "srd/spell/fireball",
    ));
    assert!(codes_for(
        &validate_manifest(&m),
        MF_VAL_ACTION_REQUIRED_CAPABILITY_REF
    )
    .is_empty());
}

#[test]
fn class12_empty_required_capability_is_skipped() {
    let mut m = base_manifest();
    m.action_definitions
        .push(gated_action("core/look", "core", ""));
    assert!(codes_for(
        &validate_manifest(&m),
        MF_VAL_ACTION_REQUIRED_CAPABILITY_REF
    )
    .is_empty());
}

#[test]
fn duplicate_capability_definition_is_fatal() {
    // The cross-fragment collision that bit the Conjure-Animals combat summon: a
    // capability re-emitted in a second fragment. The runtime rejects it with
    // MF-APPLY-020; this is the static mirror, surfaced once `--bundle` merges both
    // fragments into one capability_definitions Vec.
    let mut m = base_manifest();
    m.capability_definitions
        .push(cap_def("srd/monster-action/wolf_bite", "srd"));
    m.capability_definitions
        .push(cap_def("srd/monster-action/wolf_bite", "srd"));
    assert_eq!(
        codes_for(&validate_manifest(&m), MF_VAL_CAPABILITY_DEFINITION_DUP).len(),
        1,
        "a duplicate capability_id must be a single Fatal diagnostic"
    );
}

#[test]
fn unique_capability_definitions_admit_clean() {
    // Distinct ids never trip the duplicate check (the common, correct case).
    let mut m = base_manifest();
    m.capability_definitions
        .push(cap_def("srd/monster-action/wolf_bite", "srd"));
    m.capability_definitions
        .push(cap_def("srd/monster-action/goblin_scimitar", "srd"));
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_CAPABILITY_DEFINITION_DUP).is_empty(),
        "distinct capability_ids must not trip the duplicate check"
    );
}

#[test]
fn class12_srd_spell_capability_without_catalog_entity_warns() {
    let mut m = base_manifest();
    m.capability_definitions
        .push(cap_def("srd/spell/orphan", "core"));
    let diags = validate_manifest(&m);
    assert_eq!(
        codes_for(&diags, MF_VAL_SPELL_CAPABILITY_CATALOG_CONVENTION).len(),
        1
    );
    // Convention drift is a WARNING, never a Fatal rejection.
    assert!(diags
        .iter()
        .filter(|d| d.code == MF_VAL_SPELL_CAPABILITY_CATALOG_CONVENTION)
        .all(|d| d.severity == Severity::Warning));
}

#[test]
fn class12_srd_spell_capability_bound_to_catalog_entity_is_clean() {
    let mut m = base_manifest();
    m.capability_definitions
        .push(cap_def("srd/spell/fireball", "core"));
    m.entity_definitions
        .push(entity_def("srd/spell/fireball", "core"));
    assert!(codes_for(
        &validate_manifest(&m),
        MF_VAL_SPELL_CAPABILITY_CATALOG_CONVENTION
    )
    .is_empty());
}

/// #1227 cross-bundle leniency (CR-12 closure-completeness). A dependent-bundle
/// outcome/affordance whose `manipulation_action_id` points at a GENERIC dependency
/// action is TRUSTED when validated as a PARTIAL manifest (the dependency's own
/// `[[bundles]]` row absent — a `just seed <dependent-bundle>` apply, where the
/// runtime resolves the action from the merged world). But a cross-bundle TYPO is
/// still caught when the owning bundle's whole closure IS present (a `--bundle --dep`
/// merge). A same-bundle absent action always fatals. (The closure-completeness gate,
/// NOT an id-prefix shape check — the latter would mis-trust the typo when the
/// dependency is merged.)
#[test]
fn manip_action_cross_bundle_lenient_when_partial_but_fatal_when_closure_present() {
    fn manifest(action_id: &str, srd_present: bool) -> Manifest {
        let mut m = base_manifest(); // has the `core` bundle
        if srd_present {
            m.bundles.push(BundleRow {
                bundle_id: "srd".to_owned(),
                display_name: "srd".to_owned(),
                author: "t".to_owned(),
                version: "1".to_owned(),
                depends_on: vec!["core".to_owned()],
                description: String::new(),
                tag_prefixes: Vec::new(),
                allow_flat_tags: false,
                default_extent_shape_id: String::new(),
                default_extent_params_json: String::new(),
            });
        }
        m.bundles.push(BundleRow {
            bundle_id: "phandelver".to_owned(),
            display_name: "ph".to_owned(),
            author: "t".to_owned(),
            version: "1".to_owned(),
            depends_on: vec!["core".to_owned(), "srd".to_owned()],
            description: String::new(),
            tag_prefixes: Vec::new(),
            allow_flat_tags: false,
            default_extent_shape_id: String::new(),
            default_extent_params_json: String::new(),
        });
        m.outcome_templates.push(OutcomeTemplateRow {
            outcome_template_id: "phandelver/react/hit".to_owned(),
            bundle_id: "phandelver".to_owned(),
            display_name: "Hit".to_owned(),
            outcome_bool: true,
            outcome_expression_source: String::new(),
            manipulation_action_id: action_id.to_owned(),
            communication_speech_act_id: String::new(),
            movement_descriptor: String::new(),
            sense_query_kind: String::new(),
            sense_visibility: spellkave_schema::VisibilityClass::Local,
            consequence_template_id: String::new(),
            discovery_validated: false,
            description: String::new(),
        });
        m.affordance_templates.push(AffordanceTemplateRow {
            template_id: "phandelver/react/affordance".to_owned(),
            bundle_id: "phandelver".to_owned(),
            niche_id: "phandelver/react".to_owned(),
            capability_id: String::new(),
            display_name: "React".to_owned(),
            predicate_bool: true,
            predicate_source: String::new(),
            accepts_global_storm: false,
            locality_class: String::new(),
            reads_occupancy: false,
            description: String::new(),
            stratification_ok: true,
            outcome_template_id: "phandelver/react/hit".to_owned(),
            range_m: 1.5,
            spends_channel: None,
            recharges_on: None,
            composes: None,

            movement_cost_expression_source: String::new(),
        });
        m
    }
    let has =
        |diags: &[spellkave_validate::Diagnostic], code: &str| diags.iter().any(|d| d.code == code);
    let any_manip_fatal = |diags: &[spellkave_validate::Diagnostic]| {
        has(diags, MF_VAL_MANIP_ACTION_ACTION_REF)
            || has(diags, MF_VAL_MANIP_ACTION_TEMPLATE_ACTION_REF)
    };

    // PARTIAL closure (srd `[[bundles]]` row absent): the cross-bundle dep ref is TRUSTED.
    let partial = validate_manifest(&manifest("srd/attack/melee", false));
    assert!(
        !any_manip_fatal(&partial),
        "partial-closure cross-bundle dependency ref must be trusted; got {:?}",
        partial.iter().map(|d| d.code.clone()).collect::<Vec<_>>()
    );

    // FULL closure (srd present): a cross-bundle TYPO is caught (the load-bearing case).
    let typo_full = validate_manifest(&manifest("srd/attack/meele", true));
    assert!(
        any_manip_fatal(&typo_full),
        "a cross-bundle typo with the closure fully present must fatal"
    );

    // Same-bundle absent action (a typo) always fatals.
    let same_bundle = validate_manifest(&manifest("phandelver/typo_attack", true));
    assert!(
        any_manip_fatal(&same_bundle),
        "same-bundle absent action (typo) must fatal"
    );
}

// ---------------------------------------------------------------------------
// Class 13 — srd.weapon tag ⇒ srd/damage DSL-slot co-occurrence (#1258 / sk-hml)
// ---------------------------------------------------------------------------

/// `base_manifest()` plus an `srd` bundle (depends on core).
fn srd_manifest() -> Manifest {
    let mut m = base_manifest();
    m.bundles.push(BundleRow {
        bundle_id: "srd".to_owned(),
        display_name: "srd".to_owned(),
        author: "test".to_owned(),
        version: "1.0.0".to_owned(),
        depends_on: vec!["core".to_owned()],
        description: String::new(),
        tag_prefixes: Vec::new(),
        allow_flat_tags: false,
        default_extent_shape_id: String::new(),
        default_extent_params_json: String::new(),
    });
    m
}

fn weapon_tag(def: &str, bundle: &str) -> EntityDefinitionTagRow {
    EntityDefinitionTagRow {
        entity_def_id: def.to_owned(),
        tag: "srd.weapon".to_owned(),
        bundle_id: bundle.to_owned(),
        source: "manifest".to_owned(),
    }
}

fn damage_slot(def: &str, bundle: &str) -> EntityDefinitionItemDslSourceRow {
    EntityDefinitionItemDslSourceRow {
        entity_def_id: def.to_owned(),
        slot: "srd/damage".to_owned(),
        source: "1d8".to_owned(),
        bundle_id: bundle.to_owned(),
    }
}

#[test]
fn class13_weapon_def_without_damage_slot_warns() {
    // Slotless srd.weapon: materialises attack affordances but fails closed at
    // act time (MissingItemDslSource) — the footgun Class 13 catches early.
    let mut m = srd_manifest();
    m.entity_definitions
        .push(entity_def("srd/weapon/longsword", "srd"));
    m.entity_definition_tag
        .push(weapon_tag("srd/weapon/longsword", "srd"));
    let diags = validate_manifest(&m);
    assert_eq!(
        codes_for(&diags, MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION).len(),
        1,
        "an srd.weapon-tagged def with no srd/damage slot must warn once"
    );
    assert!(diags
        .iter()
        .filter(|d| d.code == MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION)
        .all(|d| d.severity == Severity::Warning));
}

#[test]
fn class13_weapon_def_with_damage_slot_is_clean() {
    let mut m = srd_manifest();
    m.entity_definitions
        .push(entity_def("srd/weapon/longsword", "srd"));
    m.entity_definition_tag
        .push(weapon_tag("srd/weapon/longsword", "srd"));
    m.entity_definition_item_dsl_source
        .push(damage_slot("srd/weapon/longsword", "srd"));
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION).is_empty(),
        "a weapon def carrying an srd/damage item DSL-source must not warn"
    );
}

#[test]
fn class13_weapon_property_tag_alone_does_not_warn() {
    // Sub-tags like srd.weapon.property.finesse are not the identity tag the
    // attack archetype gates on — only exact `srd.weapon` trips Class 13.
    let mut m = srd_manifest();
    m.entity_definitions
        .push(entity_def("srd/weapon/longsword", "srd"));
    m.entity_definition_tag.push(EntityDefinitionTagRow {
        entity_def_id: "srd/weapon/longsword".to_owned(),
        tag: "srd.weapon.property.finesse".to_owned(),
        bundle_id: "srd".to_owned(),
        source: "manifest".to_owned(),
    });
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION).is_empty(),
        "property tags alone must not trip the srd.weapon damage-slot convention"
    );
}

#[test]
fn class13_weapon_def_out_of_srd_scope_does_not_warn() {
    // A core-only bundle is outside the srd closure — modder/core combat may
    // use a different damage mechanism.
    let mut m = base_manifest();
    m.entity_definitions
        .push(entity_def("core/item/sword", "core"));
    m.entity_definition_tag
        .push(weapon_tag("core/item/sword", "core"));
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION).is_empty(),
        "an out-of-srd-scope weapon def must not trip the srd convention"
    );
}

fn str_resource(bundle: &str) -> ResourceDefinitionRow {
    ResourceDefinitionRow {
        resource_definition_id: format!("{bundle}/str"),
        bundle_id: bundle.to_owned(),
        display_name: "STR".to_owned(),
        value_kind: "int".to_owned(),
        min_value: 1,
        max_value: 30,
        recovery_rule: "none".to_owned(),
        initial_value: None,
        description: String::new(),
    }
}

#[test]
fn formula_unresolved_stat_in_item_dsl_source_is_fatal() {
    let mut m = srd_manifest();
    m.entity_definition_item_dsl_source
        .push(EntityDefinitionItemDslSourceRow {
            entity_def_id: "srd/weapon/battleaxe".to_owned(),
            slot: "srd/damage".to_owned(),
            source: "1d8 + actor.stat('srd/no_such_stat')".to_owned(),
            bundle_id: "srd".to_owned(),
        });
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_DSL_FORMULA_STAT_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-329; got: {diags:?}");
    assert!(msgs[0].contains("srd/weapon/battleaxe:srd/damage"));
    assert!(msgs[0].contains("srd/no_such_stat"));
    assert!(diags
        .iter()
        .any(|d| d.code == MF_VAL_DSL_FORMULA_STAT_REF && d.severity == Severity::Fatal));
}

#[test]
fn formula_catalogued_stat_in_item_dsl_source_admits() {
    let mut m = srd_manifest();
    m.resource_definitions.push(str_resource("srd"));
    m.entity_definition_item_dsl_source
        .push(EntityDefinitionItemDslSourceRow {
            entity_def_id: "srd/weapon/battleaxe".to_owned(),
            slot: "srd/damage".to_owned(),
            source: "1d8 + actor.stat('srd/str')".to_owned(),
            bundle_id: "srd".to_owned(),
        });
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_DSL_FORMULA_STAT_REF).is_empty(),
        "a formula whose stat resolves to a resource_definition must admit"
    );
}

#[test]
fn formula_object_state_stat_in_item_dsl_source_admits() {
    let mut m = srd_manifest();
    m.object_state_definitions.push(ObjectStateDefinitionRow {
        state_id: "srd/str".to_owned(),
        bundle_id: "srd".to_owned(),
        target_kind: "entity".to_owned(),
        display_name: "STR".to_owned(),
        default_visibility: "public".to_owned(),
        description: String::new(),
    });
    m.entity_definition_item_dsl_source
        .push(EntityDefinitionItemDslSourceRow {
            entity_def_id: "srd/weapon/battleaxe".to_owned(),
            slot: "srd/damage".to_owned(),
            source: "1d8 + actor.effective('srd/str')".to_owned(),
            bundle_id: "srd".to_owned(),
        });
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_DSL_FORMULA_STAT_REF).is_empty(),
        "a formula whose stat resolves to an object_state_definition must admit"
    );
}

#[test]
fn formula_unresolved_stat_in_delta_source_is_fatal() {
    let mut m = base_manifest();
    m.resource_definitions.push(hp_resource("core"));
    m.consequence_templates
        .push(frame_write_template("core", "core/swing/success"));
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "core/swing/success".to_owned(),
        ordinal: 0,
        frame_write: serde_json::json!({
            "ModifyResource": {
                "actor": "Actor",
                "resource": "core/hp",
                "delta_source": "0 - actor.stat('srd/no_such_stat')"
            }
        }),
    });
    let diags = validate_manifest(&m);
    let msgs = codes_for(&diags, MF_VAL_DSL_FORMULA_STAT_REF);
    assert_eq!(msgs.len(), 1, "expected one MF-VAL-329; got: {diags:?}");
    assert!(msgs[0].contains("srd/no_such_stat"));
}

#[test]
fn formula_kernel_projected_stat_is_not_required_in_catalog() {
    let mut m = srd_manifest();
    m.entity_definition_item_dsl_source
        .push(EntityDefinitionItemDslSourceRow {
            entity_def_id: "srd/weapon/battleaxe".to_owned(),
            slot: "srd/damage".to_owned(),
            source: "actor.stat('core/dispatch_arg_quantity')".to_owned(),
            bundle_id: "srd".to_owned(),
        });
    assert!(
        codes_for(&validate_manifest(&m), MF_VAL_DSL_FORMULA_STAT_REF).is_empty(),
        "kernel-projected facts must not require a catalog row"
    );
}

#[test]
fn formula_unresolved_stat_trusts_when_closure_incomplete_offline() {
    // sk-zqc CR-12 mirrored onto MF-VAL-329: a per-file validate of `srd`
    // alone (`core`'s `[[bundles]]` row absent) reading a stat declared in
    // another file is correct as written — Info with a hint, never Fatal.
    let mut m = srd_only_manifest();
    m.entity_definition_item_dsl_source
        .push(EntityDefinitionItemDslSourceRow {
            entity_def_id: "srd/weapon/battleaxe".to_owned(),
            slot: "srd/damage".to_owned(),
            source: "1d8 + actor.stat('srd/str')".to_owned(),
            bundle_id: "srd".to_owned(),
        });
    let diags = validate_manifest(&m);
    assert!(
        !diags.iter().any(|d| d.severity == Severity::Fatal),
        "a per-file validate whose formula stat is declared outside the file must not Fatal; got {diags:?}"
    );
    let info = diags
        .iter()
        .find(|d| d.code == MF_VAL_DSL_FORMULA_STAT_REF)
        .expect("expected an MF-VAL-329 Info diagnostic recording the trust");
    assert_eq!(info.severity, Severity::Info);
    assert!(info.hint.is_some());
    assert!(info.message.contains("srd/str"));
}

// roles-as-content: offline vocabulary and live admission are different oracles.
fn role_content_manifest() -> Manifest {
    let mut manifest = base_manifest();
    manifest.bundles[0].bundle_id = "house_rules".into();
    manifest
        .role_definitions
        .push(spellkave_schema::RoleDefinitionRow {
            role_id: "house_rules/gm".into(),
            bundle_id: "house_rules".into(),
            display_name: "House GM".into(),
            description: "An authored vocabulary role.".into(),
            capability_ids: vec![],
        });
    manifest
}

#[test]
fn role_content_missing_capability_and_empty_aggregation_fail_closed() {
    let mut manifest = role_content_manifest();
    manifest.role_definitions[0]
        .capability_ids
        .push("house_rules/missing".into());
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-366" && d.message.contains("house_rules/missing")));
    manifest.role_definitions[0].capability_ids.clear();
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.code.starts_with("MF-VAL-36") && d.code != "MF-VAL-363"));
    manifest
        .role_aggregations
        .push(spellkave_schema::RoleAggregationRow {
            role_id: "house_rules/gm".into(),
            bundle_id: "house_rules".into(),
            description: "Nothing to add.".into(),
            capability_ids: vec![],
        });
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-368"));
}

#[test]
fn role_content_offline_standard_role_needs_no_core_fixture() {
    let mut manifest = role_content_manifest();
    manifest.bundles[0].bundle_id = "elven_mini".into();
    manifest.role_definitions.clear();
    manifest
        .role_aggregations
        .push(spellkave_schema::RoleAggregationRow {
            role_id: "dm".into(),
            bundle_id: "elven_mini".into(),
            description: "Authored addition.".into(),
            capability_ids: vec!["frontier.read_hidden".into()],
        });
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| matches!(d.code.as_str(), "MF-VAL-364" | "MF-VAL-366" | "MF-VAL-368")));
    let live = LiveVocabulary::default();
    let diagnostics = validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live));
    assert!(diagnostics.iter().any(|d| d.code == "MF-VAL-366"));
    assert!(diagnostics.iter().any(|d| d.code == "MF-VAL-368"));
}

#[test]
fn role_content_foreign_takeover_refuses_but_same_owner_update_is_allowed() {
    let mut manifest = role_content_manifest();
    manifest.role_definitions[0].role_id = "dm".into();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-367"));
    manifest.role_definitions[0].bundle_id = "core".into();
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-367"));
}

#[test]
fn role_content_live_role_without_file_dependency_resolves_and_resists_takeover() {
    let original = role_content_manifest().role_definitions.remove(0);
    let mut live = LiveVocabulary::default();
    live.roles
        .insert(original.role_id.clone(), original.clone());
    live.capabilities
        .insert("live_author/weather".into(), "live_author".into());
    let mut manifest = role_content_manifest();
    manifest.role_definitions.clear();
    manifest.bundles[0].bundle_id = "other_author".into();
    manifest
        .role_aggregations
        .push(spellkave_schema::RoleAggregationRow {
            role_id: original.role_id.clone(),
            bundle_id: "other_author".into(),
            description: "Live contribution.".into(),
            capability_ids: vec!["live_author/weather".into()],
        });
    assert!(
        !validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live))
            .iter()
            .any(|d| matches!(d.code.as_str(), "MF-VAL-366" | "MF-VAL-368"))
    );
    let mut takeover = original.clone();
    takeover.bundle_id = "other_author".into();
    manifest.role_definitions.push(takeover);
    assert!(
        validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live))
            .iter()
            .any(|d| d.code == "MF-VAL-367")
    );
    assert_eq!(live.roles[&original.role_id], original);
}

#[test]
fn role_content_merged_set_keeps_all_contributors_sorted_and_unique() {
    let mut definition = role_content_manifest().role_definitions.remove(0);
    definition.capability_ids = vec!["b".into(), "a".into(), "b".into()];
    let aggregation = spellkave_schema::RoleAggregationRow {
        role_id: definition.role_id.clone(),
        bundle_id: "contributor".into(),
        description: "Contribution.".into(),
        capability_ids: vec!["c".into(), "a".into()],
    };
    assert_eq!(
        spellkave_validate::merged_role_capability_ids(
            &definition.role_id,
            &[definition.clone()],
            &[aggregation]
        ),
        vec!["a", "b", "c"]
    );
}

#[test]
fn role_content_owning_bundle_must_exist() {
    let mut manifest = role_content_manifest();
    manifest.role_definitions[0].bundle_id = "absent".into();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.path == "role_definitions.bundle_id"
            && d.message.contains("unknown owning bundle")));
}

fn binding_manifest(subject: &str) -> Manifest {
    let mut manifest = role_content_manifest();
    manifest
        .role_bindings
        .push(spellkave_schema::RoleBindingRow {
            binding_id: "house_rules/gm_binding".into(),
            bundle_id: "house_rules".into(),
            subject: subject.into(),
            role_id: "house_rules/gm".into(),
            extent_kind_ref: String::new(),
            extent_ref: String::new(),
            expires_at_tick: None,
            description: "Authorial authority.".into(),
            handover_note: None,
        });
    manifest
}

#[test]
fn role_binding_identity_and_sender_subjects_resolve_without_realizing() {
    for subject in [
        "$sender".to_owned(),
        "ab".repeat(32),
        format!("0x{}", "ab".repeat(32)),
    ] {
        let manifest = binding_manifest(&subject);
        let diagnostics = validate_manifest(&manifest);
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d.code.as_str(), "MF-VAL-369" | "MF-VAL-370")),
            "{diagnostics:?}"
        );
        assert_eq!(manifest.role_bindings.len(), 1);
    }
    for subject in ["", "operator", "abcd", "$other", &"gg".repeat(32)] {
        assert!(
            validate_manifest(&binding_manifest(subject))
                .iter()
                .any(|d| d.code == "MF-VAL-370"),
            "{subject}"
        );
    }
}

#[test]
fn role_binding_requires_live_or_prospective_role_in_bundle_scope() {
    let mut manifest = binding_manifest("$sender");
    manifest.role_bindings[0].role_id = "missing".into();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-369"));
    manifest.role_bindings[0].role_id = "dm".into();
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-369"));
    assert!(validate_manifest_with_world(
        &manifest,
        &BTreeSet::new(),
        Some(&LiveVocabulary::default())
    )
    .iter()
    .any(|d| d.code == "MF-VAL-369"));
    let bootstrap = spellkave_format::parse_manifest(spellkave_schema::CORE_BOOTSTRAP_MANIFEST)
        .unwrap()
        .manifest;
    assert!(!validate_manifest_with_world(
        &manifest,
        &BTreeSet::new(),
        Some(&LiveVocabulary::from_manifest(&bootstrap))
    )
    .iter()
    .any(|d| d.code == "MF-VAL-369"));
}

#[test]
fn role_binding_npc_subject_refuses_instance_and_definition_playable_tags() {
    let mut manifest = binding_manifest("house_rules/guard");
    manifest.actors.push(spellkave_schema::ActorRow {
        entity_key: "house_rules/guard".into(),
        bundle_id: "house_rules".into(),
        name: "Guard".into(),
        description: String::new(),
    });
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-370"));
    manifest.entity_tag.push(EntityTagRow {
        entity_key: "house_rules/guard".into(),
        bundle_id: "house_rules".into(),
        tag: spellkave_schema::PLAYER_SEAT_TAG.into(),
        source: "authored".into(),
    });
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-370"));
    manifest.entity_tag.clear();
    manifest.entities.push(EntityRow {
        entity_key: "house_rules/guard".into(),
        bundle_id: "house_rules".into(),
        entity_def_id: "house_rules/hero".into(),
        name: "Guard".into(),
        kind: EntityKindRow::Npc,
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
        description: String::new(),
    });
    manifest.entity_definition_tag.push(EntityDefinitionTagRow {
        entity_def_id: "house_rules/hero".into(),
        bundle_id: "house_rules".into(),
        tag: spellkave_schema::PLAYER_SEAT_TAG.into(),
        source: "authored".into(),
    });
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-370"));
    let live = LiveVocabulary::from_manifest(&manifest);
    manifest.actors.clear();
    manifest.entities.clear();
    manifest.entity_definition_tag.clear();
    assert!(
        validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live))
            .iter()
            .any(|d| d.code == "MF-VAL-370")
    );
}

#[test]
fn role_binding_actor_redeclaration_keeps_live_playable_tag_evidence() {
    for definition_tag in [false, true] {
        let mut manifest = binding_manifest("house_rules/guard");
        manifest.actors.push(spellkave_schema::ActorRow {
            entity_key: "house_rules/guard".into(),
            bundle_id: "house_rules".into(),
            name: "Guard".into(),
            description: String::new(),
        });
        manifest.entities.push(EntityRow {
            entity_key: "house_rules/guard".into(),
            bundle_id: "house_rules".into(),
            entity_def_id: "house_rules/hero".into(),
            name: "Guard".into(),
            kind: EntityKindRow::Npc,
            place_node_id: String::new(),
            quantity: 1,
            owned_by_entity_key: String::new(),
            description: String::new(),
        });
        if definition_tag {
            manifest.entity_definition_tag.push(EntityDefinitionTagRow {
                entity_def_id: "house_rules/hero".into(),
                bundle_id: "house_rules".into(),
                tag: spellkave_schema::PLAYER_SEAT_TAG.into(),
                source: "authored".into(),
            });
        } else {
            manifest.entity_tag.push(EntityTagRow {
                entity_key: "house_rules/guard".into(),
                bundle_id: "house_rules".into(),
                tag: spellkave_schema::PLAYER_SEAT_TAG.into(),
                source: "authored".into(),
            });
        }
        let live = LiveVocabulary::from_manifest(&manifest);
        assert!(live.role_subjects["house_rules/guard"].1);
        manifest.entity_tag.clear();
        manifest.entity_definition_tag.clear();
        let diagnostics = validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live));
        assert!(
            diagnostics.iter().any(|d| d.code == "MF-VAL-370"),
            "redeclaring an actor must not erase live playability: {diagnostics:?}"
        );
    }
}

#[test]
fn role_binding_new_actor_refuses_live_playable_definition_without_repeated_tag() {
    let mut manifest = binding_manifest("house_rules/new_actor");
    let mut live_manifest = role_content_manifest();
    live_manifest
        .entity_definition_tag
        .push(EntityDefinitionTagRow {
            entity_def_id: "house_rules/hero".into(),
            bundle_id: "house_rules".into(),
            tag: spellkave_schema::PLAYER_SEAT_TAG.into(),
            source: "authored".into(),
        });
    let live = LiveVocabulary::from_manifest(&live_manifest);
    assert!(live
        .playable_entity_definitions
        .contains("house_rules/hero"));
    assert!(!live.role_subjects.contains_key("house_rules/new_actor"));
    manifest.actors.push(spellkave_schema::ActorRow {
        entity_key: "house_rules/new_actor".into(),
        bundle_id: "house_rules".into(),
        name: "New actor".into(),
        description: String::new(),
    });
    manifest.entities.push(EntityRow {
        entity_key: "house_rules/new_actor".into(),
        bundle_id: "house_rules".into(),
        entity_def_id: "house_rules/hero".into(),
        name: "New actor".into(),
        kind: EntityKindRow::Npc,
        place_node_id: String::new(),
        quantity: 1,
        owned_by_entity_key: String::new(),
        description: String::new(),
    });
    let diagnostics = validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live));
    assert!(
        diagnostics.iter().any(|d| d.code == "MF-VAL-370"),
        "the live definition's seat tag must refuse an actor-key binding: {diagnostics:?}"
    );
    manifest.entities[0].entity_def_id = "house_rules/ordinary".into();
    assert!(
        !validate_manifest_with_world(&manifest, &BTreeSet::new(), Some(&live))
            .iter()
            .any(|d| d.code == "MF-VAL-370")
    );
}

#[test]
fn role_binding_extent_checks_and_privileged_warning_do_not_close_migration_lane() {
    let mut manifest = binding_manifest("$sender");
    manifest.role_bindings[0].extent_ref = "house_rules/missing".into();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.path == "role_bindings.extent_ref" && d.code == MF_VAL_FK_CLOSURE));
    manifest.role_bindings[0].extent_kind_ref = "party".into();
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.path.starts_with("role_bindings.extent")));
    manifest.role_bindings[0].extent_kind_ref = "missing".into();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.path == "role_bindings.extent_kind_ref"));
    manifest.role_bindings[0].extent_kind_ref.clear();
    manifest.role_bindings[0].extent_ref.clear();
    manifest.role_definitions[0].capability_ids = vec!["frontier.observe.internal".into()];
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-371" && d.severity == Severity::Warning));
    manifest
        .actor_identities
        .push(spellkave_schema::ActorIdentityRow {
            entity_key: "house_rules/guard".into(),
            bundle_id: "house_rules".into(),
            identity: "$sender".into(),
            role: "presence".into(),
            provenance: None,
        });
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-372"));
}

#[test]
fn shared_handover_note_bounds_are_utf8_bytes() {
    let mut manifest = binding_manifest("$sender");
    manifest.role_bindings[0].handover_note = Some("é".repeat(8192));
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-382"));
    manifest.role_bindings[0]
        .handover_note
        .as_mut()
        .unwrap()
        .push('é');
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-382" && d.severity == Severity::Fatal));
}
