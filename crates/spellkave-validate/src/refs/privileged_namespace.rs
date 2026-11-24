//! Reserved authority derivation is fenced; authored gates require escalation.
use super::{diag_fatal, diag_warning, RefIndex};
use crate::Diagnostic;
use spellkave_schema::Manifest;

pub const PRIVILEGED_CAPABILITY_PREFIX: &str = "frontier.";
pub const MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT: &str = "MF-VAL-364";
pub const MF_VAL_PRIVILEGED_TEMPLATE_ESCALATION: &str = "MF-VAL-375";
pub const MF_VAL_FORGED_BINDING_SOURCE: &str = "MF-VAL-380";
pub const MF_VAL_SPAWN_DEFINITION_MODE: &str = "MF-VAL-381";
pub const MF_VAL_ACTOR_CAPABILITY_REF: &str = "MF-VAL-365";
/// Derive the kernel's vocabulary from the shared inline source, never a second list.
pub fn bootstrap_capability_ids() -> Vec<String> {
    spellkave_format::parse_manifest(spellkave_schema::CORE_BOOTSTRAP_MANIFEST)
        .expect("schema bootstrap must parse")
        .manifest
        .capability_definitions
        .into_iter()
        .map(|row| row.capability_id)
        .collect()
}

pub(super) fn reject_bridge(id: &str, kind: &str, key: &str, diags: &mut Vec<Diagnostic>) {
    if id.starts_with(PRIVILEGED_CAPABILITY_PREFIX) {
        if matches!(
            kind,
            "tag_capability_grant"
                | "condition_capability_extent_grants"
                | "entity_definition_initial_capability"
                | "condition_definitions.grants_capability_while_active"
                | "condition_definitions.suppresses_capability_while_active"
                | "entity_definitions.grants_capability_while_equipped"
                | "entity_definitions.grants_capability_while_present_in_location"
        ) {
            diags.push(diag_fatal(MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT, kind,
                format!("{kind} '{key}' cannot derive reserved capability '{id}'; use an explicit publisher-authorized actor_capabilities grant")));
        } else {
            diags.push(diag_warning(MF_VAL_PRIVILEGED_TEMPLATE_ESCALATION, kind,
                format!("{kind} '{key}' names reserved capability '{id}'; applying this authority-bearing row requires bundle-scoped escalation")));
        }
    }
}

