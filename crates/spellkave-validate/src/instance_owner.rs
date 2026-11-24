//! Applying-owner admission shared by offline validation and server expansion.
use crate::{Diagnostic, Severity};
use spellkave_schema::{Manifest, PlaceGraphInstanceRow};

pub const MF_VAL_INSTANCE_OWNER: &str = "MF-VAL-373";

pub fn instance_owner<'a>(
    manifest: &'a Manifest,
    row: &'a PlaceGraphInstanceRow,
) -> Result<&'a str, String> {
    if let Some(owner) = row.bundle_id.as_deref() {
        if !owner.is_empty()
            && manifest
                .bundles
                .iter()
                .any(|bundle| bundle.bundle_id == owner)
        {
            return Ok(owner);
        }
        return Err(format!("{MF_VAL_INSTANCE_OWNER}: instance '{}' claims bundle '{owner}' outside the applying bundle headers", row.blueprint_id));
    }
    if manifest.bundles.len() == 1 && !manifest.bundles[0].bundle_id.is_empty() {
        return Ok(&manifest.bundles[0].bundle_id);
    }
    // A blueprint declared in the same manifest has its own authored owner;
    // foreign blueprints have no such applying declaration to disambiguate.
    if let Some(blueprint) = manifest.place_graphs.iter().find(|graph| {
        graph.place_graph_id == row.blueprint_id
            && !graph.bundle_id.is_empty()
            && manifest
                .bundles
                .iter()
                .any(|bundle| bundle.bundle_id == graph.bundle_id)
    }) {
        return Ok(&blueprint.bundle_id);
    }
    Err(format!("{MF_VAL_INSTANCE_OWNER}: foreign blueprint '{}' has an ambiguous applying owner; set place_graph_instances.bundle_id", row.blueprint_id))
}

pub(crate) fn validate(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for (index, row) in manifest.place_graph_instances.iter().enumerate() {
        if let Err(message) = instance_owner(manifest, row) {
            diags.push(Diagnostic {
                code: MF_VAL_INSTANCE_OWNER.into(),
                severity: Severity::Fatal,
                message,
                path: format!("place_graph_instances[{index}].bundle_id"),
                span: None,
                hint: Some("Set bundle_id to a bundle declared by this apply.".into()),
                details: None,
            });
        }
    }
}
