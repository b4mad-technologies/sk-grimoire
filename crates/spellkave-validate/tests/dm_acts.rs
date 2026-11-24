use serde_json::json;
use spellkave_schema::{ConsequenceFrameWriteRow, Manifest};
use spellkave_validate::{validate_manifest, Severity};

fn manifest(write: serde_json::Value) -> Manifest {
    let mut m = Manifest::default();
    m.consequence_templates.push(
        serde_json::from_value(json!({
            "template_id":"test/dm", "bundle_id":"test"
        }))
        .unwrap(),
    );
    m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
        template_id: "test/dm".into(),
        ordinal: 0,
        frame_write: write,
    });
    m
}
fn refused(m: &Manifest, code: &str) -> bool {
    validate_manifest(m)
        .iter()
        .any(|d| d.code == code && d.severity == Severity::Fatal)
}
#[test]
fn dm_tag_selection_cannot_write_the_player_seat_tag() {
    let mut m = manifest(json!({"SetEntityTag":{
        "entity":"TargetEntity", "tag":{"allowed_ids":["srd.player_character"]},"present":true
    }}));
    declare_tag(&mut m, "srd.player_character", "srd");
    assert!(refused(&m, "MF-VAL-383"));
}
#[test]
fn dm_tag_selection_cannot_write_a_party_extent_tag() {
    let mut m = manifest(json!({"SetEntityTag":{
        "entity":"TargetEntity", "tag":{"allowed_ids":["test.party.heroes"]},"present":false
    }}));
    m.role_bindings.push(
        serde_json::from_value(json!({
            "binding_id":"test/dm", "bundle_id":"test", "subject":"test/aldwin", "role_id":"dm",
            "extent_kind_ref":"party", "extent_ref":"test.party.heroes", "description":"test"
        }))
        .unwrap(),
    );
    declare_tag(&mut m, "test.party.heroes", "test");
    assert!(refused(&m, "MF-VAL-383"));
}
#[test]
fn dm_selections_require_nonempty_declared_ids() {
    for ids in [json!([]), json!([""]), json!(["test.undeclared"])] {
        let m = manifest(
            json!({"SetEntityTag":{"entity":"TargetEntity","tag":{"allowed_ids":ids},"present":true}}),
        );
        assert!(refused(&m, "MF-VAL-388"), "{ids}");
    }
}
#[test]
fn dm_text_bounds_are_admitted_before_execution() {
    for bound in [0, 4097] {
        let m = manifest(
            json!({"SetEntityText":{"entity":"TargetEntity","destination":"EntityName","max_chars":bound}}),
        );
        assert!(refused(&m, "MF-VAL-384"));
    }
    for bound in [1, 4096] {
        let m = manifest(
            json!({"SetEntityText":{"entity":"TargetEntity","destination":"EntityName","max_chars":bound}}),
        );
        assert!(!refused(&m, "MF-VAL-384"));
    }
}
#[test]
fn literal_menu_labels_obey_the_dispatch_bounds() {
    for options in [json!(["x".repeat(121)]), json!(vec!["x"; 17])] {
        let m =
            manifest(json!({"Inscribe":{"payload_json": json!({"options": options}).to_string()}}));
        assert!(refused(&m, "MF-VAL-385"));
    }
}

#[test]
fn dm_selection_respects_the_owning_consequence_dependency_closure() {
    let mut m = manifest(json!({"SetEntityTag": {
        "entity": "TargetEntity", "tag": {"allowed_ids": ["other.identity"]}, "present": true
    }}));
    for bundle in ["test", "other"] {
        m.bundles.push(serde_json::from_value(json!({
            "bundle_id": bundle, "display_name": bundle, "author": "test", "version": "1", "depends_on": []
        })).unwrap());
    }
    m.tag_value_definitions.push(serde_json::from_value(json!({
        "tag": "other.identity", "prefix": "other", "bundle_id": "other", "description": "Scoped identity."
    })).unwrap());
    assert!(refused(&m, "MF-VAL-214"));
    m.bundles[0].depends_on.push("other".into());
    assert!(!refused(&m, "MF-VAL-214"));
    assert!(!refused(&m, "MF-VAL-388"));
}

#[test]
fn dm_insert_selection_cannot_also_supply_a_literal_definition() {
    let m = manifest(json!({"InsertItem": {
        "item_def_id": "test/item", "selection": {"allowed_ids": ["test/item"]},
        "count": 1, "destination": {"ActorInventory": {"actor_ref": "TargetActor"}}
    }}));
    let mut diags = Vec::new();
    spellkave_validate::dm_acts::validate(&m, &mut diags);
    assert!(diags
        .iter()
        .any(|d| d.code == "MF-VAL-388" && d.message.contains("literal")));
}