fn predicate(
    source: &str,
    kind: &str,
    key: &str,
    macros: &[spellkave_dsl::DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    if let Ok(expression) = spellkave_dsl::compile_predicate_with_macros(source, macros) {
        for node in &expression.nodes {
            if let spellkave_dsl::Op::HasCapability(op) = &node.op {
                reject_bridge(&op.capability_id, kind, key, diags);
            }
        }
    }
}

pub(super) fn validate(manifest: &Manifest, index: &RefIndex, diags: &mut Vec<Diagnostic>) {
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);
    // These predicate owners do not participate in the older cross-ref walk.
    for row in &manifest.niche_definitions {
        predicate(
            &row.predicate_source,
            "niche_definitions.predicate_source",
            &row.niche_id,
            &macros,
            diags,
        );
    }
    for row in &manifest.prompt_definitions {
        predicate(
            &row.open_predicate_source,
            "prompt_definitions.open_predicate_source",
            &row.prompt_def_id,
            &macros,
            diags,
        );
    }
    for row in &manifest.action_definitions {
        predicate(
            &row.resolution_predicate_source,
            "action_definitions.resolution_predicate_source",
            &row.action_id,
            &macros,
            diags,
        );
    }
    // Check unused Bool macro bodies too, through the same typed expansion.
    for row in &manifest.dsl_macro_definitions {
        if row.return_type != spellkave_schema::DslValueType::Bool {
            continue;
        }
        let mut arguments = vec![format!("'{}'", row.macro_id)];
        let mut supported = true;
        for parameter in &row.parameters {
            arguments.push(match parameter.value_type {
                spellkave_schema::DslValueType::Bool => "false".into(),
                spellkave_schema::DslValueType::Int => "0".into(),
                spellkave_schema::DslValueType::Text => "''".into(),
                _ => {
                    supported = false;
                    break;
                }
            });
        }
        if supported {
            predicate(
                &format!("macro({})", arguments.join(", ")),
                "dsl_macro_definitions.body_source",
                &row.macro_id,
                &macros,
                diags,
            );
        }
    }
    for row in &manifest.tag_capability_grant {
        reject_bridge(&row.capability_id, "tag_capability_grant", &row.tag, diags);
    }
    for row in &manifest.entity_definition_initial_capability {
        reject_bridge(
            &row.capability_id,
            "entity_definition_initial_capability",
            &row.entity_def_id,
            diags,
        );
    }
    for row in &manifest.condition_definitions {
        for (kind, ids) in [
            (
                "condition_definitions.grants_capability_while_active",
                &row.grants_capability_while_active,
            ),
            (
                "condition_definitions.suppresses_capability_while_active",
                &row.suppresses_capability_while_active,
            ),
        ] {
            for id in ids {
                reject_bridge(id, kind, &row.condition_definition_id, diags);
            }
        }
    }
    for row in &manifest.entity_definitions {
        for (kind, csv) in [
            (
                "entity_definitions.grants_capability_while_equipped",
                &row.grants_capability_while_equipped,
            ),
            (
                "entity_definitions.grants_capability_while_present_in_location",
                &row.grants_capability_while_present_in_location,
            ),
        ] {
            for id in csv.split(',').map(str::trim).filter(|id| !id.is_empty()) {
                reject_bridge(id, kind, &row.entity_def_id, diags);
            }
        }
    }
    for row in &manifest.affordance_templates {
        reject_bridge(
            &row.capability_id,
            "affordance_templates.capability_id",
            &row.template_id,
            diags,
        );
    }
    for row in &manifest.action_definitions {
        reject_bridge(
            &row.required_capability,
            "action_definitions.required_capability",
            &row.action_id,
            diags,
        );
    }
    for row in &manifest.effect_definitions {
        if row
            .primitives_csv
            .split(',')
            .any(|primitive| primitive.trim() == "Spawn")
        {
            if let Err(reason) = validate_spawn_definition_parameters(&row.parameters_schema_json) {
                diags.push(diag_fatal(
                    MF_VAL_SPAWN_DEFINITION_MODE,
                    "effect_definitions.parameters_schema_json",
                    format!(
                        "effect '{}' has invalid Spawn parameters: {reason}",
                        row.effect_def_id
                    ),
                ));
            }
        }
    }
    let bootstrap = bootstrap_capability_ids();
    for row in &manifest.actor_capabilities {
        if row.source.starts_with("role_binding:") {
            diags.push(diag_fatal(
                MF_VAL_FORGED_BINDING_SOURCE,
                "actor_capabilities.source",
                format!(
                    "actor_capability '{}' uses reserved binding-realisation source prefix",
                    row.actor_entity_key
                ),
            ));
        }
        let id = &row.capability_id;
        let admitted = bootstrap.contains(id)
            || index
                .capability_definitions
                .get(id)
                .is_some_and(|bundle| index.bundle_scope.in_scope(&row.bundle_id, bundle));
        if !admitted {
            diags.push(diag_fatal(MF_VAL_ACTOR_CAPABILITY_REF, "actor_capabilities.capability_id",
                format!("actor_capability '{}:{id}' does not resolve in bundle '{}' dependency closure or kernel bootstrap vocabulary", row.actor_entity_key, row.bundle_id)));
        }
    }
}

