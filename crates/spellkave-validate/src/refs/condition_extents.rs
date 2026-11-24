//! Condition-derived grant rules use existing capability, extent and bundle vocabulary.
use super::{diag_fatal, BundleScope, LiveVocabulary};
use crate::Diagnostic;
use spellkave_schema::{storage_key::storage_key_strs, Manifest, CORE_EXTENT_KINDS};
use std::collections::{BTreeMap, BTreeSet};

pub const MF_VAL_CONDITION_EXTENT_GRANT: &str = "MF-VAL-389";

#[cfg(test)]
mod tests;

pub(super) fn validate(
    manifest: &Manifest,
    live: Option<&LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    let empty = BTreeMap::new();
    let scope = BundleScope::build_with_live(
        manifest,
        live.map(|world| &world.dm.bundle_dependencies)
            .unwrap_or(&empty),
    );
    let mut conditions = live
        .map(|world| world.dm.conditions.clone())
        .unwrap_or_default();
    conditions.extend(
        manifest
            .condition_definitions
            .iter()
            .map(|row| (row.condition_definition_id.clone(), row.bundle_id.clone())),
    );
    let mut capabilities = live
        .map(|world| world.capabilities.clone())
        .unwrap_or_default();
    capabilities.extend(
        manifest
            .capability_definitions
            .iter()
            .map(|row| (row.capability_id.clone(), row.bundle_id.clone())),
    );
    let mut graphs = live
        .map(|world| world.place_graphs.clone())
        .unwrap_or_default();
    graphs.extend(
        manifest
            .place_graphs
            .iter()
            .map(|row| (row.place_graph_id.clone(), row.bundle_id.clone())),
    );
    let mut tags = live.map(|world| world.dm.tags.clone()).unwrap_or_default();
    tags.extend(
        manifest
            .tag_value_definitions
            .iter()
            .map(|row| (row.tag.clone(), row.bundle_id.clone())),
    );
    let mut shapes: BTreeMap<String, String> = CORE_EXTENT_KINDS
        .iter()
        .map(|row| (row.kind_id.into(), row.resolution_shape.into()))
        .collect();
    if let Some(world) = live {
        shapes.extend(world.extent_shapes.clone());
    }
    shapes.extend(
        manifest
            .extent_kind_definitions
            .iter()
            .map(|row| (row.kind_id.clone(), row.resolution_shape.clone())),
    );
    let mut keys = BTreeSet::new();
    for (ordinal, row) in manifest
        .condition_capability_extent_grants
        .iter()
        .enumerate()
    {
        let path = format!("condition_capability_extent_grants[{ordinal}]");
        if !manifest
            .bundles
            .iter()
            .any(|bundle| bundle.bundle_id == row.bundle_id)
            && !live.is_some_and(|world| world.bundles.contains(&row.bundle_id))
        {
            diags.push(diag_fatal(
                MF_VAL_CONDITION_EXTENT_GRANT,
                &format!("{path}.bundle_id"),
                "owning bundle must exist".into(),
            ));
        }
        let fields = [
            &row.condition_definition_id,
            &row.capability_id,
            &row.extent_kind_ref,
            &row.extent_ref,
        ];
        if fields.iter().any(|field| field.trim().is_empty()) || row.bundle_id.trim().is_empty() {
            diags.push(diag_fatal(
                MF_VAL_CONDITION_EXTENT_GRANT,
                &path,
                "condition, capability, extent kind, extent reference and owner must be nonempty"
                    .into(),
            ));
        }
        let key = storage_key_strs(&fields.map(String::as_str));
        if live
            .and_then(|world| world.dm.condition_extent_owners.get(&key))
            .is_some_and(|owner| owner != &row.bundle_id)
        {
            diags.push(diag_fatal(
                MF_VAL_CONDITION_EXTENT_GRANT,
                &path,
                "condition extent grant cannot change its owning bundle".into(),
            ));
        }
        if !keys.insert(key) {
            diags.push(diag_fatal(
                MF_VAL_CONDITION_EXTENT_GRANT,
                &path,
                "duplicate four-reference condition extent grant".into(),
            ));
        }
        for (field, reference, owner) in [
            (
                "condition_definition_id",
                &row.condition_definition_id,
                conditions.get(&row.condition_definition_id),
            ),
            (
                "capability_id",
                &row.capability_id,
                capabilities.get(&row.capability_id),
            ),
        ] {
            if !owner.is_some_and(|owner| scope.in_scope(&row.bundle_id, owner)) {
                diags.push(diag_fatal(
                    MF_VAL_CONDITION_EXTENT_GRANT,
                    &format!("{path}.{field}"),
                    format!(
                        "'{reference}' must resolve inside the owning bundle's dependency closure"
                    ),
                ));
            }
        }
        super::privileged_namespace::reject_bridge(
            &row.capability_id,
            "condition_capability_extent_grants",
            &row.condition_definition_id,
            diags,
        );
        let owner = match shapes.get(&row.extent_kind_ref).map(String::as_str) {
            Some("PlaceGraph") => graphs.get(&row.extent_ref),
            Some("Bundle") => {
                if scope.in_scope(&row.bundle_id, &row.extent_ref)
                    && (manifest
                        .bundles
                        .iter()
                        .any(|bundle| bundle.bundle_id == row.extent_ref)
                        || live.is_some_and(|world| world.bundles.contains(&row.extent_ref)))
                {
                    continue;
                }
                None
            }
            // Party references are open membership tags. Declared tag ownership
            // constrains foreign references; an undeclared open tag has no members
            // until an admitted membership write provides them.
            Some("Party") => {
                if !tags.contains_key(&row.extent_ref) {
                    continue;
                }
                tags.get(&row.extent_ref)
            }
            _ => None,
        };
        if !owner.is_some_and(|owner| scope.in_scope(&row.bundle_id, owner)) {
            diags.push(diag_fatal(MF_VAL_CONDITION_EXTENT_GRANT, &format!("{path}.extent_ref"), "extent kind and reference must resolve inside the owning bundle's dependency closure".into()));
        }
    }
}
