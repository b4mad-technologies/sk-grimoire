//! Slice 4.4 (#1968 R7, ADR-0087 task 4.4): `attacks` (Multiattack) and
//! `resourceCost` — the two CROSS-CUTTING `ActorBehavior` modifiers, not
//! full shapes. `attacks` overrides shape dispatch entirely (an
//! `attacks`-carrying behavior never reaches its own `b.shape` arm — the
//! multiattack check runs FIRST in `compose.ts`'s `clusterRows`);
//! `resourceCost` layers on top of WHATEVER shape ran, both appending a
//! trailing debit frame_write AND adding a pool-availability clause to the
//! affordance's own gate predicate.

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{row, s, PatchResult};

/// One `MultiAttack` entry — `compose.ts`'s `ActorBehavior.attacks[]`.
#[derive(Deserialize)]
pub struct MultiAttackInput {
    #[serde(rename = "deltaSource")]
    pub delta_source: String,
}

/// `ActorBehavior.resourceCost` — `{ resource, amount }`.
#[derive(Deserialize)]
pub struct ResourceCostInput {
    pub resource: String,
    pub amount: Value,
}

/// `b.attacks.map((atk) => ({ ModifyResource: { actor: "TargetActor",
/// resource: "srd/hp", delta_source: atk.deltaSource } }))` — N independent
/// self-gating attacks, one bare `ModifyResource` per entry. Each delta is
/// expected to already fold its own attack-roll `cond(...)` (the caller's
/// job, not this function's).
pub fn attacks_frame_writes(attacks: &[MultiAttackInput]) -> Vec<Value> {
    attacks
        .iter()
        .map(|atk| {
            row([(
                "ModifyResource",
                row([
                    ("actor", s("TargetActor")),
                    ("resource", s("srd/hp")),
                    ("delta_source", s(atk.delta_source.clone())),
                ]),
            )])
        })
        .collect()
}

/// Parses `attacks_json` (a JSON array of `MultiAttack`) and returns the
/// frame_writes array as a JSON string.
pub fn attacks_frame_writes_json(attacks_json: &str) -> PatchResult<String> {
    let attacks: Vec<MultiAttackInput> = serde_json::from_str(attacks_json)
        .map_err(|err| format!("attacks JSON parse failed: {err}"))?;
    let value = Value::Array(attacks_frame_writes(&attacks));
    serde_json::to_string(&value)
        .map_err(|err| format!("attacks output JSON serialization failed: {err}"))
}