/// The same mode and required spatial shape check is used offline and by Spawn.
pub fn validate_spawn_definition_parameters(parameters: &str) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_str(parameters).map_err(|_| "malformed JSON")?;
    let blueprint = value
        .get("blueprint_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let from_args = match value.get("definition_from_args") {
        None => false,
        Some(serde_json::Value::Bool(value)) => *value,
        _ => return Err("definition_from_args must be a boolean".into()),
    };
    let allowlist = value.get("allowlist_tag").filter(|value| !value.is_null());
    let tagged = allowlist
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    if from_args {
        if !blueprint.trim().is_empty() || !tagged {
            return Err(
                "definition_from_args requires an allowlist_tag and excludes blueprint_id".into(),
            );
        }
    } else if blueprint.trim().is_empty() || allowlist.is_some() {
        return Err("fixed blueprint_id is required and excludes allowlist_tag".into());
    }
    let location = value
        .get("location_ref")
        .ok_or("location_ref is required")?;
    for key in ["kind", "id"] {
        if !location
            .get(key)
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(format!("location_ref.{key} is required"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{EntityDefinitionInitialCapabilityRow, TagCapabilityGrantRow};

    fn fixture() -> Manifest {
        let mut ordinary: Manifest = toml::from_str(include_str!("../../../../packages/spellkave-mechanics/fixtures/role_capability_write_gate/manifest.toml"))
            .expect("existing ordinary fixture parses");
        let bridge: Manifest = toml::from_str(include_str!("../../../../packages/spellkave-mechanics/fixtures/privileged_namespace_bridge/manifest.toml")).unwrap();
        let mut condition = bridge.condition_definitions[0].clone();
        condition.grants_capability_while_active.clear();
        condition.suppresses_capability_while_active.clear();
        ordinary.condition_definitions.push(condition);
        ordinary.entity_definitions.push(toml::from_str("entity_def_id = 'role_capability_write_gate/item'\nbundle_id = 'role_capability_write_gate'\ndisplay_name = 'Item'").unwrap());
        ordinary
    }

    #[test]
    fn spawn_mode_check_is_shared_between_fixed_and_args_selected_definitions() {
        for parameters in [
            r#"{"blueprint_id":"wolf","location_ref":{"kind":"place_node","id":"forest/clearing"}}"#,
            r#"{"definition_from_args":true,"allowlist_tag":"core/spawnable","location_ref":{"kind":"place_node","id":"forest/clearing"}}"#,
        ] {
            assert!(validate_spawn_definition_parameters(parameters).is_ok());
        }
        for parameters in [
            r#"{"blueprint_id":"wolf","definition_from_args":true,"allowlist_tag":"core/spawnable","location_ref":{"kind":"place_node","id":"forest/clearing"}}"#,
            r#"{"definition_from_args":true,"location_ref":{"kind":"place_node","id":"forest/clearing"}}"#,
            r#"{"blueprint_id":"wolf","location_ref":{"kind":"place_node","id":""}}"#,
        ] {
            assert!(validate_spawn_definition_parameters(parameters).is_err());
        }
    }

    #[test]
    fn privileged_templates_warn_and_forged_binding_sources_fail() {
        let mut manifest = fixture();
        manifest.affordance_templates[0].capability_id = "frontier.despawn".into();
        let diagnostics = crate::validate_manifest(&manifest);
        assert!(diagnostics
            .iter()
            .any(|d| d.code == "MF-VAL-375" && d.severity == crate::Severity::Warning));
        assert!(!diagnostics
            .iter()
            .any(|d| d.code == MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT));
        manifest.actor_capabilities[0].source = "role_binding:forged".into();
        assert!(crate::validate_manifest(&manifest)
            .iter()
            .any(|d| d.code == "MF-VAL-380" && d.severity == crate::Severity::Fatal));
    }

    #[test]
    fn reserved_namespace_refuses_six_bridges_and_warns_three_gates() {
        let names = [
            "tag-grant",
            "definition-seed",
            "condition-grant",
            "condition-suppression",
            "equipment",
            "location-present",
            "affordance",
            "action",
            "predicate",
        ];
        for (placement, name) in names.iter().enumerate() {
            let mut manifest = fixture();
            let id = "frontier.pace_time".to_string();
            match placement {
                0 => manifest.tag_capability_grant.push(TagCapabilityGrantRow {
                    tag: "ordinary".into(),
                    capability_id: id,
                    bundle_id: "role_capability_write_gate".into(),
                }),
                1 => manifest.entity_definition_initial_capability.push(
                    EntityDefinitionInitialCapabilityRow {
                        extent_kind_ref: String::new(),
                        extent_ref: String::new(),
                        entity_def_id: manifest.entity_definitions[0].entity_def_id.clone(),
                        capability_id: id,
                        bundle_id: "role_capability_write_gate".into(),
                    },
                ),
                2 => manifest.condition_definitions[0]
                    .grants_capability_while_active
                    .push(id),
                3 => manifest.condition_definitions[0]
                    .suppresses_capability_while_active
                    .push(id),
                4 => {
                    manifest.entity_definitions[0].grants_capability_while_equipped =
                        format!("ordinary, {id}")
                }
                5 => {
                    manifest.entity_definitions[0].grants_capability_while_present_in_location = id
                }
                6 => manifest.affordance_templates[0].capability_id = id,
                7 => manifest.action_definitions[0].required_capability = id,
                8 => {
                    manifest.affordance_templates[0].predicate_source =
                        "actor.has_capability('frontier.pace_time')".into()
                }
                _ => unreachable!(),
            }
            let directory = std::path::PathBuf::from(
                std::env::var_os("CARGO_MANIFEST_DIR")
                    .expect("cargo test must supply the current package directory"),
            )
            .join("tests/fixtures/privileged_namespace");
            let fixture_path = directory.join(format!("{name}.toml"));
            let snapshot_path = directory.join(format!("{name}.json"));
            if std::env::var_os("UPDATE_AUTH32_FIXTURES").is_some() {
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(
                    &fixture_path,
                    format!(
                        "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\n{}",
                        toml::to_string(&manifest).unwrap()
                    ),
                )
                .unwrap();
            }
            let frozen: Manifest =
                toml::from_str(&std::fs::read_to_string(&fixture_path).unwrap()).unwrap();
            assert_eq!(
                frozen, manifest,
                "{name} fixture differs from its isolated placement"
            );
            let diagnostics = crate::validate_manifest(&manifest);
            let found: Vec<_> = diagnostics
                .iter()
                .filter(|diag| {
                    diag.code
                        == if placement < 6 {
                            MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT
                        } else {
                            MF_VAL_PRIVILEGED_TEMPLATE_ESCALATION
                        }
                })
                .collect();
            assert_eq!(found.len(), 1, "placement {placement}: {diagnostics:?}");
            assert_eq!(
                found[0].severity,
                if placement < 6 {
                    crate::Severity::Fatal
                } else {
                    crate::Severity::Warning
                }
            );
            assert!(found[0].message.contains("frontier.pace_time"));
            let snapshot = format!("{}\n", serde_json::to_string_pretty(&found).unwrap());
            if std::env::var_os("UPDATE_AUTH32_FIXTURES").is_some() {
                std::fs::write(&snapshot_path, &snapshot).unwrap();
            }
            assert_eq!(
                std::fs::read_to_string(snapshot_path).unwrap(),
                snapshot,
                "{name} diagnostic snapshot"
            );
        }
    }

    #[test]
    fn other_predicate_owners_and_unused_macros_are_checked() {
        for owner in ["niche", "prompt", "action", "macro"] {
            let mut manifest = fixture();
            let source = "actor.has_capability('frontier.pace_time')";
            match owner {
                "niche" => manifest.niche_definitions[0].predicate_source = source.into(),
                "action" => manifest.action_definitions[0].resolution_predicate_source = source.into(),
                "prompt" => manifest.prompt_definitions.push(toml::from_str(&format!("prompt_def_id = 'test/prompt'\nbundle_id = 'test'\nsource_selector = ''\naudience_kind_ref = ''\nopen_predicate_source = \"{source}\"")).unwrap()),
                "macro" => manifest.dsl_macro_definitions.push(toml::from_str(&format!("macro_id = 'test/reserved'\nbundle_id = 'test'\nreturn_type = 'Bool'\nbody_source = \"{source}\"")).unwrap()),
                _ => unreachable!(),
            }
            assert!(
                crate::validate_manifest(&manifest)
                    .iter()
                    .any(|diag| diag.code == MF_VAL_PRIVILEGED_TEMPLATE_ESCALATION),
                "owner {owner}"
            );
        }
    }

    #[test]
    fn explicit_grants_use_closure_or_bootstrap_but_never_unknown_ids() {
        let mut manifest = fixture();
        assert!(!crate::validate_manifest(&manifest)
            .iter()
            .any(|diag| diag.code == MF_VAL_ACTOR_CAPABILITY_REF));
        manifest.actor_capabilities[0].capability_id = "frontier.pace_time".into();
        assert!(!crate::validate_manifest(&manifest)
            .iter()
            .any(|diag| diag.code == MF_VAL_ACTOR_CAPABILITY_REF
                || diag.code == MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT));
        manifest.actor_capabilities[0].capability_id = "frontier.unknown".into();
        assert!(crate::validate_manifest(&manifest)
            .iter()
            .any(|diag| diag.code == MF_VAL_ACTOR_CAPABILITY_REF));
    }

    #[test]
    fn binding_path_is_dynamic_and_gameplay_predicates_remain_legal() {
        let mut manifest = fixture();
        for source in [
            "actor.has_capability('$bindings.authority')",
            "actor.has_capability('ordinary')",
        ] {
            manifest.affordance_templates[0].predicate_source = source.into();
            let diagnostics = crate::validate_manifest(&manifest);
            assert!(
                !diagnostics.iter().any(|diag| diag.code == "MF-VAL-201"
                    || diag.code == MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT),
                "{diagnostics:?}"
            );
        }
    }
    #[test]
    fn session_dsl_owners_reject_reserved_literals_and_keep_other_sources_legal() {
        for field in [
            "busy_allow_pred",
            "busy_break_pred",
            "termination_pred",
            "winner_predicate",
        ] {
            for source in [
                "actor.has_capability('frontier.pace_time')",
                "actor.has_capability('$bindings.authority')",
                "actor.has_capability('ordinary')",
            ] {
                assert!(spellkave_dsl::compile_predicate_with_macros(source, &[]).is_ok());
                let mut manifest = fixture();
                let session = &mut manifest.session_definitions[0];
                let key = session.session_def_id.clone();
                match field {
                    "busy_allow_pred" => session.busy_allow_pred = Some(source.into()),
                    "busy_break_pred" => session.busy_break_pred = Some(source.into()),
                    "termination_pred" => session.termination_pred = source.into(),
                    "winner_predicate" => session.winner_predicate = source.into(),
                    _ => unreachable!(),
                }
                let diagnostics = crate::validate_manifest(&manifest);
                let found: Vec<_> = diagnostics
                    .iter()
                    .filter(|diag| diag.code == MF_VAL_PRIVILEGED_TEMPLATE_ESCALATION)
                    .collect();
                if source.contains("frontier.") {
                    assert_eq!(found.len(), 1, "{field}: {diagnostics:?}");
                    assert_eq!(found[0].path, format!("session_definitions.{field}"));
                    assert!(found[0].message.contains(&key));
                    assert!(found[0].message.contains("frontier.pace_time"));
                } else {
                    assert!(found.is_empty(), "{field}: {diagnostics:?}");
                }
            }
        }
        let mut manifest = fixture();
        let row = &mut manifest.session_definitions[0];
        row.participants_pred = "actor.has_capability('frontier.pace_time')".into();
        row.turn_rule = "frontier.pace_time".into();
        row.visibility_policy = "frontier.pace_time".into();
        row.stakes_required = "frontier.pace_time".into();
        row.on_terminate = "frontier.pace_time".into();
        assert!(!crate::validate_manifest(&manifest)
            .iter()
            .any(|diag| diag.code == MF_VAL_PRIVILEGED_CAPABILITY_PLACEMENT));
    }
}
