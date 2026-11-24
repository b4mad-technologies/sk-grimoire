//! OWN-91-Q1: an instance directive names its applying owner, not its blueprint owner.
use spellkave_schema::{BundleRow, Manifest, PlaceGraphInstanceRow};
use spellkave_validate::{validate_manifest, Severity};

fn manifest(explicit: Option<&str>) -> Manifest {
    let mut manifest = Manifest {
        bundles: ["adventure", "other"]
            .into_iter()
            .map(|id| BundleRow {
                bundle_id: id.into(),
                display_name: id.into(),
                author: "test".into(),
                version: "1.0.0".into(),
                depends_on: vec!["srd".into()],
                description: String::new(),
                tag_prefixes: vec![],
                allow_flat_tags: false,
                default_extent_shape_id: String::new(),
                default_extent_params_json: String::new(),
            })
            .collect(),
        ..Manifest::default()
    };
    let mut directive = serde_json::json!({"blueprint_id":"srd/blueprint", "instance_id":"adventure/instance", "anchor":{"kind":"h3","id":"0"}});
    if let Some(owner) = explicit {
        directive["bundle_id"] = owner.into();
    }
    manifest
        .place_graph_instances
        .push(serde_json::from_value::<PlaceGraphInstanceRow>(directive).unwrap());
    manifest
}

#[test]
fn ambiguous_foreign_omission_is_fatal() {
    let diags = validate_manifest(&manifest(None));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "MF-VAL-373" && d.severity == Severity::Fatal),
        "{diags:?}"
    );
}

#[test]
fn explicit_applying_owner_is_admitted() {
    assert!(!validate_manifest(&manifest(Some("adventure")))
        .iter()
        .any(|d| d.code == "MF-VAL-373"));
}

#[test]
fn owner_outside_the_apply_is_refused() {
    assert!(validate_manifest(&manifest(Some("srd")))
        .iter()
        .any(|d| d.code == "MF-VAL-373" && d.severity == Severity::Fatal));
}

#[test]
fn omitted_single_applying_bundle_stays_compatible() {
    let mut manifest = manifest(None);
    manifest.bundles.truncate(1);
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-373"));
}

#[test]
fn incoming_local_blueprint_uses_its_applying_header() {
    let mut manifest = manifest(None);
    manifest.place_graphs.push(serde_json::from_value(serde_json::json!({
        "place_graph_id":"srd/blueprint", "bundle_id":"adventure", "substrate_id":"indoor", "display_name":"Inline owner proof"
    })).unwrap());
    assert_eq!(
        spellkave_validate::instance_owner::instance_owner(
            &manifest,
            &manifest.place_graph_instances[0]
        )
        .unwrap(),
        "adventure"
    );
}

#[test]
fn no_header_inline_declaration_cannot_supply_an_owner() {
    let mut manifest = manifest(None);
    manifest.bundles.clear();
    manifest.place_graphs.push(serde_json::from_value(serde_json::json!({
        "place_graph_id":"srd/blueprint", "bundle_id":"adventure", "substrate_id":"indoor", "display_name":"Missing header proof"
    })).unwrap());
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-373" && d.severity == Severity::Fatal));
}

#[test]
fn an_empty_header_cannot_supply_the_world_owner() {
    let mut manifest = manifest(None);
    manifest.bundles.truncate(1);
    manifest.bundles[0].bundle_id.clear();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-373" && d.severity == Severity::Fatal));
}
