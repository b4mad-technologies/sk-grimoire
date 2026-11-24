use spellkave_schema::{row_identity::manifest_row_identities, Manifest};

#[test]
fn place_seal_round_trip_keeps_natural_key_and_break_overlay() {
    let source = r#"schema_version = "1.0.0"
[[place_seals]]
place_graph_id = "seal_lab/dungeon"
bundle_id = "seal_lab"
description = "Prepared dungeon"
broken = true
break_reason = "Author accepted the breach"
"#;
    let manifest: Manifest = toml::from_str(source).unwrap();
    let row = &manifest.place_seals[0];
    assert_eq!(row.place_graph_id, "seal_lab/dungeon");
    assert_eq!(row.broken, Some(true));
    let exported = toml::to_string(&manifest).unwrap();
    let restored: Manifest = toml::from_str(&exported).unwrap();
    assert_eq!(restored.place_seals, manifest.place_seals);
    assert!(manifest_row_identities(&restored)
        .rows
        .iter()
        .any(|row| row.kind == "place_seals" && row.key == "seal_lab/dungeon"));
}

#[test]
fn warden_is_an_elective_proposal_without_automatic_break_authority() {
    let manifest: Manifest = toml::from_str(spellkave_schema::CORE_BOOTSTRAP_MANIFEST).unwrap();
    assert!(manifest
        .capability_definitions
        .iter()
        .any(|row| row.capability_id == "frontier.break_seal"));
    let warden = manifest
        .role_definitions
        .iter()
        .find(|row| row.role_id == "warden")
        .unwrap();
    assert_eq!(warden.capability_ids, ["frontier.break_seal"]);
    assert!(manifest
        .role_definitions
        .iter()
        .filter(|row| row.role_id != "warden")
        .all(|row| !row
            .capability_ids
            .iter()
            .any(|id| id == "frontier.break_seal")));
    assert!(manifest.role_bindings.is_empty());
    assert!(manifest.actor_capabilities.is_empty());
}
