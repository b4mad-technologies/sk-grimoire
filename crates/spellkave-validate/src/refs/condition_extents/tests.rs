//! K1: reference closure and ownership admission are pure validator contracts.
use super::*;
use spellkave_schema::ConditionCapabilityExtentGrantRow;

fn fixture() -> (Manifest, LiveVocabulary) {
    let mut manifest = Manifest::default();
    manifest
        .condition_capability_extent_grants
        .push(ConditionCapabilityExtentGrantRow {
            condition_definition_id: "custom/bond".into(),
            capability_id: "custom/command".into(),
            extent_kind_ref: "party".into(),
            extent_ref: "custom.members".into(),
            bundle_id: "custom".into(),
        });
    let mut live = LiveVocabulary {
        bundles: ["core".into(), "custom".into(), "foreign".into()].into(),
        ..LiveVocabulary::default()
    };
    live.dm
        .bundle_dependencies
        .insert("custom".into(), BTreeSet::new());
    live.dm
        .conditions
        .insert("custom/bond".into(), "custom".into());
    live.capabilities
        .insert("custom/command".into(), "custom".into());
    (manifest, live)
}

fn diagnostics(manifest: &Manifest, live: &LiveVocabulary) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    validate(manifest, Some(live), &mut diags);
    diags
}

// K1: open Party tags remain valid; unknown owning bundles cannot bypass closure.
#[test]
fn unknown_owner_is_rejected_and_known_open_party_tag_is_admitted() {
    let (mut manifest, live) = fixture();
    assert!(diagnostics(&manifest, &live).is_empty());
    manifest.condition_capability_extent_grants[0].bundle_id = "missing".into();
    assert!(diagnostics(&manifest, &live)
        .iter()
        .any(|d| d.path.ends_with(".bundle_id")));
}

// K1: missing definitions and undeclared dependency edges must not confer authority.
#[test]
fn references_require_existence_and_owner_dependency_closure() {
    let (manifest, mut live) = fixture();
    live.dm.conditions.clear();
    assert!(diagnostics(&manifest, &live)
        .iter()
        .any(|d| d.path.ends_with(".condition_definition_id")));
    live.dm
        .conditions
        .insert("custom/bond".into(), "foreign".into());
    assert!(!diagnostics(&manifest, &live).is_empty());
    live.dm
        .bundle_dependencies
        .insert("custom".into(), ["foreign".into()].into());
    assert!(diagnostics(&manifest, &live).is_empty());
    live.capabilities.clear();
    assert!(diagnostics(&manifest, &live)
        .iter()
        .any(|d| d.path.ends_with(".capability_id")));
}

// K1: owner lock and duplicate identity survive whole-bundle replacement admission.
#[test]
fn duplicate_key_and_foreign_takeover_are_rejected() {
    let (mut manifest, mut live) = fixture();
    let row = &manifest.condition_capability_extent_grants[0];
    let key = storage_key_strs(&[
        &row.condition_definition_id,
        &row.capability_id,
        &row.extent_kind_ref,
        &row.extent_ref,
    ]);
    live.dm
        .condition_extent_owners
        .insert(key, "foreign".into());
    assert!(diagnostics(&manifest, &live)
        .iter()
        .any(|d| d.message.contains("owning bundle")));
    live.dm.condition_extent_owners.clear();
    manifest
        .condition_capability_extent_grants
        .push(manifest.condition_capability_extent_grants[0].clone());
    assert!(diagnostics(&manifest, &live)
        .iter()
        .any(|d| d.message.contains("duplicate")));
}

// K1: the rule cannot synthesize a new extent shape or target an unknown graph.
#[test]
fn existing_extent_shapes_resolve_their_references() {
    let (mut manifest, mut live) = fixture();
    let row = &mut manifest.condition_capability_extent_grants[0];
    row.extent_kind_ref = "missing".into();
    assert!(!diagnostics(&manifest, &live).is_empty());
    manifest.condition_capability_extent_grants[0].extent_kind_ref = "place_graph".into();
    manifest.condition_capability_extent_grants[0].extent_ref = "custom/room".into();
    assert!(!diagnostics(&manifest, &live).is_empty());
    live.place_graphs
        .insert("custom/room".into(), "custom".into());
    assert!(diagnostics(&manifest, &live).is_empty());
    manifest.condition_capability_extent_grants[0].extent_kind_ref = "bundle".into();
    manifest.condition_capability_extent_grants[0].extent_ref = "custom".into();
    assert!(diagnostics(&manifest, &live).is_empty());
}

// K1: reserved frontier capabilities cannot be acquired through condition rules.
#[test]
fn frontier_and_empty_reference_are_rejected() {
    let (mut manifest, mut live) = fixture();
    live.capabilities
        .insert("frontier.drive".into(), "core".into());
    manifest.condition_capability_extent_grants[0].capability_id = "frontier.drive".into();
    assert!(diagnostics(&manifest, &live)
        .iter()
        .any(|d| d.code
            == super::super::privileged_namespace::MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT));
    manifest.condition_capability_extent_grants[0].capability_id = "custom/command".into();
    manifest.condition_capability_extent_grants[0]
        .extent_ref
        .clear();
    assert!(!diagnostics(&manifest, &live).is_empty());
}
