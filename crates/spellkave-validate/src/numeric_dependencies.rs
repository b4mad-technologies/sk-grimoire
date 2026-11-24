//! Shared bounded numeric dependency ordering; no evaluator or admission policy.
use std::collections::{BTreeMap, BTreeSet};

pub fn dependency_order(
    edges: &BTreeMap<String, BTreeSet<String>>,
    max_axes: usize,
    max_depth: usize,
) -> Result<Vec<String>, &'static str> {
    let axes: BTreeSet<_> = edges
        .keys()
        .chain(edges.values().flatten())
        .cloned()
        .collect();
    if axes.len() > max_axes {
        return Err("numeric_axis_budget_exhausted");
    }
    let mut depths = BTreeMap::<String, usize>::new();
    let mut active = BTreeSet::new();
    let mut order = Vec::new();
    for root in axes {
        let mut pending = vec![(root, false)];
        while let Some((axis, exiting)) = pending.pop() {
            if depths.contains_key(&axis) {
                continue;
            }
            if exiting {
                let depth = 1 + edges
                    .get(&axis)
                    .into_iter()
                    .flatten()
                    .filter_map(|dependency| depths.get(dependency))
                    .copied()
                    .max()
                    .unwrap_or(0);
                if depth > max_depth {
                    return Err("effective_chain_budget_exhausted");
                }
                active.remove(&axis);
                depths.insert(axis.clone(), depth);
                order.push(axis);
            } else {
                if !active.insert(axis.clone()) {
                    return Err("effective_dependency_cycle");
                }
                pending.push((axis.clone(), true));
                for dependency in edges.get(&axis).into_iter().flatten().rev() {
                    if active.contains(dependency) {
                        return Err("effective_dependency_cycle");
                    }
                    if !depths.contains_key(dependency) {
                        pending.push((dependency.clone(), false));
                    }
                }
            }
        }
    }
    Ok(order)
}
