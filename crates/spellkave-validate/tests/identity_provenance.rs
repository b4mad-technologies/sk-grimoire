use serde_json::json;
use spellkave_schema::Manifest;
use spellkave_validate::validate_manifest;

fn codes(primary: Option<&str>, backup: &str) -> Vec<String> {
    let mut value = json!({"schema_version":"1.0.0", "actor_identities": [
        {"entity_key":"guard", "bundle_id":"lab", "identity":"backup", "role":"co-pilot", "provenance":backup}
    ]});
    if let Some(provenance) = primary {
        value["actor_identities"].as_array_mut().unwrap().push(json!({
            "entity_key":"guard", "bundle_id":"lab", "identity":"primary", "role":"primary", "provenance":provenance
        }));
    }
    let manifest: Manifest = serde_json::from_value(value).unwrap();
    validate_manifest(&manifest)
        .into_iter()
        .filter(|d| ["MF-VAL-376", "MF-VAL-377"].contains(&d.code.as_str()))
        .map(|d| d.code)
        .collect()
}

#[test]
fn authored_backup_requires_service_primary_when_primary_is_known() {
    assert!(codes(Some("service"), "service").is_empty());
    assert_eq!(codes(Some("person"), "service"), ["MF-VAL-377"]);
    // No primary in this fragment: live commit owns that decision.
    assert!(codes(None, "service").is_empty());
}

#[test]
fn invalid_provenance_is_refused_and_omission_stays_optional() {
    assert_eq!(codes(None, "robot"), ["MF-VAL-376"]);
    let row: spellkave_schema::ActorIdentityRow = serde_json::from_value(json!({
        "entity_key":"guard", "bundle_id":"lab", "identity":"host", "role":"primary"
    }))
    .unwrap();
    assert_eq!(row.provenance, None);
}

#[test]
fn a_service_primary_never_cancels_a_person_primaries_consent_requirement() {
    let manifest:Manifest = serde_json::from_value(json!({"schema_version":"1.0.0", "actor_identities":[
        {"entity_key":"guard","bundle_id":"lab","identity":"human","role":"primary","provenance":"person"},
        {"entity_key":"guard","bundle_id":"lab","identity":"host","role":"primary","provenance":"service"},
        {"entity_key":"guard","bundle_id":"lab","identity":"backup","role":"co-pilot","provenance":"service"}
    ]})).unwrap();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-377"));
}

#[test]
fn authored_consent_cannot_cross_a_person_narrator_or_authored_person_backup() {
    assert_eq!(codes(Some("service"), "person"), ["MF-VAL-377"]);
    let manifest: Manifest = serde_json::from_value(json!({"schema_version":"1.0.0", "actor_identities":[
        {"entity_key":"guard","bundle_id":"lab","identity":"human","role":"narrator","provenance":"person"},
        {"entity_key":"guard","bundle_id":"lab","identity":"host","role":"primary","provenance":"service"},
        {"entity_key":"guard","bundle_id":"lab","identity":"backup","role":"co-pilot","provenance":"service"}
    ]})).unwrap();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-377"));
}
