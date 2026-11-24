//! Canonical row/set controls; full duty/action relationship admission is separate.
use serde_json::{json, Value};
use spellkave_schema::Manifest;
use spellkave_validate::dm_duty_references::CanonicalWorlds;

fn world(extra: Value) -> Manifest {
    let mut root = json!({"schema_version":"1.0.0"});
    root.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(root).unwrap()
}
fn action_world() -> Manifest {
    world(json!({
        "action_definitions":[{"action_id":"example/act","bundle_id":"example","required_capability":"example/write","display_name":"Act","on_success":"example/success"}],
        "consequence_templates":[{"template_id":"example/success","bundle_id":"example"}],
        "consequence_frame_writes":[
            {"template_id":"example/success","ordinal":2,"frame_write":{"ModifyResource":{"actor":"TargetActor","resource":"example/b","delta_source":"1"}}},
            {"template_id":"example/success","ordinal":0,"frame_write":{"ModifyResource":{"actor":"TargetActor","resource":"example/a","delta_source":"1"}}}
        ]
    }))
}

// K2: canonical closure must include foreign roots and use registered row keys.
#[test]
fn indexes_complete_roots_and_normalizes_only_identical_rows_and_ordered_sets() {
    let action = action_world();
    let foreign = world(
        json!({"role_aggregations":[{"bundle_id":"foreign","role_id":"example/reader","description":"Foreign contribution","capability_ids":["foreign/extra"]}]}),
    );
    let index = CanonicalWorlds::new(&[action.clone(), foreign.clone(), action]).unwrap();
    assert_eq!(index.rows("role_aggregations").unwrap().count(), 1);
    assert_eq!(
        index.rows("role_aggregations").unwrap().next().unwrap()["bundle_id"],
        "foreign"
    );
    let key = spellkave_schema::row_identity::row_key_of(
        "role_aggregations",
        &serde_json::to_value(&foreign.role_aggregations[0]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        index.row("role_aggregations", &key).unwrap()["role_id"],
        "example/reader"
    );
    let writes = index.action_writes("example/act").unwrap();
    assert_eq!(
        writes.iter().map(|row| row.ordinal).collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert_eq!(
        writes[0].frame_write["ModifyResource"]["resource"],
        "example/a"
    );
    assert!(index.row("entities", "absent").is_err());
    assert_eq!(
        index
            .keyed_row("unregistered_kind", &json!({}))
            .unwrap_err(),
        "dm-references: unregistered row kind unregistered_kind"
    );
    assert!(index.action_writes("absent").is_err());
    assert!(CanonicalWorlds::new(&[]).is_err());
}

// K2: metadata cannot choose a convenient copy when source identities conflict.
#[test]
fn refuses_conflicting_definitions_including_full_optional_columns() {
    for (table, row, column) in [
        (
            "metadata_namespaces",
            json!({"namespace":"example.data","bundle_id":"example","audience":"example/read","description":"First"}),
            "description",
        ),
        (
            "capability_definitions",
            json!({"capability_id":"example/read","bundle_id":"example","actor_kind":"entity","display_name":"Read","description":"First"}),
            "description",
        ),
        (
            "role_aggregations",
            json!({"bundle_id":"foreign","role_id":"example/reader","description":"First","capability_ids":["foreign/extra"]}),
            "description",
        ),
    ] {
        let first = world(json!({table:[row.clone()]}));
        let mut second = row.clone();
        second[column] = json!("Different");
        assert!(
            CanonicalWorlds::new(&[first.clone(), world(json!({table:[second.clone()]}))]).is_err(),
            "{table}"
        );
        assert!(
            CanonicalWorlds::new(&[world(json!({table:[row,second]}))]).is_err(),
            "{table}"
        );
        assert!(
            CanonicalWorlds::new(&[first.clone(), first]).is_ok(),
            "{table}"
        );
    }
}

// K2: set-level consequences cannot be assembled from incompatible partial roots.
#[test]
fn refuses_partial_or_conflicting_success_sets_and_missing_success_targets() {
    let first = action_world();
    let mut partial = first.clone();
    partial.consequence_frame_writes.pop();
    assert!(CanonicalWorlds::new(&[first.clone(), partial]).is_err());
    let mut changed = first.clone();
    changed.consequence_frame_writes[0].frame_write = json!({"Different":true});
    assert!(CanonicalWorlds::new(&[first.clone(), changed]).is_err());
    let mut duplicate = first.clone();
    duplicate.consequence_frame_writes[1].ordinal = duplicate.consequence_frame_writes[0].ordinal;
    assert!(CanonicalWorlds::new(&[duplicate]).is_err());
    let mut absent = first.clone();
    absent.consequence_templates.clear();
    assert!(CanonicalWorlds::new(&[absent])
        .unwrap()
        .action_writes("example/act")
        .is_err());
    let mut absent = first.clone();
    absent.consequence_frame_writes.clear();
    assert!(CanonicalWorlds::new(&[absent])
        .unwrap()
        .action_writes("example/act")
        .is_err());
    let mut duplicate = first.clone();
    duplicate
        .consequence_frame_writes
        .push(first.consequence_frame_writes[0].clone());
    assert!(CanonicalWorlds::new(&[duplicate]).is_ok());
}
