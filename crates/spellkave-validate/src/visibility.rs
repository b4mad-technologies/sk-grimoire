//! Authored tokens for visibility and elevation classes.

use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;

/// An authored visibility field holds a token that is not a visibility class.
pub const MF_VAL_VISIBILITY_CLASS: &str = "MF-VAL-337";

/// `actor_capabilities.elevation_class` is not an admitted elevation ceiling.
pub const MF_VAL_ELEVATION_CLASS: &str = "MF-VAL-338";

/// Every spelling `VisibilityClass::parse_authored` accepts.
pub const VISIBILITY_CLASS_TOKENS: &[&str] = &[
    "Public",
    "public",
    "Local",
    "local",
    "Hidden",
    "hidden",
    "Internal",
    "internal",
    "Discoverable",
    "discoverable",
];

/// Every spelling `manifest::elevation_ceiling` accepts (`""` is the default).
pub const ELEVATION_CLASS_TOKENS: &[&str] = &[
    "", "hidden", "Hidden", "internal", "Internal", "full", "Full",
];

/// Reject unknown tokens in every authored visibility field and in
/// `actor_capabilities.elevation_class`.
pub fn validate_visibility_tokens(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for (i, row) in manifest.object_state_definitions.iter().enumerate() {
        check(
            &row.default_visibility,
            &format!("object_state_definitions[{i}].default_visibility"),
            &format!("object_state_definition '{}'", row.state_id),
            diags,
        );
    }
    for (i, row) in manifest.capability_definitions.iter().enumerate() {
        check(
            &row.default_visibility,
            &format!("capability_definitions[{i}].default_visibility"),
            &format!("capability_definition '{}'", row.capability_id),
            diags,
        );
    }
    for (i, row) in manifest.object_state.iter().enumerate() {
        if let Some(value) = &row.visibility {
            check(
                value,
                &format!("object_state[{i}].visibility"),
                &format!("object_state '{}'", row.state_id),
                diags,
            );
        }
    }
    for (i, row) in manifest.actor_capabilities.iter().enumerate() {
        if let Some(value) = &row.visibility {
            check(
                value,
                &format!("actor_capabilities[{i}].visibility"),
                &format!("actor_capability '{}'", row.capability_id),
                diags,
            );
        }
        if !ELEVATION_CLASS_TOKENS.contains(&row.elevation_class.as_str()) {
            diags.push(fatal(
                MF_VAL_ELEVATION_CLASS,
                &format!("actor_capabilities[{i}].elevation_class"),
                format!(
                    "actor_capability '{}' uses unknown elevation_class '{}' (expected hidden, internal or full)",
                    row.capability_id, row.elevation_class
                ),
            ));
        }
    }
    for (i, row) in manifest.spatial_niche_attachments.iter().enumerate() {
        check(
            &row.visibility,
            &format!("spatial_niche_attachments[{i}].visibility"),
            &format!(
                "spatial_niche_attachment '{}' on '{}'",
                row.niche_id, row.place_node_id
            ),
            diags,
        );
    }
    for (i, row) in manifest.spatial_environment_state.iter().enumerate() {
        check(
            &row.visibility,
            &format!("spatial_environment_state[{i}].visibility"),
            &format!("spatial_environment_state '{}'", row.state_id),
            diags,
        );
    }
}

fn check(value: &str, path: &str, subject: &str, diags: &mut Vec<Diagnostic>) {
    if VISIBILITY_CLASS_TOKENS.contains(&value) {
        return;
    }
    diags.push(fatal(
        MF_VAL_VISIBILITY_CLASS,
        path,
        format!(
            "{subject} uses unknown visibility class '{value}' (expected public, local, hidden, internal or discoverable)"
        ),
    ));
}

fn fatal(code: &str, path: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        message,
        path: path.to_owned(),
        span: None,
        hint: None,
        details: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{ActorCapabilityRow, SpatialNicheAttachmentRow};

    fn niche(visibility: &str) -> SpatialNicheAttachmentRow {
        SpatialNicheAttachmentRow {
            bundle_id: "demo".into(),
            place_node_id: "demo/graph/room".into(),
            place_edge_id: None,
            niche_id: "core/shared_table_surface".into(),
            visibility: visibility.into(),
            condition_bool: false,
            ecological_invariant: false,
            on_entry_action_id: String::new(),
            on_entry_target_entity_key: String::new(),
        }
    }

    fn grant(elevation_class: &str) -> ActorCapabilityRow {
        ActorCapabilityRow {
            actor_entity_key: "demo/actor".into(),
            capability_id: "demo/cap".into(),
            source: "manifest".into(),
            source_args_hash: String::new(),
            bundle_id: "demo".into(),
            visibility: None,
            extent_ref: String::new(),
            extent_kind_ref: String::new(),
            elevation_class: elevation_class.into(),
        }
    }

    fn run(manifest: &Manifest) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        validate_visibility_tokens(manifest, &mut diags);
        diags
    }

    #[test]
    fn niche_attachment_with_private_visibility_is_fatal() {
        let mut manifest = Manifest::default();
        manifest.spatial_niche_attachments.push(niche("private"));
        let diags = run(&manifest);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, MF_VAL_VISIBILITY_CLASS);
        assert_eq!(diags[0].path, "spatial_niche_attachments[0].visibility");
        assert!(diags[0].message.contains("'private'"));
    }

    #[test]
    fn every_admitted_spelling_passes() {
        let mut manifest = Manifest::default();
        for token in VISIBILITY_CLASS_TOKENS {
            manifest.spatial_niche_attachments.push(niche(token));
        }
        assert!(run(&manifest).is_empty());
    }

    #[test]
    fn unknown_elevation_class_is_fatal_and_empty_is_default() {
        let mut manifest = Manifest::default();
        manifest.actor_capabilities.push(grant(""));
        manifest.actor_capabilities.push(grant("godlike"));
        let diags = run(&manifest);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, MF_VAL_ELEVATION_CLASS);
        assert_eq!(diags[0].path, "actor_capabilities[1].elevation_class");
    }
}
