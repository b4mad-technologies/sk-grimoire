//! Spatial/selector facet controls, independent of full carrier admission.
use serde_json::json;
use spellkave_schema::Manifest;
use spellkave_validate::{
    dm_document_authority::PrincipalPlan,
    dm_duties::{parse_source_shape_json, DutySource},
    dm_duty_references::CanonicalWorlds,
    dm_route_references::{validate_duty_routes, validate_principal_action},
};

fn fixture() -> (DutySource, PrincipalPlan, Manifest) {
    let mut source =
        parse_source_shape_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    source.advancement.return_node_id = "example/return".into();
    source.companions.rogue.spawn_node_id = "example/spawn".into();
    let r = &source.routes;
    let binding = |id: &str, graph: &str| json!({"binding_id":id,"role_id":"example/dm","extent_kind_ref":"place_graph","extent_ref":graph,"description":"Route"});
    let plan = serde_json::from_value(json!({"schema_version":1,"bundleId":source.bundle_id,"bodyKey":source.body_key,
        "bindings":[binding(&r.region_binding_id,"example/region"),binding(&r.creation_binding_id,"example/creation"),binding(&r.advancement_binding_id,"example/advancement"),binding(&r.graph_author_binding_id,"example/region"),
            {"binding_id":r.bundle_author_binding_id,"role_id":"example/author","extent_kind_ref":"bundle","extent_ref":source.bundle_id,"description":"Author"}],
        "retiredBindings":[],"sightGrants":[],"fictionCapabilities":[],"seatCapability":"example/seat",
        "roleCapabilities":{"example/dm":["example/act"],"example/author":["example/author"]}})).unwrap();
    let node = |id: &str, graph: &str| json!({"node_id":id,"bundle_id":source.bundle_id,"place_graph_id":graph,"substrate_id":"example","local_id":id,"display_name":"Node"});
    let graph = |id: &str, parent: &str| json!({"place_graph_id":id,"bundle_id":source.bundle_id,"substrate_id":"example","display_name":"Graph","parent_graph_id":parent});
    let entity = |id: &str, definition: &str, node: &str| json!({"entity_key":id,"bundle_id":source.bundle_id,"entity_def_id":definition,"place_node_id":node,"name":"Body","kind":"npc"});
    let world = serde_json::from_value(json!({"schema_version":"1.0.0",
        "place_graphs":[graph("example/region",""),graph("example/room","example/region"),graph("example/creation",""),graph("example/advancement","")],
        "place_nodes":[node(&source.vessels.rest_node_id,"example/room"),node(&source.vessels.creation_station_node_id,"example/creation"),node(&source.advancement.limbo_node_id,"example/advancement"),node(&source.advancement.return_node_id,"example/region"),node(&source.companions.rogue.spawn_node_id,"example/region")],
        "entities":[entity(&source.body_key,"example/body",&source.vessels.rest_node_id),entity(&source.vessels.candidate_key,&source.vessels.candidate_blueprint_id,&source.vessels.creation_station_node_id)],
        "capability_definitions":[{"bundle_id":"example","capability_id":"example/act","actor_kind":"entity","display_name":"Act","description":"Act"}],
        "action_definitions":[{"bundle_id":"example","action_id":"example/action","required_capability":"example/act","on_success":"example/success","display_name":"Act"},
            {"bundle_id":"example","action_id":source.creation.open_seat_action_id,"required_capability":"example/act","on_success":"example/spawn-consequence","display_name":"Spawn"}],
        "entity_definitions":[{"bundle_id":source.bundle_id,"entity_def_id":source.vessels.candidate_blueprint_id,"display_name":"Candidate"}],
        "consequence_templates":[{"bundle_id":"example","template_id":"example/spawn-consequence","effect_refs_csv":"example/spawn"}],
        "effect_definitions":[{"bundle_id":"example","effect_def_id":"example/spawn","primitives_csv":"Spawn","parameters_schema_json":json!({"blueprint_id":source.vessels.candidate_blueprint_id,"location_ref":{"kind":"binding","id":"TargetSpatialRef"},"unbound_actor":true}).to_string()}]
    })).unwrap();
    (source, plan, world)
}

// K2: exact original selectors and authored graph ancestry separate duties;
// swapping equally privileged bindings must still fail the spatial relation.
#[test]
fn original_routes_match_graph_ancestry_candidate_home_and_plan_identity() {
    let (mut source, plan, world) = fixture();
    let worlds = CanonicalWorlds::new(&[world]).unwrap();
    validate_duty_routes(&source, &plan, &worlds).unwrap();
    std::mem::swap(
        &mut source.routes.region_binding_id,
        &mut source.routes.creation_binding_id,
    );
    assert!(validate_duty_routes(&source, &plan, &worlds).is_err());
    let (source, mut plan, world) = fixture();
    let worlds = CanonicalWorlds::new(&[world]).unwrap();
    plan.body_key.push_str("-other");
    assert!(validate_duty_routes(&source, &plan, &worlds).is_err());
}

// K2: a reader or wrong role cannot become an act selector, and malformed
// source ancestry cannot pass by relying on an unbounded parent walk.
#[test]
fn refuses_reader_selectors_unreachable_actions_and_cyclic_ancestry() {
    let (source, mut plan, mut world) = fixture();
    let worlds = CanonicalWorlds::new(std::slice::from_ref(&world)).unwrap();
    validate_principal_action(
        "example/action",
        &source.routes.region_binding_id,
        &plan,
        &worlds,
    )
    .unwrap();
    assert!(validate_principal_action(
        "example/action",
        &source.routes.bundle_author_binding_id,
        &plan,
        &worlds
    )
    .is_err());
    assert!(validate_principal_action("example/action", "example/reader", &plan, &worlds).is_err());
    let mut reader = plan.bindings[0].clone();
    reader.binding_id = "example/reader".into();
    plan.bindings.push(reader);
    assert!(validate_duty_routes(&source, &plan, &worlds).is_err());
    plan.bindings.pop();
    world
        .place_graphs
        .iter_mut()
        .find(|row| row.place_graph_id == "example/room")
        .unwrap()
        .parent_graph_id = "example/room".into();
    let worlds = CanonicalWorlds::new(&[world]).unwrap();
    assert!(validate_duty_routes(&source, &plan, &worlds).is_err());
}
