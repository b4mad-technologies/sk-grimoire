//! Predicate-dependency graph stratification (ADR-0027 D-2).
//!
//! Builds a directed graph where:
//! - Edge `predicate_id → table` exists when the predicate reads from that table.
//! - Edge `materialiser_rule → target_table` exists for each materialiser rule.
//!
//! Runs Tarjan's SCC over this graph and rejects any SCC of size > 1 that
//! includes the `affordance` cache table (a write-target of the materialiser).

use std::collections::{BTreeMap, BTreeSet};

/// The materialised cache table name. Any SCC containing this must be size 1.
pub const AFFORDANCE_TABLE: &str = "affordance";

/// The `prompt_occurrence` materialised cache table (ADR-0142 D-1). Any SCC
/// containing this must be size 1 — a `prompt_definition.open_predicate_source`
/// that read its own occurrence write-target would loop the same way an
/// `affordance_template.predicate_source` reading `affordance` would.
pub const PROMPT_OCCURRENCE_TABLE: &str = "prompt_occurrence";

/// Tables that affordance-template predicates can read (derived from DSL Op survey).
/// These are the logical table names the classifier tracks.
pub const TRACKED_TABLES: &[&str] = &[
    "affordance",
    "actor",
    "actor_spatial_location",
    "world_object",
    "item_stack",
    "trace",
    "capability_grant",
    "status_effect",
];

/// A single predicate → table read edge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PredicateReadEdge {
    pub predicate_id: String,
    pub table_name: String,
}

/// A materialiser rule → target table edge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaterialiserWriteEdge {
    pub rule_id: String,
    pub target_table: String,
}

/// Result of running stratification analysis.
#[derive(Clone, Debug)]
pub struct StratificationResult {
    pub ok: bool,
    /// If not ok, names the SCC nodes involved in the rejected cycle.
    pub cycle_nodes: Vec<String>,
}

/// Infer which tables a predicate reads based on the DSL ops it uses.
///
/// Per ADR-0027 D-1/D-4: the tracked table set is defined in the live
/// architecture doc; this function maps DSL op types to logical table names.
pub fn tables_read_by_predicate(expr: &spellkave_dsl::Expression) -> BTreeSet<String> {
    use spellkave_dsl::Op;
    let mut tables = BTreeSet::new();
    for node in &expr.nodes {
        match &node.op {
            Op::HasCapability(_) => {
                tables.insert("capability_grant".to_string());
            }
            Op::HasNiche(_) => {
                tables.insert("actor".to_string());
            }
            Op::HasStatus(_) => {
                tables.insert("status_effect".to_string());
            }
            Op::TracePresent(_) => {
                tables.insert("trace".to_string());
            }
            Op::KnowsEntity(_) => {
                tables.insert("observer_knowledge".to_string());
            }
            Op::ReadInt(rf) | Op::ReadEffectiveInt(rf) | Op::ReadText(rf) | Op::ReadBool(rf) => {
                use spellkave_dsl::SubjectRef;
                match &rf.subject {
                    // The wielded item is actor-scoped (read via the actor's own
                    // equipment/inventory), not the affordance table → no cycle
                    // risk; grouped with the actor's own-state surface.
                    SubjectRef::Actor
                    | SubjectRef::ActorWieldedItem
                    | SubjectRef::ActorEquippedItem { .. } => {
                        tables.insert("actor".to_string());
                    }
                    SubjectRef::Location | SubjectRef::Target => {
                        tables.insert("actor_spatial_location".to_string());
                    }
                    SubjectRef::World => {
                        // Global singleton state — not a separate table read that
                        // creates a cycle risk, but tracked for completeness.
                    }
                    SubjectRef::Session | SubjectRef::Binding(_) | SubjectRef::Event(_) => {}
                }
            }
            _ => {}
        }
    }
    tables
}

