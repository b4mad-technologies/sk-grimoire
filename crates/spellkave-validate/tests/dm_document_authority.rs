//! Authority controls for the carrier compiler, not live grant/readiness proof.
use serde_json::{json, Value};
use spellkave_validate::dm_document_authority::{admit_read_authority, unique_json};

fn input() -> Value {
    json!({
        "schema_version":1,"bundle_id":"example","body_key":"example/guide",
        "catalog_namespace":"example.quests",
        "principal_plan":{
            "schema_version":1,"bundleId":"example","bodyKey":"example/guide",
            "bindings":[
                {"binding_id":"example/region","role_id":"dm","extent_kind_ref":"place_graph","extent_ref":"example/region","description":"Region"},
                {"binding_id":"example/creation","role_id":"dm","extent_kind_ref":"place_graph","extent_ref":"example/creation","description":"Creation"},
                {"binding_id":"example/advance","role_id":"dm","extent_kind_ref":"place_graph","extent_ref":"example/advance","description":"Advancement"},
                {"binding_id":"example/author","role_id":"example/author","extent_kind_ref":"place_graph","extent_ref":"example/region","description":"Author"},
                {"binding_id":"example/bundle-author","role_id":"example/author","extent_kind_ref":"bundle","extent_ref":"example","description":"Bundle author"}
            ],
            "retiredBindings":[{"binding_id":"example/old-host","role_id":"npc-host","extent_kind_ref":"party","extent_ref":"example.heroes","description":"Retired"}],
            "sightGrants":[
                {"capability_id":"example/watch","extent_kind_ref":"place_graph","extent_ref":"example/region","elevation_class":"full"},
                {"capability_id":"example/watch-limbo","extent_kind_ref":"place_graph","extent_ref":"example/creation","elevation_class":""}
            ],
            "fictionCapabilities":["example/seat","example/talk"],"seatCapability":"example/seat",
            "roleCapabilities":{"dm":["frontier.set_state"],"example/author":["frontier.apply_manifest","frontier.open_session"]}
        },
        "reads":{
            "reader_role_id":"example/data/reader","reader_binding_id":"example/guide-reader",
            "catalog_read_capability_id":"example/data/catalog",
            "duty_read_capability_id":"example/data/duties",
            "presentation_read_capability_id":"example/data/presentation"
        }
    })
}
fn worlds() -> Value {
    json!([{
        "schema_version":"1.0.0",
        "bundles":[{"bundle_id":"example"}],
        "entities":[{"bundle_id":"example","entity_key":"example/guide"}],
        "capability_definitions":[{"bundle_id":"example","capability_id":"example/data/catalog","actor_kind":"entity","display_name":"Catalog","description":"Read catalog"}],
        "metadata_namespaces":[{"bundle_id":"example","namespace":"example.quests","audience":"example/data/catalog","on_demand":false}],
        "role_definitions":[
            {"bundle_id":"core","role_id":"dm","display_name":"DM","description":"Authority","capability_ids":["frontier.set_state"]},
            {"bundle_id":"example","role_id":"example/author","display_name":"Author","description":"Author","capability_ids":["frontier.apply_manifest"]}
        ],
        "role_aggregations":[{"bundle_id":"foreign","role_id":"example/author","description":"Observed contribution","capability_ids":["frontier.open_session"]}],
        "actor_capabilities":[{"actor_entity_key":"example/other-giver","capability_id":"example/data/catalog"}]
    }])
}
fn refuses(input: &Value, worlds: &Value) {
    assert!(admit_read_authority(&input.to_string(), &worlds.to_string()).is_err());
}

