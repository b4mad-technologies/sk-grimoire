//! One compiled row classifier shared by enqueue, cursor recheck and connected plans.
use spellkave_schema::Manifest;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivilegedTemplateRow {
    pub kind: String,
    pub key: String,
    pub bundle_id: String,
    pub capability_ids: Vec<String>,
}

pub fn predicate_ids(source: &str, macros: &[spellkave_dsl::DslMacroDefinition]) -> Vec<String> {
    let Ok(expression) = spellkave_dsl::compile_predicate_with_macros(source, macros) else {
        return Vec::new(); // Malformed sources are rejected by the existing parse gate.
    };
    expression
        .nodes
        .iter()
        .filter_map(|node| match &node.op {
            spellkave_dsl::Op::HasCapability(op)
                if op
                    .capability_id
                    .starts_with(super::privileged_namespace::PRIVILEGED_CAPABILITY_PREFIX) =>
            {
                Some(op.capability_id.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Field names here select the DSL owners, never textual substrings in prose.
/// Opaque session participants/policy strings deliberately do not participate.
pub fn rows(manifest: &Manifest) -> Vec<PrivilegedTemplateRow> {
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);
    let value = serde_json::to_value(manifest).expect("manifest serializes");
    let mut result = Vec::new();
    for (kind, values) in value.as_object().expect("manifest object") {
        let Some(values) = values.as_array() else {
            continue;
        };
        for value in values {
            let Some(key) = spellkave_schema::row_identity::row_key_of(kind, value) else {
                continue;
            };
            let mut ids = BTreeSet::new();
            let Some(fields) = value.as_object() else {
                continue;
            };
            for (field, value) in fields {
                let Some(source) = value.as_str() else {
                    continue;
                };
                if (kind == "affordance_templates" && field == "capability_id")
                    || (kind == "action_definitions" && field == "required_capability")
                {
                    if source.starts_with(super::privileged_namespace::PRIVILEGED_CAPABILITY_PREFIX)
                    {
                        ids.insert(source.to_owned());
                    }
                } else if matches!(
                    field.as_str(),
                    "predicate_source"
                        | "resolution_predicate_source"
                        | "gate_predicate_source"
                        | "remove_predicate_source"
                        | "open_predicate_source"
                        | "guard_predicate_source"
                        | "busy_allow_pred"
                        | "busy_break_pred"
                        | "termination_pred"
                        | "winner_predicate"
                ) {
                    ids.extend(predicate_ids(source, &macros));
                }
            }
            if kind == "dsl_macro_definitions" {
                if let Some(row) = manifest
                    .dsl_macro_definitions
                    .iter()
                    .find(|row| row.macro_id == key)
                {
                    if row.return_type == spellkave_schema::DslValueType::Bool {
                        let args: Option<Vec<_>> = row
                            .parameters
                            .iter()
                            .map(|parameter| match parameter.value_type {
                                spellkave_schema::DslValueType::Bool => Some("false"),
                                spellkave_schema::DslValueType::Int => Some("0"),
                                spellkave_schema::DslValueType::Text => Some("''"),
                                _ => None,
                            })
                            .collect();
                        if let Some(args) = args {
                            let mut arguments = vec![format!("'{}'", row.macro_id)];
                            arguments.extend(args.into_iter().map(str::to_owned));
                            ids.extend(predicate_ids(
                                &format!("macro({})", arguments.join(", ")),
                                &macros,
                            ));
                        }
                    }
                }
            }
            if ids.is_empty() {
                continue;
            };
            let bundle = fields
                .get("bundle_id")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .or_else(|| {
                    fields
                        .get("outcome_graph_id")
                        .and_then(|value| value.as_str())
                        .and_then(|id| {
                            manifest
                                .outcome_graphs
                                .iter()
                                .find(|row| row.outcome_graph_id == id)
                        })
                        .map(|row| row.bundle_id.clone())
                })
                .unwrap_or_default();
            result.push(PrivilegedTemplateRow {
                kind: kind.clone(),
                key,
                bundle_id: bundle,
                capability_ids: ids.into_iter().collect(),
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rows_use_compiled_literals_and_shared_authored_identities() {
        let manifest: Manifest = toml::from_str(r#"
schema_version = "1.0.0"
[[affordance_templates]]
template_id = 'own/act'
display_name = 'Act'
bundle_id = 'own'
capability_id = 'frontier.despawn'
niche_id = 'own/target'
outcome_template_id = 'own/outcome'
predicate_source = "actor.has_capability('frontier.set_state') && actor.has_capability('$bindings.dynamic')"
[[action_definitions]]
action_id = 'own/action'
bundle_id = 'own'
required_capability = 'frontier.transfer'
display_name = 'Action'
on_success = ''
"#).unwrap();
        let rows = rows(&manifest);
        assert_eq!(rows.len(), 2);
        let act = rows
            .iter()
            .find(|row| row.kind == "affordance_templates")
            .unwrap();
        assert_eq!(act.key, "own/act");
        assert_eq!(act.bundle_id, "own");
        assert_eq!(
            act.capability_ids,
            ["frontier.despawn", "frontier.set_state"]
        );
    }
    #[test]
    fn opaque_session_policy_identifiers_are_not_authority_predicates() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = "1.0.0"
[[session_definitions]]
session_def_id = 'own/session'
bundle_id = 'own'
participants_pred = "actor.has_capability('frontier.despawn')"
turn_rule = 'frontier.transfer'
"#,
        )
        .unwrap();
        assert!(rows(&manifest).is_empty());
    }
}