#[test]
fn unbound_actor_refuses_a_place_graph_blueprint() {
    let mut m = Manifest::default();
    m.place_graphs.push(
        serde_json::from_value(json!({
            "place_graph_id": "test/rooms", "bundle_id": "test", "display_name": "Rooms",
            "substrate_id": "indoor", "anchor_node_id": "test/rooms/root"
        }))
        .unwrap(),
    );
    m.effect_definitions.push(serde_json::from_value(json!({
        "effect_def_id": "test/spawn", "bundle_id": "test", "primitives_csv": "Spawn",
        "parameters_schema_json": json!({"blueprint_id": "test/rooms", "unbound_actor": true}).to_string()
    })).unwrap());
    assert!(refused(&m, "MF-VAL-386"));
    m.effect_definitions[0].parameters_schema_json =
        json!({"blueprint_id": "test/rooms", "unbound_actor": false}).to_string();
    assert!(!refused(&m, "MF-VAL-386"));
}

#[test]
fn authored_acts_reference_exact_genesis_capabilities_without_redeclaring_them() {
    let mut m = Manifest::default();
    m.action_definitions.push(
        serde_json::from_value(json!({
            "action_id":"test/name", "bundle_id":"test", "display_name":"Name",
            "required_capability":"frontier.set_state", "on_success":"test/success"
        }))
        .unwrap(),
    );
    assert!(!refused(&m, "MF-VAL-265"));
    m.action_definitions[0].required_capability = "frontier.set_sttae".into();
    assert!(refused(&m, "MF-VAL-265"));
}

#[test]
fn reconstructed_bootstrap_definitions_are_references_but_incoming_definitions_are_checked() {
    let bootstrap = spellkave_format::parse_manifest(spellkave_schema::CORE_BOOTSTRAP_MANIFEST)
        .unwrap()
        .manifest;
    let mut incoming = Manifest::default();
    incoming.bundles.push(serde_json::from_value(json!({
        "bundle_id": "test", "display_name": "Test", "author": "test", "version": "1", "depends_on": ["core"]
    })).unwrap());
    let mut augmented = incoming.clone();
    augmented
        .capability_definitions
        .extend(bootstrap.capability_definitions);
    let validate = |references: &Manifest, applying: &Manifest| {
        spellkave_validate::validate_manifest_with_world_for_apply(
            references,
            applying,
            &Default::default(),
            Some(&Default::default()),
        )
    };
    assert!(!validate(&augmented, &incoming)
        .iter()
        .any(|d| d.code == "MF-VAL-291"));
    incoming.capability_definitions.push(serde_json::from_value(json!({
        "capability_id": "frontier.forged", "actor_kind": "any", "bundle_id": "test", "display_name": "Forged", "description": "Incoming definition."
    })).unwrap());
    augmented
        .capability_definitions
        .push(incoming.capability_definitions[0].clone());
    assert!(validate(&augmented, &incoming)
        .iter()
        .any(|d| d.code == "MF-VAL-291"
            && d.severity == Severity::Warning
            && d.message.contains("frontier.forged")));
}

#[test]
fn headerless_selection_resolves_live_conditions_items_and_tags_with_dependency_scope() {
    let mut world = Manifest::default();
    for (bundle, dependencies) in [("test", vec!["other"]), ("other", vec![])] {
        world.bundles.push(serde_json::from_value(json!({"bundle_id": bundle, "display_name": bundle, "author": "test", "version": "1", "depends_on": dependencies})).unwrap());
    }
    world.tag_value_definitions.push(serde_json::from_value(json!({"tag": "other.identity", "prefix": "other", "bundle_id": "other", "description":"Identity"})).unwrap());
    world.condition_definitions.push(serde_json::from_value(json!({"condition_definition_id": "other/phase", "bundle_id": "other", "display_name":"Phase", "gate_predicate_source":"true"})).unwrap());
    world.entity_definitions.push(
        serde_json::from_value(
            json!({"entity_def_id": "other/item", "bundle_id": "other", "display_name":"Item"}),
        )
        .unwrap(),
    );
    let writes = [
        json!({"SetEntityTag":{"entity":"TargetEntity", "tag":{"allowed_ids":["other.identity"]},"present":true}}),
        json!({"ApplyStatus":{"target":"TargetEntity", "selection":{"allowed_ids":["other/phase"]}}}),
        json!({"InsertItem":{"selection":{"allowed_ids":["other/item"]},"count":1}}),
    ];
    for write in writes {
        let incoming = manifest(write);
        let mut diags = vec![];
        spellkave_validate::dm_acts::validate_with_world(
            &incoming,
            Some(&spellkave_validate::LiveVocabulary::from_manifest(&world)),
            &mut diags,
        );
        assert!(
            !diags
                .iter()
                .any(|d| d.code == "MF-VAL-388" || d.code == "MF-VAL-214"),
            "{diags:?}"
        );
        world.bundles[0].depends_on.clear();
        diags.clear();
        spellkave_validate::dm_acts::validate_with_world(
            &incoming,
            Some(&spellkave_validate::LiveVocabulary::from_manifest(&world)),
            &mut diags,
        );
        assert!(diags.iter().any(|d| d.code == "MF-VAL-214"), "{diags:?}");
        world.bundles[0].depends_on.push("other".into());
    }
}

