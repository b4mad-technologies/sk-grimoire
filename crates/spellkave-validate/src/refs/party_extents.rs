//! Party tag references derived from authored extent kinds.

use std::collections::{BTreeMap, BTreeSet};

use spellkave_schema::Manifest;

pub fn party_extent_tags(
    manifest: &Manifest,
    live: Option<&super::LiveVocabulary>,
) -> BTreeSet<String> {
    let mut shapes: BTreeMap<&str, &str> = spellkave_schema::CORE_EXTENT_KINDS
        .iter()
        .map(|row| (row.kind_id, row.resolution_shape))
        .collect();
    if let Some(live) = live {
        for (kind, shape) in &live.extent_shapes {
            shapes.insert(kind, shape);
        }
    }
    for row in &manifest.extent_kind_definitions {
        shapes.insert(&row.kind_id, &row.resolution_shape);
    }
    manifest
        .role_bindings
        .iter()
        .map(|r| (&r.extent_kind_ref, &r.extent_ref))
        .chain(
            manifest
                .actor_capabilities
                .iter()
                .map(|r| (&r.extent_kind_ref, &r.extent_ref)),
        )
        .chain(
            manifest
                .entity_definition_initial_capability
                .iter()
                .map(|r| (&r.extent_kind_ref, &r.extent_ref)),
        )
        .chain(
            manifest
                .condition_capability_extent_grants
                .iter()
                .map(|r| (&r.extent_kind_ref, &r.extent_ref)),
        )
        .filter(|(kind, _)| shapes.get(kind.as_str()) == Some(&"Party"))
        .map(|(_, tag)| tag.clone())
        .collect()
}
