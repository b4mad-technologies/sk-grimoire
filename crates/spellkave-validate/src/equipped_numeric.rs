//! Advisory author diagnostics. Unknown wearers/other equipment cannot be
//! certified statically; the concrete EquipItem snapshot remains authoritative.
use crate::{Diagnostic, Severity};
use spellkave_dsl::{compile_int_expression_with_macros, EvaluationLimits, Op, SubjectRef};
use spellkave_schema::Manifest;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn validate(manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>) {
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);
    let definitions: BTreeSet<_> = manifest
        .entity_definition_equip_modifier
        .iter()
        .map(|link| link.entity_def_id.as_str())
        .collect();
    for definition in definitions {
        let template_ids: BTreeSet<_> = manifest
            .entity_definition_equip_modifier
            .iter()
            .filter(|link| link.entity_def_id == definition)
            .map(|link| link.modifier_template_id.as_str())
            .collect();
        let mut edges = BTreeMap::<String, BTreeSet<String>>::new();
        let mut reasons = BTreeSet::new();
        let mut has_wearer_read = false;
        let own_slot = manifest
            .entity_definitions
            .iter()
            .find(|row| row.entity_def_id == definition)
            .and_then(|row| {
                row.slot_compatibility_predicate
                    .trim()
                    .strip_prefix("slot_id == \u{22}")
                    .and_then(|rest| rest.strip_suffix('"'))
            })
            .map(str::to_owned);
        for link in manifest
            .entity_definition_equip_modifier
            .iter()
            .filter(|link| link.entity_def_id == definition)
        {
            let Some(template) = manifest
                .modifier_templates
                .iter()
                .find(|row| row.modifier_template_id == link.modifier_template_id)
            else {
                continue;
            };
            let Ok(expression) =
                compile_int_expression_with_macros(&template.modifier_expression_source, &macros)
            else {
                continue;
            };
            let mut todo = vec![(expression, false)];
            let mut nodes = 0usize;
            while let Some((expression, in_source)) = todo.pop() {
                let mut pending = vec![expression.root_node_id];
                let mut visited = BTreeSet::new();
                while let Some(id) = pending.pop() {
                    if !visited.insert(id) {
                        continue;
                    }
                    nodes += 1;
                    if nodes > EvaluationLimits::default().max_nodes as usize {
                        reasons.insert("numeric_node_budget_exhausted");
                        break;
                    }
                    let Some(node) = expression.node(id) else {
                        continue;
                    };
                    match &node.op {
                        Op::ReadInt(read) if read.subject == SubjectRef::Actor => {
                            has_wearer_read = true;
                        }
                        Op::ReadEffectiveInt(read) if read.subject == SubjectRef::Actor => {
                            has_wearer_read = true;
                            edges
                                .entry(template.applies_to.clone())
                                .or_default()
                                .insert(read.field_id.clone());
                        }
                        Op::ReadInt(read) | Op::ReadEffectiveInt(read)
                            if read.subject == SubjectRef::Target =>
                        {
                            has_wearer_read = true;
                            reasons.insert("numeric_value_has_no_bound_target");
                        }
                        Op::DslSource(source) => {
                            let resolved = if !in_source {
                                if let SubjectRef::ActorEquippedItem { slot_id } = &source.subject {
                                    if own_slot.as_deref() == Some(slot_id.as_str()) {
                                        expression
                                            .node(source.slot_node)
                                            .and_then(|node| match &node.op {
                                                Op::TextLiteral(text) => manifest
                                                    .entity_definition_item_dsl_source
                                                    .iter()
                                                    .find(|row| {
                                                        row.entity_def_id == definition
                                                            && row.slot == text.value
                                                    }),
                                                _ => None,
                                            })
                                            .and_then(|row| {
                                                compile_int_expression_with_macros(
                                                    &row.source,
                                                    &macros,
                                                )
                                                .ok()
                                            })
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            if let Some(nested) = resolved {
                                todo.push((nested, true));
                            } else {
                                reasons.insert(
                                    "unresolved_equipped_source_requires_a_concrete_equip_snapshot",
                                );
                            }
                        }
                        _ => {}
                    }
                    pending.extend(node.op.child_nodes());
                }
            }
        }
        if has_wearer_read {
            let limits = EvaluationLimits::default();
            if let Err(reason) = crate::numeric_dependencies::dependency_order(
                &edges,
                limits.max_nodes as usize,
                limits.max_depth as usize,
            ) {
                reasons.insert(reason);
            }
            // A leaf can be supplied by stored wearer state; ordinary raw or
            // effective reads do not themselves prove a dependency hazard.
            // Concrete missing leaves are diagnosed by the runtime snapshot.
        }
        // Predicates may deactivate rows and the wearer may supply missing leaves.
        // These are possible hazards, never an independent author admission gate.
        for reason in reasons {
            diagnostics.push(Diagnostic {
                code:"MF-VAL-390".into(), severity:Severity::Warning,
                message:format!("potential equipped dependency hazard in definition {definition}: {reason}; any statically followed source assumes a possible compatible placement; predicates, wearer state and other equipment determine the concrete result"),
                path:"entity_definition_equip_modifier".into(),span:None,
                hint:Some("check the prospective wearer graph with EquipItem; resolve cycles/sources before equipping".into()),details:Some(serde_json::json!({"entity_def_id":definition,"modifier_template_ids":template_ids})),
            });
        }
    }
}