#[test]
fn live_party_extents_protect_tags_on_headerless_selections() {
    let mut world = Manifest::default();
    world.actor_capabilities.push(serde_json::from_value(json!({"actor_entity_key":"test/hero", "capability_id":"test/sight", "bundle_id":"test", "source":"test", "extent_kind_ref":"party", "extent_ref":"test.party"})).unwrap());
    declare_tag(&mut world, "test.party", "test");
    let incoming = manifest(
        json!({"SetEntityTag":{"entity":"TargetEntity", "tag":{"allowed_ids":["test.party"]},"present":true}}),
    );
    let mut diags = vec![];
    spellkave_validate::dm_acts::validate_with_world(
        &incoming,
        Some(&spellkave_validate::LiveVocabulary::from_manifest(&world)),
        &mut diags,
    );
    assert!(diags.iter().any(|d| d.code == "MF-VAL-383"), "{diags:?}");
}

fn declare_tag(m: &mut Manifest, tag: &str, owner: &str) {
    m.tag_value_definitions.push(
        serde_json::from_value(json!({
            "tag": tag, "prefix": owner, "bundle_id": owner, "description": "Admission proof"
        }))
        .unwrap(),
    );
}

#[test]
fn authority_tag_diagnostic_does_not_disclose_out_of_scope_or_undeclared_party_extents() {
    let mut world = Manifest::default();
    for bundle in ["test", "private"] {
        world.bundles.push(serde_json::from_value(json!({ "bundle_id": bundle, "display_name": bundle, "author": "test", "version": "1", "depends_on": [] })).unwrap());
    }
    declare_tag(&mut world, "private.party", "private");
    let mut live = spellkave_validate::LiveVocabulary::from_manifest(&world);
    live.dm
        .party_tags
        .extend(["private.party".into(), "undeclared.party".into()]);
    for tag in ["private.party", "undeclared.party"] {
        let incoming = manifest(
            json!({"SetEntityTag": {"entity":"TargetEntity", "tag":{"allowed_ids":[tag]}, "present": true}}),
        );
        let mut diags = vec![];
        spellkave_validate::dm_acts::validate_with_world(&incoming, Some(&live), &mut diags);
        assert!(!diags.iter().any(|d| d.code == "MF-VAL-383"), "{diags:?}");
        assert!(diags.iter().any(|d| d.code
            == if tag == "private.party" {
                "MF-VAL-214"
            } else {
                "MF-VAL-388"
            }));
    }
}

#[test]
fn reconstructed_consequence_writes_are_not_revalidated_as_incoming_edits() {
    let mut references = manifest(
        json!({"SetEntityTag": {"entity":"TargetEntity", "tag":{"allowed_ids":["test.party"]}, "present": true}}),
    );
    declare_tag(&mut references, "test.party", "test");
    let applying = Manifest::default();
    let mut live = spellkave_validate::LiveVocabulary::default();
    live.dm.party_tags.insert("test.party".into());
    let mut diags = vec![];
    spellkave_validate::dm_acts::validate_for_apply(
        &references,
        &applying,
        Some(&live),
        &mut diags,
    );
    assert!(diags.is_empty(), "{diags:?}");
    spellkave_validate::dm_acts::validate_with_world(&references, Some(&live), &mut diags);
    assert!(diags.iter().any(|d| d.code == "MF-VAL-383"));
}
