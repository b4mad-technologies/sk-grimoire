//! Slice 4.8 (#1968 R7, ADR-0087 task 4.8): the `outcome-graph`
//! `ActorBehavior` shape (#858/ADR-0058) — a BRANCH-ON-OUTCOME spell whose
//! success consequence is EMPTY; the effect is applied at the SELECTED graph
//! node instead. Ported verbatim from `compose.ts`'s `outcome-graph` arm,
//! which touches FIVE tables (`outcome_graphs`, `outcome_templates`,
//! `outcome_graph_nodes`, `outcome_graph_edges`, `consequence_templates`,
//! `consequence_frame_writes` — six, counting both templates/frame_writes)
//! per behavior — this module returns all of them keyed by table, the same
//! "changed tables only" shape `compose::live_wiring` already established
//! (task 4.1), so the caller concat-appends each returned array onto its
//! own accumulator.

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{patched_tables, row, s, PatchResult};

/// One `OutcomeBranch` entry — `compose.ts`'s `ActorBehavior.branches[]`.
#[derive(Deserialize)]
pub struct OutcomeBranchInput {
    pub node: String,
    pub guard: String,
    #[serde(rename = "deltaSource")]
    pub delta_source: String,
    #[serde(default)]
    pub label: Option<String>,
    // Playable-fighter-l1-20 D-1 (#2081-adjacent): the branch's own write
    // Symbolic actor reference ("Actor" or "TargetActor").
    // The target takes the hit when this field is omitted.
    #[serde(default)]
    pub subject: Option<String>,
    // The branch's own write RESOURCE (defaults to "srd/hp" — every existing
    // authored branch omits this and must keep rendering the damage-only
    // shape it authored against).
    #[serde(default)]
    pub resource: Option<String>,
}

/// The subset of `ActorBehavior` fields the outcome-graph shape reads.
#[derive(Deserialize)]
pub struct OutcomeGraphBehaviorInput {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub branches: Option<Vec<OutcomeBranchInput>>,
}

/// Builds every row `compose.ts`'s `outcome-graph` arm emits (the root node
/// plus one arm per branch), returned as `{"<table>": [...], ...}` — see
/// this module's own doc comment for the full table list. `graph_id` is the
/// caller-derived `${action}/graph` id (from `compose::ids::compose_ids`,
/// computed once by the caller — this function does not re-derive it).
pub fn outcome_graph_tables(
    behavior: &OutcomeGraphBehaviorInput,
    graph_id: &str,
    bundle_id: &str,
) -> Value {
    let branches = behavior.branches.as_deref().unwrap_or(&[]);
    let root_node_id = "root";

    let mut outcome_graphs = Vec::new();
    let mut outcome_templates = Vec::new();
    let mut outcome_graph_nodes = Vec::new();
    let mut outcome_graph_edges = Vec::new();
    let mut consequence_templates = Vec::new();
    let mut consequence_frame_writes = Vec::new();

    // `Math.max(1, branches.length)` — carried as a plain i64 (branches.len()
    // is always a small non-negative usize; no numeric-fidelity concern like
    // an authored JSON Number needs).
    let max_draws = branches.len().max(1) as i64;
    outcome_graphs.push(row([
        ("outcome_graph_id", s(graph_id.to_owned())),
        ("bundle_id", s(bundle_id.to_owned())),
        ("display_name", s(behavior.display_name.clone())),
        ("root_node_id", s(root_node_id)),
        ("max_depth", Value::from(4)),
        ("max_nodes", Value::from(branches.len() as i64 + 1)),
        ("max_draws", Value::from(max_draws)),
    ]));

    // Root node — non-terminal, no consequence (a pure branch point).
    let root_outcome = format!("{graph_id}/root");
    outcome_templates.push(row([
        ("outcome_template_id", s(root_outcome.clone())),
        ("bundle_id", s(bundle_id.to_owned())),
        (
            "display_name",
            s(format!("{} (root)", behavior.display_name)),
        ),
        ("outcome_bool", Value::Bool(true)),
    ]));
    outcome_graph_nodes.push(row([
        ("outcome_graph_id", s(graph_id.to_owned())),
        ("outcome_graph_node_id", s(root_outcome.clone())),
        ("node_id", s(root_node_id)),
        ("outcome_template_id", s(root_outcome)),
        ("authored_order", Value::from(0)),
        ("terminal", Value::Bool(false)),
    ]));

    for (i, branch) in branches.iter().enumerate() {
        let node_outcome = format!("{graph_id}/{}", branch.node);
        let branch_consequence = format!("{graph_id}/{}_effect", branch.node);

        // Terminal node → its outcome_template carries the consequence_template_id.
        outcome_templates.push(row([
            ("outcome_template_id", s(node_outcome.clone())),
            ("bundle_id", s(bundle_id.to_owned())),
            (
                "display_name",
                s(format!(
                    "{} ({})",
                    behavior.display_name,
                    branch.label.as_deref().unwrap_or(&branch.node)
                )),
            ),
            ("outcome_bool", Value::Bool(true)),
            ("consequence_template_id", s(branch_consequence.clone())),
        ]));
        outcome_graph_nodes.push(row([
            ("outcome_graph_id", s(graph_id.to_owned())),
            ("outcome_graph_node_id", s(node_outcome)),
            ("node_id", s(branch.node.clone())),
            (
                "outcome_template_id",
                s(format!("{graph_id}/{}", branch.node)),
            ),
            ("authored_order", Value::from(i as i64 + 1)),
            ("terminal", Value::Bool(true)),
        ]));
        outcome_graph_edges.push(row([
            ("outcome_graph_id", s(graph_id.to_owned())),
            (
                "outcome_graph_edge_id",
                s(format!("{graph_id}/root_to_{}", branch.node)),
            ),
            ("from_node_id", s(root_node_id)),
            ("to_node_id", s(branch.node.clone())),
            ("guard_predicate_source", s(branch.guard.clone())),
            ("authored_order", Value::from(i as i64)),
        ]));

        // The branch's effect — one ModifyResource on the cast's target.
        consequence_templates.push(row([
            ("template_id", s(branch_consequence.clone())),
            ("bundle_id", s(bundle_id.to_owned())),
        ]));
        let subject = branch.subject.as_deref().unwrap_or("TargetActor");
        let resource = branch.resource.as_deref().unwrap_or("srd/hp");
        consequence_frame_writes.push(row([
            ("template_id", s(branch_consequence)),
            ("ordinal", Value::from(0)),
            (
                "frame_write",
                row([(
                    "ModifyResource",
                    row([
                        ("actor", s(subject.to_owned())),
                        ("resource", s(resource.to_owned())),
                        ("delta_source", s(branch.delta_source.clone())),
                    ]),
                )]),
            ),
        ]));
    }

    patched_tables([
        ("outcome_graphs", outcome_graphs),
        ("outcome_templates", outcome_templates),
        ("outcome_graph_nodes", outcome_graph_nodes),
        ("outcome_graph_edges", outcome_graph_edges),
        ("consequence_templates", consequence_templates),
        ("consequence_frame_writes", consequence_frame_writes),
    ])
}

