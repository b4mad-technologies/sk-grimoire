use serde_json::{json, Value};
use spellkave_schema::Manifest;
use spellkave_validate::{
    validate_manifest, validate_manifest_with_world_for_apply, LiveVocabulary,
};

fn manifest(value: Value) -> Manifest {
    serde_json::from_value(value).unwrap()
}
fn base() -> Value {
    json!({"schema_version":"1.0.0", "bundles":[{"bundle_id":"seal_lab","display_name":"Lab","author":"Test","version":"1.0.0","depends_on":[]}],
        "place_graphs":[{"place_graph_id":"seal_lab/region","bundle_id":"seal_lab","substrate_id":"topo","display_name":"Region"}, {"place_graph_id":"seal_lab/dungeon","bundle_id":"seal_lab","substrate_id":"topo","display_name":"Dungeon","parent_graph_id":"seal_lab/region"}],
        "place_seals":[{"place_graph_id":"seal_lab/dungeon","bundle_id":"seal_lab","description":"Prepared"}]})
}
fn codes(value: Value) -> Vec<String> {
    validate_manifest(&manifest(value))
        .into_iter()
        .filter(|d| ["MF-VAL-378", "MF-VAL-374"].contains(&d.code.as_str()))
        .map(|d| d.code)
        .collect()
}
#[test]
fn own_root_is_valid() {
    assert!(codes(base()).is_empty());
}
#[test]
fn unknown_graph_is_fatal() {
    let mut v = base();
    v["place_seals"][0]["place_graph_id"] = json!("seal_lab/attic");
    assert_eq!(codes(v), ["MF-VAL-378"]);
}
#[test]
fn foreign_root_is_fatal_even_if_child_stamp_matches() {
    let mut v = base();
    v["place_graphs"][0]["bundle_id"] = json!("foreign");
    assert_eq!(codes(v), ["MF-VAL-378"]);
}
#[test]
fn instance_is_fatal() {
    let mut v = base();
    v["place_graph_instances"] = json!([{"blueprint_id":"seal_lab/dungeon","instance_id":"seal_lab/vault#2","anchor":{"kind":"place_node","id":"seal_lab/dungeon/center"}}]);
    v["place_seals"][0]["place_graph_id"] = json!("seal_lab/vault#2");
    assert_eq!(codes(v), ["MF-VAL-378"]);
}
#[test]
fn duplicate_seal_is_fatal() {
    let mut v = base();
    let seal = v["place_seals"][0].clone();
    v["place_seals"].as_array_mut().unwrap().push(seal);
    assert_eq!(codes(v), ["MF-VAL-378"]);
}
#[test]
fn export_only_fields_are_fatal_even_false_or_empty() {
    for (key, value) in [("broken", json!(false)), ("break_reason", json!(""))] {
        let mut v = base();
        v["place_seals"][0][key] = value;
        assert_eq!(codes(v), ["MF-VAL-378"]);
    }
}
#[test]
fn dm_fully_sealed_extent_warns_but_own_bundle_and_custom_role_do_not() {
    for (role, kind, reference, expected) in [
        ("dm", "place_graph", "seal_lab/dungeon", true),
        ("dm", "place_graph", "seal_lab/region", false),
        ("dm", "bundle", "seal_lab", false),
        ("dm", "bundle", "foreign", true),
        ("custom/dm", "place_graph", "seal_lab/dungeon", false),
    ] {
        let mut v = base();
        v["role_bindings"] = json!([{"binding_id":"test","bundle_id":"seal_lab","subject":"npc/test","role_id":role,"extent_kind_ref":kind,"extent_ref":reference,"description":"Synthetic DM binding"}]);
        assert_eq!(
            codes(v).contains(&"MF-VAL-374".into()),
            expected,
            "{role}/{kind}/{reference}"
        );
    }
}
#[test]
fn live_graph_facts_detect_instances_and_world_owned_roots_without_applyset_memory() {
    for (field, value, expected) in [
        ("bundle_id", json!("seal_lab"), false),
        ("bundle_id", json!(""), true),
        ("blueprint_id", json!("seal_lab/blueprint"), true),
    ] {
        let mut world = base();
        world["place_seals"] = json!([]);
        world["place_graphs"][0][field] = value;
        let live = LiveVocabulary::from_manifest(&manifest(world));
        let input = manifest(
            json!({"schema_version":"1.0.0","place_seals":[{"place_graph_id":"seal_lab/region","bundle_id":"seal_lab"}]}),
        );
        let diags = validate_manifest_with_world_for_apply(
            &input,
            &input,
            &Default::default(),
            Some(&live),
        );
        assert_eq!(
            diags.iter().any(|d| d.code == "MF-VAL-378"),
            expected,
            "{field}"
        );
    }
}

#[test]
fn dm_lint_resolves_authored_and_live_extent_aliases() {
    for (shape, reference, expected) in [
        ("Bundle", "foreign", true),
        ("Bundle", "seal_lab", false),
        ("PlaceGraph", "seal_lab/dungeon", true),
    ] {
        let mut v = base();
        v["extent_kind_definitions"] = json!([{"kind_id":"seal_lab/custom","bundle_id":"seal_lab","resolution_shape":shape,"description":"Authored alias"}]);
        v["role_bindings"] = json!([{"binding_id":"test","bundle_id":"seal_lab","subject":"npc/test","role_id":"dm","extent_kind_ref":"seal_lab/custom","extent_ref":reference,"description":"DM alias"}]);
        assert_eq!(codes(v.clone()).contains(&"MF-VAL-374".into()), expected);
        let mut live = LiveVocabulary::from_manifest(&manifest(v.clone()));
        live.extent_shapes
            .insert("seal_lab/custom".into(), shape.into());
        v["extent_kind_definitions"] = json!([]);
        let input = manifest(v);
        let augmented = input.clone();
        let found = validate_manifest_with_world_for_apply(
            &augmented,
            &input,
            &Default::default(),
            Some(&live),
        )
        .iter()
        .any(|d| d.code == "MF-VAL-374");
        assert_eq!(found, expected);
    }
}

#[test]
fn planned_graph_upserts_cannot_hide_live_instance_or_world_root_facts() {
    for (field, value) in [("bundle_id", ""), ("blueprint_id", "seal_lab/blueprint")] {
        let mut world = base();
        world["place_seals"] = json!([]);
        world["place_graphs"][0][field] = json!(value);
        let live = LiveVocabulary::from_manifest(&manifest(world));
        let mut input = base();
        input["place_seals"][0]["place_graph_id"] = json!("seal_lab/region");
        let input = manifest(input);
        let diags = validate_manifest_with_world_for_apply(
            &input,
            &input,
            &Default::default(),
            Some(&live),
        );
        assert!(
            diags.iter().any(|d| d.code == "MF-VAL-378"),
            "live {field} survives an authored upsert"
        );
    }
}
