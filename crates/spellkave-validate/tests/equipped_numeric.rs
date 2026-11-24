use serde_json::json;
use spellkave_schema::Manifest;
use spellkave_validate::{validate_manifest, Severity};

fn fixture(source: &str) -> Manifest {
    let mut manifest = Manifest::default();
    manifest.modifier_templates.push(
        serde_json::from_value(json!({
            "modifier_template_id":"test/item/value", "bundle_id":"test",
            "display_name":"Wearer numeric value", "applies_to":"test/value",
            "predicate_source":"", "modifier_expression_source":source
        }))
        .unwrap(),
    );
    manifest.entity_definition_equip_modifier.push(serde_json::from_value(json!({
        "entity_def_id":"test/item", "modifier_template_id":"test/item/value", "bundle_id":"test"
    })).unwrap());
    manifest
}

#[test]
fn author_validation_warns_on_cyclic_equipped_wearer_values() {
    let diagnostics = validate_manifest(&fixture(
        "let amount = actor.effective(\u{27}test/value\u{27}); amount",
    ));
    assert!(diagnostics.iter().any(|d| d.code == "MF-VAL-390"
        && d.severity == Severity::Warning
        && d.message.contains("cycle")));
}

#[test]
fn author_validation_warns_when_equipped_source_cannot_be_resolved_statically() {
    let diagnostics = validate_manifest(&fixture("let amount = actor.equipped_item(\u{27}test/chest\u{27}).source(\u{27}test/formula\u{27}); amount"));
    assert!(diagnostics.iter().any(|d| d.code == "MF-VAL-390"
        && d.severity == Severity::Warning
        && d.message.contains("source")));
}

#[test]
fn literal_equipped_value_does_not_emit_a_wearer_warning() {
    assert!(!validate_manifest(&fixture("let amount = 3; amount"))
        .iter()
        .any(|d| d.code == "MF-VAL-390"));
}

// keep(K1): advisory classification must distinguish unresolved reads from ordinary raw leaves.
#[test]
fn healthy_wearer_leaves_do_not_emit_dependency_hazards() {
    for source in [
        "let ac = 12 + min(actor.effective('srd/dex'), 2); ac",
        "let amount = actor.stat('test/raw'); amount",
    ] {
        let diagnostics = validate_manifest(&fixture(source));
        assert!(
            !diagnostics.iter().any(|d| d.code == "MF-VAL-390"),
            "{diagnostics:?}"
        );
    }
}

// keep(K1): transitive dependency diagnostics include actual writers, including literal leaves.
#[test]
fn linked_writer_chain_is_healthy_until_it_closes_a_cycle() {
    let mut manifest = fixture("let amount = actor.effective('test/derived'); amount");
    for (id, axis, source) in [
        (
            "test/item/derived",
            "test/derived",
            "let amount = actor.effective('test/leaf'); amount",
        ),
        ("test/item/leaf", "test/leaf", "let amount = 3; amount"),
    ] {
        let mut template = manifest.modifier_templates[0].clone();
        template.modifier_template_id = id.into();
        template.applies_to = axis.into();
        template.modifier_expression_source = source.into();
        manifest.modifier_templates.push(template);
        let mut link = manifest.entity_definition_equip_modifier[0].clone();
        link.modifier_template_id = id.into();
        manifest.entity_definition_equip_modifier.push(link);
    }
    assert!(!validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-390"));
    manifest.modifier_templates[2].modifier_expression_source =
        "let amount = actor.effective('test/value'); amount".into();
    assert!(validate_manifest(&manifest)
        .iter()
        .any(|d| d.code == "MF-VAL-390" && d.message.contains("cycle")));
}