/// Parses `behavior_json` and returns `outcome_graph_tables` as a JSON
/// string.
pub fn outcome_graph_tables_json(
    behavior_json: &str,
    graph_id: &str,
    bundle_id: &str,
) -> PatchResult<String> {
    let behavior: OutcomeGraphBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("outcome-graph behavior JSON parse failed: {err}"))?;
    let value = outcome_graph_tables(&behavior, graph_id, bundle_id);
    serde_json::to_string(&value)
        .map_err(|err| format!("outcome-graph output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn acid_arrow_behavior() -> OutcomeGraphBehaviorInput {
        serde_json::from_value(json!({
            "displayName": "Melf's Acid Arrow",
            "branches": [
                { "node": "hit", "guard": "d20 >= 15", "deltaSource": "4d4", "label": "Hit" },
                { "node": "miss", "guard": "1 == 1", "deltaSource": "2d4" },
            ],
        }))
        .unwrap()
    }

    #[test]
    fn root_node_is_non_terminal_with_no_consequence() {
        let value = outcome_graph_tables(&acid_arrow_behavior(), "srd/acid-arrow/graph", "srd");
        let nodes = value["outcome_graph_nodes"].as_array().unwrap();
        assert_eq!(nodes[0]["node_id"], json!("root"));
        assert_eq!(nodes[0]["terminal"], json!(false));
        assert_eq!(nodes[0]["authored_order"], json!(0));
    }

    #[test]
    fn each_branch_gets_a_terminal_node_edge_and_consequence() {
        let value = outcome_graph_tables(&acid_arrow_behavior(), "srd/acid-arrow/graph", "srd");
        let nodes = value["outcome_graph_nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 3); // root + 2 branches
        assert_eq!(nodes[1]["node_id"], json!("hit"));
        assert_eq!(nodes[1]["terminal"], json!(true));
        assert_eq!(nodes[1]["authored_order"], json!(1));

        let edges = value["outcome_graph_edges"].as_array().unwrap();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0]["from_node_id"], json!("root"));
        assert_eq!(edges[0]["to_node_id"], json!("hit"));
        assert_eq!(edges[0]["guard_predicate_source"], json!("d20 >= 15"));
        assert_eq!(edges[0]["authored_order"], json!(0));

        let consequences = value["consequence_templates"].as_array().unwrap();
        assert_eq!(consequences.len(), 2);
        assert_eq!(
            consequences[0]["template_id"],
            json!("srd/acid-arrow/graph/hit_effect")
        );

        let frame_writes = value["consequence_frame_writes"].as_array().unwrap();
        assert_eq!(
            frame_writes[0]["frame_write"]["ModifyResource"]["delta_source"],
            json!("4d4")
        );
    }

    #[test]
    fn label_defaults_to_node_id_when_unset() {
        let value = outcome_graph_tables(&acid_arrow_behavior(), "srd/acid-arrow/graph", "srd");
        let templates = value["outcome_templates"].as_array().unwrap();
        // templates[0] is the root; templates[1] is "hit" (has a label),
        // templates[2] is "miss" (no label, defaults to node id).
        assert_eq!(
            templates[1]["display_name"],
            json!("Melf's Acid Arrow (Hit)")
        );
        assert_eq!(
            templates[2]["display_name"],
            json!("Melf's Acid Arrow (miss)")
        );
    }

    #[test]
    fn max_nodes_max_draws_derive_from_branch_count() {
        let value = outcome_graph_tables(&acid_arrow_behavior(), "srd/acid-arrow/graph", "srd");
        let graph = &value["outcome_graphs"].as_array().unwrap()[0];
        assert_eq!(graph["max_nodes"], json!(3)); // 2 branches + root
        assert_eq!(graph["max_draws"], json!(2));
        assert_eq!(graph["max_depth"], json!(4));
    }

    #[test]
    fn omitted_subject_and_resource_render_byte_identically_to_the_pre_d1_shape() {
        // Playable-fighter-l1-20 D-1 acceptance bar: a branch declared with
        // only {node, guard, delta, label} — the shape every existing SRD
        // fixture (e.g. acid-arrow's hit/miss branches) authors today — must
        // keep writing ModifyResource(TargetActor, srd/hp, ...), unchanged.
        let value = outcome_graph_tables(&acid_arrow_behavior(), "srd/acid-arrow/graph", "srd");
        let frame_writes = value["consequence_frame_writes"].as_array().unwrap();
        for fw in frame_writes {
            let mr = &fw["frame_write"]["ModifyResource"];
            assert_eq!(mr["actor"], json!("TargetActor"));
            assert_eq!(mr["resource"], json!("srd/hp"));
        }
    }

    #[test]
    fn branch_naming_acting_subject_and_a_non_hp_resource_renders_that_write() {
        let behavior: OutcomeGraphBehaviorInput = serde_json::from_value(json!({
            "displayName": "Second Wind",
            "branches": [
                {
                    "node": "recover",
                    "guard": "1 == 1",
                    "deltaSource": "1d10",
                    "subject": "Actor",
                    "resource": "srd/hp",
                },
            ],
        }))
        .unwrap();
        let value = outcome_graph_tables(&behavior, "srd/second-wind/graph", "srd");
        let frame_writes = value["consequence_frame_writes"].as_array().unwrap();
        let mr = &frame_writes[0]["frame_write"]["ModifyResource"];
        assert_eq!(mr["actor"], json!("Actor"));
        assert_eq!(mr["resource"], json!("srd/hp"));
        assert_eq!(mr["delta_source"], json!("1d10"));
    }

    #[test]
    fn branch_naming_target_subject_explicitly_matches_omitting_it() {
        let explicit: OutcomeGraphBehaviorInput = serde_json::from_value(json!({
            "displayName": "Melf's Acid Arrow",
            "branches": [
                {
                    "node": "hit",
                    "guard": "d20 >= 15",
                    "deltaSource": "4d4",
                    "subject": "TargetActor",
                    "resource": "srd/hp",
                },
            ],
        }))
        .unwrap();
        let omitted: OutcomeGraphBehaviorInput = serde_json::from_value(json!({
            "displayName": "Melf's Acid Arrow",
            "branches": [
                { "node": "hit", "guard": "d20 >= 15", "deltaSource": "4d4" },
            ],
        }))
        .unwrap();
        let explicit_value = outcome_graph_tables(&explicit, "srd/acid-arrow/graph", "srd");
        let omitted_value = outcome_graph_tables(&omitted, "srd/acid-arrow/graph", "srd");
        assert_eq!(
            explicit_value["consequence_frame_writes"],
            omitted_value["consequence_frame_writes"]
        );
    }

    #[test]
    fn empty_branches_still_yields_a_root_only_graph_with_max_draws_one() {
        let behavior: OutcomeGraphBehaviorInput =
            serde_json::from_value(json!({ "displayName": "Empty Graph" })).unwrap();
        let value = outcome_graph_tables(&behavior, "srd/empty/graph", "srd");
        let graph = &value["outcome_graphs"].as_array().unwrap()[0];
        assert_eq!(graph["max_nodes"], json!(1));
        assert_eq!(graph["max_draws"], json!(1)); // Math.max(1, 0) = 1
        assert_eq!(value["outcome_graph_nodes"].as_array().unwrap().len(), 1);
        assert_eq!(value["outcome_graph_edges"].as_array().unwrap().len(), 0);
    }
}