/// Run stratification check over a set of predicates and their shared
/// materialised-cache write target.
///
/// ADR-0027 D-2: no predicate may read from the cache table its own
/// materialiser writes. A predicate that reads its own write-target creates a
/// cycle: predicate reads → `target_table` → materialiser runs predicate →
/// writes `target_table`. `affordance_template.predicate_source` /
/// `AFFORDANCE_TABLE` is the original instance (ADR-0027); ADR-0142 D-1 adds
/// `open_predicate_source` / `PROMPT_OCCURRENCE_TABLE` as a second, same-shape
/// instance — `target_table` is the caller's choice of which pair this call
/// checks, not a second hard-coded constant.
///
/// Implementation: any `PredicateReadEdge` with `table_name == target_table`
/// is immediately a cycle, because the materialiser always writes that table.
/// For completeness, also run Tarjan's SCC over the full read/write graph to
/// catch indirect cycles (predicate → T → ... → target_table → predicate).
///
/// Graph model:
/// - Edge `predicate_id → table_name`: predicate reads that table.
/// - Edge `target_table → predicate_id`: materialiser writes target_table, then
///   re-evaluates predicates for the next frame (feedback arc).
pub fn check_stratification(
    predicate_read_edges: &[PredicateReadEdge],
    materialiser_write_edges: &[MaterialiserWriteEdge],
    target_table: &str,
) -> StratificationResult {
    // Fast path: direct read of the materialised cache is always a cycle.
    for edge in predicate_read_edges {
        if edge.table_name == target_table {
            return StratificationResult {
                ok: false,
                cycle_nodes: vec![edge.predicate_id.clone(), target_table.to_string()],
            };
        }
    }

    // Full graph check for indirect cycles.
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for edge in predicate_read_edges {
        graph
            .entry(edge.predicate_id.clone())
            .or_default()
            .push(edge.table_name.clone());
        graph.entry(edge.table_name.clone()).or_default();
    }

    // Feedback arc: materialiser writes target_table, which can re-trigger
    // predicates that read that table on the next frame.
    // Edge: target_table → predicate_id (for any predicate that reads target_table).
    let predicate_reads_table: BTreeMap<String, BTreeSet<String>> = predicate_read_edges
        .iter()
        .fold(BTreeMap::new(), |mut acc, e| {
            acc.entry(e.table_name.clone())
                .or_default()
                .insert(e.predicate_id.clone());
            acc
        });

    for write_edge in materialiser_write_edges {
        if let Some(preds) = predicate_reads_table.get(&write_edge.target_table) {
            for pred_id in preds {
                graph
                    .entry(write_edge.target_table.clone())
                    .or_default()
                    .push(pred_id.clone());
            }
        }
        graph.entry(write_edge.target_table.clone()).or_default();
    }

    let sccs = tarjan_scc(&graph);

    for scc in &sccs {
        if scc.len() > 1 && scc.iter().any(|n| n == target_table) {
            return StratificationResult {
                ok: false,
                cycle_nodes: scc.iter().cloned().collect(),
            };
        }
    }

    StratificationResult {
        ok: true,
        cycle_nodes: Vec::new(),
    }
}