/// `${resourceCost.amount}` — plain JS Number-to-string coercion (no forced
/// decimal, unlike `aoe`'s `toFixed(1)` radius formatting). The original
/// JSON token's own Display matches JS's coercion for the whole-number
/// amounts every real caller passes.
fn plain_number(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// The resource-pool debit — the LAST frame_write ordinal, after every
/// damage/heal/condition write, on the CASTER (`"Actor"`, never the target):
/// a leveled spell spends its slot regardless of what the body did. Mirrors
/// `compose.ts`'s `frameWrites.push({ ModifyResource: { actor: "Actor",
/// resource, delta_source: \`0 - ${amount}\` } })`.
pub fn resource_cost_debit_frame_write(cost: &ResourceCostInput) -> Value {
    row([(
        "ModifyResource",
        row([
            ("actor", s("Actor")),
            ("resource", s(cost.resource.clone())),
            (
                "delta_source",
                s(format!("0 - {}", plain_number(&cost.amount))),
            ),
        ]),
    )])
}

/// Parses `resource_cost_json` (`{"resource": ..., "amount": ...}`) and
/// returns the debit frame_write as a JSON string.
pub fn resource_cost_debit_frame_write_json(resource_cost_json: &str) -> PatchResult<String> {
    let cost: ResourceCostInput = serde_json::from_str(resource_cost_json)
        .map_err(|err| format!("resourceCost JSON parse failed: {err}"))?;
    let value = resource_cost_debit_frame_write(&cost);
    serde_json::to_string(&value)
        .map_err(|err| format!("resourceCost output JSON serialization failed: {err}"))
}

/// The subset of `ActorBehavior` fields the affordance-predicate computation
/// reads, plus `resourceCost` (both cross-cutting modifiers this slice
/// covers together, since `resourceCost` affects both the trailing debit AND
/// this predicate).
#[derive(Deserialize)]
pub struct AffordancePredicateInput {
    pub shape: String,
    #[serde(rename = "predicateSource")]
    pub predicate_source: String,
    #[serde(default)]
    pub attacks: Option<Vec<Value>>,
    #[serde(rename = "resourceCost", default)]
    pub resource_cost: Option<ResourceCostInput>,
}

/// The affordance_templates row's predicate portion — EITHER
/// `{"predicate_source": ...}` OR `{"predicate_bool": true}`, ready to
/// `Object.assign`/spread onto the rest of the row. Mirrors `compose.ts`'s
/// `bodyGateOnAffordance`/`resourceGateSource`/`affordancePredicateSource`/
/// `gateOnAffordance` chain verbatim:
///   - the shape's OWN gate (`predicateSource`) applies to the affordance
///     only for `single-target`/`clock` shapes, only when non-empty, and
///     NEVER when `attacks` is set (each sub-attack self-gates instead);
///   - `resourceCost` ALWAYS adds its own `actor.stat(...) >= amount` clause,
///     regardless of shape;
///   - the two clauses join with `" && "` when both are present; an empty
///     result falls back to `predicate_bool: true` (ungated).
pub fn affordance_predicate(input: &AffordancePredicateInput) -> Value {
    let body_gate_on_affordance = (input.shape == "single-target" || input.shape == "clock")
        && !input.predicate_source.is_empty()
        && input.attacks.is_none();
    let resource_gate_source = input.resource_cost.as_ref().map(|cost| {
        format!(
            "actor.stat('{}') >= {}",
            cost.resource,
            plain_number(&cost.amount)
        )
    });

    let mut clauses: Vec<&str> = Vec::new();
    if body_gate_on_affordance {
        clauses.push(&input.predicate_source);
    }
    if let Some(clause) = &resource_gate_source {
        clauses.push(clause);
    }
    let affordance_predicate_source = clauses.join(" && ");

    if affordance_predicate_source.is_empty() {
        row([("predicate_bool", Value::Bool(true))])
    } else {
        row([("predicate_source", s(affordance_predicate_source))])
    }
}

/// Parses `behavior_json` and returns the affordance predicate portion as a
/// JSON string.
pub fn affordance_predicate_json(behavior_json: &str) -> PatchResult<String> {
    let input: AffordancePredicateInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("affordance predicate behavior JSON parse failed: {err}"))?;
    let value = affordance_predicate(&input);
    serde_json::to_string(&value)
        .map_err(|err| format!("affordance predicate output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn attacks_produces_one_modify_resource_per_entry() {
        let attacks: Vec<MultiAttackInput> = serde_json::from_value(json!([
            { "deltaSource": "cond(d20 >= 15, 0 - 1d6, 0)" },
            { "deltaSource": "cond(d20 >= 15, 0 - 1d6, 0)" },
        ]))
        .unwrap();
        let frame_writes = attacks_frame_writes(&attacks);
        assert_eq!(frame_writes.len(), 2);
        assert_eq!(
            frame_writes[0]["ModifyResource"]["resource"],
            json!("srd/hp")
        );
        assert_eq!(
            frame_writes[0]["ModifyResource"]["actor"],
            json!("TargetActor")
        );
    }

    #[test]
    fn resource_cost_debit_uses_actor_not_target() {
        let cost = ResourceCostInput {
            resource: "srd/spell_slot_1".to_owned(),
            amount: json!(1),
        };
        let value = resource_cost_debit_frame_write(&cost);
        assert_eq!(
            value,
            json!({
                "ModifyResource": {
                    "actor": "Actor",
                    "resource": "srd/spell_slot_1",
                    "delta_source": "0 - 1",
                }
            })
        );
    }

    #[test]
    fn single_target_with_predicate_gates_the_affordance() {
        let input = AffordancePredicateInput {
            shape: "single-target".to_owned(),
            predicate_source: "d20 >= 13".to_owned(),
            attacks: None,
            resource_cost: None,
        };
        assert_eq!(
            affordance_predicate(&input),
            json!({ "predicate_source": "d20 >= 13" })
        );
    }

    #[test]
    fn aoe_shape_never_gates_the_affordance_even_with_predicate_source() {
        let input = AffordancePredicateInput {
            shape: "aoe".to_owned(),
            predicate_source: "d20 >= 13".to_owned(),
            attacks: None,
            resource_cost: None,
        };
        assert_eq!(
            affordance_predicate(&input),
            json!({ "predicate_bool": true })
        );
    }

    #[test]
    fn attacks_suppresses_the_shape_body_gate_even_for_single_target() {
        let input = AffordancePredicateInput {
            shape: "single-target".to_owned(),
            predicate_source: "d20 >= 13".to_owned(),
            attacks: Some(vec![json!({ "deltaSource": "0" })]),
            resource_cost: None,
        };
        assert_eq!(
            affordance_predicate(&input),
            json!({ "predicate_bool": true })
        );
    }

    #[test]
    fn resource_cost_adds_its_own_clause_regardless_of_shape() {
        let input = AffordancePredicateInput {
            shape: "aoe".to_owned(),
            predicate_source: "".to_owned(),
            attacks: None,
            resource_cost: Some(ResourceCostInput {
                resource: "srd/spell_slot_3".to_owned(),
                amount: json!(1),
            }),
        };
        assert_eq!(
            affordance_predicate(&input),
            json!({ "predicate_source": "actor.stat('srd/spell_slot_3') >= 1" })
        );
    }

    #[test]
    fn both_clauses_join_with_double_ampersand() {
        let input = AffordancePredicateInput {
            shape: "single-target".to_owned(),
            predicate_source: "d20 >= 13".to_owned(),
            attacks: None,
            resource_cost: Some(ResourceCostInput {
                resource: "srd/spell_slot_1".to_owned(),
                amount: json!(1),
            }),
        };
        assert_eq!(
            affordance_predicate(&input),
            json!({ "predicate_source": "d20 >= 13 && actor.stat('srd/spell_slot_1') >= 1" })
        );
    }
}