// K2: Pdocs must add exactly the narrow reader without rewriting P authority.
#[test]
fn derives_one_reader_and_preserves_every_original_plan_field() {
    let input = input();
    let admitted = admit_read_authority(&input.to_string(), &worlds().to_string()).unwrap();
    let mut plan = serde_json::to_value(admitted.principal_plan()).unwrap();
    let binding = plan["bindings"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(binding["binding_id"], input["reads"]["reader_binding_id"]);
    assert_eq!(binding["extent_kind_ref"], "bundle");
    assert_eq!(binding["extent_ref"], "example");
    assert!(binding.get("subject").is_none());
    let caps = plan["roleCapabilities"]
        .as_object_mut()
        .unwrap()
        .remove("example/data/reader")
        .unwrap();
    assert_eq!(caps, json!(["example/data/catalog", "example/data/duties"]));
    assert_eq!(plan, input["principal_plan"]);
    let rows = admitted.definition_rows();
    // The intermediate contribution uses the existing manifest row schema.
    let _: spellkave_schema::Manifest = serde_json::from_value(rows.clone()).unwrap();
    assert!(rows.get("role_bindings").is_none());
    assert!(rows.get("metadata_attachments").is_none());
    assert_eq!(rows["actor_capabilities"].as_array().unwrap().len(), 1);
    assert_eq!(
        rows["actor_capabilities"][0]["actor_entity_key"],
        "example/guide"
    );
    assert_eq!(
        rows["actor_capabilities"][0]["capability_id"],
        "example/data/presentation"
    );
    assert_eq!(
        rows["metadata_namespaces"][0]["namespace"],
        "npc/dm/presentation/v1"
    );
    assert_eq!(
        rows["metadata_namespaces"][1]["namespace"],
        "npc/dm/duties/v1"
    );
    assert_eq!(rows["capability_definitions"].as_array().unwrap().len(), 2);
    // Both new read capabilities are service-only audiences.
    assert_eq!(
        rows["capability_definition_tag"],
        json!([
            {"bundle_id":"example","capability_id":"example/data/duties",
             "tag":spellkave_schema::SERVICE_ONLY_AUDIENCE_TAG,"source":"manifest"},
            {"bundle_id":"example","capability_id":"example/data/presentation",
             "tag":spellkave_schema::SERVICE_ONLY_AUDIENCE_TAG,"source":"manifest"}
        ])
    );
    assert_eq!(rows["role_definitions"].as_array().unwrap().len(), 1);
    assert_eq!(
        rows["role_definitions"][0]["capability_ids"],
        json!(["example/data/catalog", "example/data/duties"])
    );
    assert!(rows["capability_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["capability_id"] != "example/data/catalog"));
    // Identical dependency rows normalize; a conflicting definition does not.
    let mut repeated = worlds();
    let same_world = repeated[0].clone();
    repeated.as_array_mut().unwrap().push(same_world);
    assert!(admit_read_authority(&input.to_string(), &repeated.to_string()).is_ok());
}

// K2: new reader IDs/capabilities cannot alias act, drive, sight or body grants.
#[test]
fn refuses_missing_overlapping_malformed_and_extended_source_authority() {
    for (path, bad) in [
        ("/schema_version", json!(2)),
        ("/principal_plan/schema_version", json!(2)),
        ("/body_key", json!("example/other")),
        ("/bundle_id", json!("foreign")),
        ("/catalog_namespace", json!("npc/dm/duties/v1")),
        ("/reads/reader_role_id", json!("dm")),
        ("/reads/reader_binding_id", json!("example/region")),
        ("/reads/reader_binding_id", json!("example/old-host")),
        ("/reads/duty_read_capability_id", json!("frontier.drive")),
        ("/reads/duty_read_capability_id", json!("example/watch")),
        ("/reads/duty_read_capability_id", json!("example/seat")),
        (
            "/reads/duty_read_capability_id",
            json!("example/data/catalog"),
        ),
        ("/reads/reader_role_id", json!("example/data/presentation")),
        ("/reads/reader_binding_id", json!("example/author")),
        (
            "/principal_plan/roleCapabilities/dm",
            json!(["frontier.drive"]),
        ),
        ("/principal_plan/roleCapabilities/dm", json!([])),
        ("/principal_plan/bindings", json!([])),
        ("/principal_plan/seatCapability", json!("")),
    ] {
        let mut altered = input();
        *altered.pointer_mut(path).unwrap() = bad;
        refuses(&altered, &worlds());
    }
    for path in [
        "",
        "/reads",
        "/principal_plan",
        "/principal_plan/bindings/0",
        "/principal_plan/sightGrants/0",
    ] {
        let mut altered = input();
        altered
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), json!(true));
        refuses(&altered, &worlds());
    }
    let mut missing = input();
    missing["reads"]
        .as_object_mut()
        .unwrap()
        .remove("duty_read_capability_id");
    refuses(&missing, &worlds());
    let mut duplicate = input();
    duplicate["principal_plan"]["bindings"][1] = duplicate["principal_plan"]["bindings"][0].clone();
    refuses(&duplicate, &worlds());
}

// K2: static closure includes foreign aggregation and all read-capability users.
#[test]
fn refuses_role_widening_collisions_other_actor_grants_and_read_to_act_leakage() {
    for table in ["role_definitions", "role_aggregations"] {
        let mut changed = worlds();
        changed[0][table][0]["capability_ids"]
            .as_array_mut()
            .unwrap()
            .push(json!("frontier.drive"));
        refuses(&input(), &changed);
        let mut changed = worlds();
        changed[0][table].as_array_mut().unwrap().push(json!({"bundle_id":"foreign","role_id":"example/data/reader","description":"Foreign widening","display_name":"Reader","capability_ids":["frontier.drive"]}));
        // Aggregations have no display_name, so also test the well-shaped row.
        if table == "role_aggregations" {
            changed[0][table]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("display_name");
        }
        refuses(&input(), &changed);
    }
    for cap in [
        "example/data/catalog",
        "example/data/duties",
        "example/data/presentation",
    ] {
        let mut changed = worlds();
        changed[0]["action_definitions"] =
            json!([{"action_id":"example/privileged","required_capability":cap}]);
        refuses(&input(), &changed);
    }
    for actor in ["example/guide", "example/other"] {
        for cap in ["example/data/duties", "example/data/presentation"] {
            let mut changed = worlds();
            changed[0]["actor_capabilities"]
                .as_array_mut()
                .unwrap()
                .push(json!({"actor_entity_key":actor,"capability_id":cap}));
            refuses(&input(), &changed);
        }
    }
    let mut changed = worlds();
    changed[0]["role_aggregations"][0]["capability_ids"] = json!(["example/data/duties"]);
    refuses(&input(), &changed);
    for (path, bad) in [
        ("/0/metadata_namespaces/0/audience", json!("Public")),
        ("/0/metadata_namespaces/0/on_demand", json!(true)),
        ("/0/capability_definitions/0/bundle_id", json!("foreign")),
        (
            "/0/capability_definitions/0/actor_kind",
            json!("place_node"),
        ),
        ("/0/entities", json!([])),
        ("/0/bundles", json!([])),
        ("/0/role_definitions", json!([])),
        ("/0/role_aggregations", json!([])),
    ] {
        let mut changed = worlds();
        *changed.pointer_mut(path).unwrap() = bad;
        refuses(&input(), &changed);
    }
}

// K2: old applied P/Pdocs must not be reinterpreted as a fresh authority input.
#[test]
fn refuses_existing_principal_or_body_reader_materialization() {
    for id in ["example/region", "example/old-host", "example/guide-reader"] {
        let mut changed = worlds();
        changed[0]["role_bindings"] = json!([{"binding_id":id,"subject":"retained-identity"}]);
        refuses(&input(), &changed);
    }
    let mut changed = worlds();
    changed[0]["actor_capabilities"]
        .as_array_mut()
        .unwrap()
        .push(json!({"actor_entity_key":"example/guide","capability_id":"example/data/catalog"}));
    refuses(&input(), &changed);
    let mut changed = worlds();
    changed[0]["metadata_namespaces"]
        .as_array_mut()
        .unwrap()
        .push(json!({"namespace":"npc/dm/duties/v1","audience":"example/data/duties"}));
    refuses(&input(), &changed);
}

// K2: raw duplicate refusal must precede canonicalization at every depth.
#[test]
fn refuses_duplicate_nested_keys_trailing_json_and_oversized_job_input() {
    for text in [
        r#"{"key":1,"key":2}"#,
        r#"[{"nested":{"key":1,"key":2}}]"#,
        r#"{"key":1}{}"#,
    ] {
        assert!(unique_json(text).is_err());
    }
    let raw = input().to_string();
    let duplicate = raw.replacen(
        "\"roleCapabilities\":{",
        "\"roleCapabilities\":{\"dm\":[],",
        1,
    );
    assert!(admit_read_authority(&duplicate, &worlds().to_string()).is_err());
    assert!(unique_json(
        &" ".repeat(spellkave_validate::dm_document_authority::MAX_ARTIFACT_BYTES + 1)
    )
    .is_err());
    assert_eq!(
        unique_json(r#"[null,true,false,-1,1,1.5,"text"]"#).unwrap(),
        json!([null, true, false, -1, 1, 1.5, "text"])
    );
}

// K2: canonical authority rows are raw inputs too; duplicate map fields cannot
// hide widening before the exact merged-role comparison.
#[test]
fn canonical_world_json_requires_unique_keys_objects_and_byte_bound() {
    let raw = input().to_string();
    let duplicate = worlds().to_string().replacen(
        "\"capability_ids\":[",
        "\"capability_ids\":[\"frontier.drive\"],\"capability_ids\":[",
        1,
    );
    assert!(admit_read_authority(&raw, &duplicate).is_err());
    for bad in ["{}", "[]", "[null]", "[false]", "[{}]", "{", "[{}]{}"] {
        assert!(admit_read_authority(&raw, bad).is_err());
    }
    assert!(admit_read_authority(
        &raw,
        &" ".repeat(spellkave_validate::dm_document_authority::MAX_ARTIFACT_BYTES + 1)
    )
    .is_err());
}

// K2: full-column projection cannot hide conflicting definition identities or
// coerce a present namespace option into the absent/false default.
#[test]
fn refuses_full_column_collisions_and_present_invalid_namespace_options() {
    for (table, column) in [
        ("capability_definitions", "display_name"),
        ("capability_definitions", "description"),
        ("capability_definitions", "default_visibility"),
        ("metadata_namespaces", "description"),
    ] {
        let mut changed = worlds();
        let mut conflict = changed[0][table][0].clone();
        conflict[column] = json!("different");
        changed[0][table].as_array_mut().unwrap().push(conflict);
        refuses(&input(), &changed);
    }
    for value in [json!(true), Value::Null, json!("false"), json!(0)] {
        let mut changed = worlds();
        changed[0]["metadata_namespaces"][0]["on_demand"] = value;
        refuses(&input(), &changed);
    }
    let mut absent = worlds();
    absent[0]["metadata_namespaces"][0]
        .as_object_mut()
        .unwrap()
        .remove("on_demand");
    assert!(admit_read_authority(&input().to_string(), &absent.to_string()).is_ok());
}