fn tarjan_scc(graph: &BTreeMap<String, Vec<String>>) -> Vec<BTreeSet<String>> {
    struct State<'a> {
        graph: &'a BTreeMap<String, Vec<String>>,
        index_counter: usize,
        stack: Vec<String>,
        on_stack: BTreeSet<String>,
        index: BTreeMap<String, usize>,
        lowlink: BTreeMap<String, usize>,
        sccs: Vec<BTreeSet<String>>,
    }

    impl<'a> State<'a> {
        fn strongconnect(&mut self, v: String) {
            self.index.insert(v.clone(), self.index_counter);
            self.lowlink.insert(v.clone(), self.index_counter);
            self.index_counter += 1;
            self.stack.push(v.clone());
            self.on_stack.insert(v.clone());

            let neighbors = self.graph.get(&v).cloned().unwrap_or_default();
            for w in neighbors {
                if !self.index.contains_key(&w) {
                    self.strongconnect(w.clone());
                    let w_ll = *self.lowlink.get(&w).unwrap();
                    let v_ll = self.lowlink.get_mut(&v).unwrap();
                    *v_ll = (*v_ll).min(w_ll);
                } else if self.on_stack.contains(&w) {
                    let w_idx = *self.index.get(&w).unwrap();
                    let v_ll = self.lowlink.get_mut(&v).unwrap();
                    *v_ll = (*v_ll).min(w_idx);
                }
            }

            if self.lowlink[&v] == self.index[&v] {
                let mut scc = BTreeSet::new();
                loop {
                    let w = self.stack.pop().unwrap();
                    self.on_stack.remove(&w);
                    scc.insert(w.clone());
                    if w == v {
                        break;
                    }
                }
                self.sccs.push(scc);
            }
        }
    }

    let mut state = State {
        graph,
        index_counter: 0,
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        index: BTreeMap::new(),
        lowlink: BTreeMap::new(),
        sccs: Vec::new(),
    };

    let nodes: Vec<String> = graph.keys().cloned().collect();
    for node in nodes {
        if !state.index.contains_key(&node) {
            state.strongconnect(node);
        }
    }

    state.sccs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acyclic_predicates_pass() {
        let read_edges = vec![
            PredicateReadEdge {
                predicate_id: "p1".into(),
                table_name: "actor".into(),
            },
            PredicateReadEdge {
                predicate_id: "p1".into(),
                table_name: "trace".into(),
            },
        ];
        let write_edges = vec![MaterialiserWriteEdge {
            rule_id: "mat/p1".into(),
            target_table: AFFORDANCE_TABLE.into(),
        }];
        let result = check_stratification(&read_edges, &write_edges, AFFORDANCE_TABLE);
        assert!(result.ok, "acyclic predicates should pass stratification");
    }

    #[test]
    fn predicate_reading_affordance_table_is_rejected() {
        // Predicate p_evil reads from affordance; materialiser writes affordance.
        // This creates: p_evil → affordance → mat/p_evil → affordance (cycle).
        let read_edges = vec![PredicateReadEdge {
            predicate_id: "p_evil".into(),
            table_name: AFFORDANCE_TABLE.into(),
        }];
        let write_edges = vec![MaterialiserWriteEdge {
            rule_id: "mat/p_evil".into(),
            target_table: AFFORDANCE_TABLE.into(),
        }];
        let result = check_stratification(&read_edges, &write_edges, AFFORDANCE_TABLE);
        assert!(
            !result.ok,
            "predicate reading affordance table must be rejected"
        );
        assert!(
            result.cycle_nodes.iter().any(|n| n == AFFORDANCE_TABLE),
            "cycle_nodes must name the affordance table"
        );
    }

    /// ADR-0142 D-8 gate 8, second half: a `prompt_definition.
    /// open_predicate_source` that reads `prompt_occurrence` — its own
    /// write-target — is rejected the same way. The DSL has no op that can
    /// read `prompt_occurrence` today (no `PredicateReadEdge` naming it is
    /// producible from a real authored expression), so this is a synthetic
    /// edge, the same way `predicate_reading_affordance_table_is_rejected`
    /// simulates its case; it proves the mechanism, ready for the day a DSL
    /// op reads this table.
    #[test]
    fn predicate_reading_prompt_occurrence_table_is_rejected() {
        let read_edges = vec![PredicateReadEdge {
            predicate_id: "prompt/p_evil".into(),
            table_name: PROMPT_OCCURRENCE_TABLE.into(),
        }];
        let write_edges = vec![MaterialiserWriteEdge {
            rule_id: "prompt_occurrence/p_evil".into(),
            target_table: PROMPT_OCCURRENCE_TABLE.into(),
        }];
        let result = check_stratification(&read_edges, &write_edges, PROMPT_OCCURRENCE_TABLE);
        assert!(
            !result.ok,
            "predicate reading prompt_occurrence table must be rejected"
        );
        assert!(
            result
                .cycle_nodes
                .iter()
                .any(|n| n == PROMPT_OCCURRENCE_TABLE),
            "cycle_nodes must name the prompt_occurrence table"
        );
    }

    /// A read edge naming `AFFORDANCE_TABLE` must NOT trip the
    /// `prompt_occurrence` check — the two cache tables are independent
    /// write-targets, so `target_table` selects which one this call guards.
    #[test]
    fn affordance_read_edge_does_not_trip_prompt_occurrence_check() {
        let read_edges = vec![PredicateReadEdge {
            predicate_id: "p1".into(),
            table_name: AFFORDANCE_TABLE.into(),
        }];
        let write_edges = vec![MaterialiserWriteEdge {
            rule_id: "mat/p1".into(),
            target_table: AFFORDANCE_TABLE.into(),
        }];
        let result = check_stratification(&read_edges, &write_edges, PROMPT_OCCURRENCE_TABLE);
        assert!(
            result.ok,
            "an affordance-table read must not fail a prompt_occurrence-scoped check"
        );
    }

    #[test]
    fn non_affordance_cycle_does_not_reject() {
        // Two tables cycle among themselves but affordance is not involved.
        let read_edges = vec![
            PredicateReadEdge {
                predicate_id: "p1".into(),
                table_name: "table_a".into(),
            },
            PredicateReadEdge {
                predicate_id: "p2".into(),
                table_name: "table_b".into(),
            },
        ];
        // No materialiser writes to affordance here — no cycle involving it.
        let write_edges = vec![];
        let result = check_stratification(&read_edges, &write_edges, AFFORDANCE_TABLE);
        assert!(result.ok);
    }
}
