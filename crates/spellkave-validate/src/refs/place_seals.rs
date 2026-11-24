//! Spatial author policy validation. Stamp provenance here is distinct from
//! applyset ownership used by live authority (ADR-0154 D-2).
use super::{BundleScope, LiveVocabulary};
use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;
use std::collections::{BTreeMap, BTreeSet};

pub const MF_VAL_PLACE_SEAL: &str = "MF-VAL-378";
pub const MF_VAL_SEAL_DM_BINDING: &str = "MF-VAL-374";

#[derive(Clone, Debug, Default)]
pub struct SealGraph {
    pub bundle_id: String,
    pub parent_graph_id: String,
    pub blueprint_id: String,
}

pub(crate) fn validate(
    manifest: &Manifest,
    live: Option<&LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    let scope = BundleScope::build(manifest);
    let mut graphs: BTreeMap<String, SealGraph> = live
        .map(|live| live.seal_graphs.clone())
        .unwrap_or_default();
    for graph in &manifest.place_graphs {
        graphs.insert(
            graph.place_graph_id.clone(),
            SealGraph {
                bundle_id: graph.bundle_id.clone(),
                parent_graph_id: graph.parent_graph_id.clone(),
                blueprint_id: graph.blueprint_id.clone(),
            },
        );
    }
    let mut seen = BTreeSet::new();
    for (i, row) in manifest.place_seals.iter().enumerate() {
        let graph = graphs.get(&row.place_graph_id);
        let planned_root = graph_root(&graphs, &row.place_graph_id);
        // H-5 reads live provenance, even when this request reauthors the graph.
        let live_graph = live.and_then(|facts| facts.seal_graphs.get(&row.place_graph_id));
        let live_root = live.and_then(|facts| graph_root(&facts.seal_graphs, &row.place_graph_id));
        let reason = if !seen.insert(&row.place_graph_id) {
            Some("a second seal names this graph")
        } else if row.broken.is_some() || row.break_reason.is_some() {
            Some("broken and break_reason are export-only fields")
        } else if manifest
            .place_graph_instances
            .iter()
            .any(|instance| instance.instance_id.as_deref() == Some(&row.place_graph_id))
            || graph.is_some_and(|g| !g.blueprint_id.is_empty())
            || live_graph.is_some_and(|g| !g.blueprint_id.is_empty())
        {
            Some("instances inherit a copied seal; they cannot be sealed directly")
        } else if graph.is_none() || planned_root.is_none() {
            Some("graph or root is unknown, cyclic or beyond the ancestor depth cap")
        } else if planned_root.is_some_and(|g| g.bundle_id.is_empty())
            || live_root.is_some_and(|g| g.bundle_id.is_empty())
        {
            Some("a world-owned root can only inherit a seal")
        } else if planned_root.is_some_and(|g| g.bundle_id != row.bundle_id)
            || graph.is_some_and(|g| !scope.in_scope(&row.bundle_id, &g.bundle_id))
        {
            Some("the seal's bundle must own the graph's root in its dependency closure")
        } else {
            None
        };
        if let Some(reason) = reason {
            diags.push(Diagnostic { code:MF_VAL_PLACE_SEAL.into(), severity:Severity::Fatal, message:format!("place seal '{}' refused: {reason}",row.place_graph_id), path:format!("place_seals[{i}]"), span:None, hint:Some("Declare the seal on an authored blueprint owned by its root bundle; omit export-only fields.".into()), details:None });
        }
    }
    let sealed: BTreeSet<_> = manifest
        .place_seals
        .iter()
        .map(|r| r.place_graph_id.as_str())
        .collect();
    let is_sealed = |id: &str| {
        let mut current = id;
        let mut seen = BTreeSet::new();
        for _ in 0..64 {
            if !seen.insert(current) {
                return false;
            }
            if sealed.contains(current) {
                return true;
            }
            let Some(graph) = graphs.get(current) else {
                return false;
            };
            if graph.parent_graph_id.is_empty() {
                return false;
            }
            current = &graph.parent_graph_id;
        }
        false
    };
    let mut shapes: BTreeMap<String, String> = spellkave_schema::CORE_EXTENT_KINDS
        .iter()
        .map(|r| (r.kind_id.into(), r.resolution_shape.into()))
        .collect();
    if let Some(live) = live {
        shapes.extend(live.extent_shapes.clone());
    }
    shapes.extend(
        manifest
            .extent_kind_definitions
            .iter()
            .map(|r| (r.kind_id.clone(), r.resolution_shape.clone())),
    );
    for (i, binding) in manifest
        .role_bindings
        .iter()
        .enumerate()
        .filter(|(_, r)| r.role_id == "dm")
    {
        let reason = if shapes
            .get(&binding.extent_kind_ref)
            .is_some_and(|s| s == "Bundle")
            && binding.extent_ref != binding.bundle_id
        {
            Some("this foreign bundle's seals bind the DM nowhere")
        } else if shapes
            .get(&binding.extent_kind_ref)
            .is_some_and(|s| s == "PlaceGraph")
            && is_sealed(&binding.extent_ref)
        {
            Some("this place-graph extent and its descendants are fully sealed")
        } else {
            None
        };
        if let Some(reason) = reason {
            diags.push(Diagnostic { code:MF_VAL_SEAL_DM_BINDING.into(), severity:Severity::Warning, message:format!("DM binding '{}': {reason}",binding.binding_id), path:format!("role_bindings[{i}]"), span:None, hint:Some("Own-bundle extents are authorship and pass seals; use a place extent to respect a prepared region.".into()), details:None });
        }
    }
}

fn graph_root<'a>(graphs: &'a BTreeMap<String, SealGraph>, id: &str) -> Option<&'a SealGraph> {
    let mut current = id;
    let mut seen = BTreeSet::new();
    for _ in 0..64 {
        if !seen.insert(current) {
            return None;
        }
        let graph = graphs.get(current)?;
        if graph.parent_graph_id.is_empty() {
            return Some(graph);
        }
        current = &graph.parent_graph_id;
    }
    None
}
