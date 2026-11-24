//! Offline metadata-attachment target-kind admission (ADR-0073).
//!
//! Checks that metadata attachments use supported targets and audiences.
//!
//! Public subject art is admitted on every kind in this list; the validator
//! does **not** special-case `*.art.image` (ADR-0073 D-1: the kernel stays
//! type-blind to payload and namespace).

use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;

/// `metadata_attachments.target_kind` is not an admitted metadata target.
pub const MF_VAL_METADATA_TARGET_KIND: &str = "MF-VAL-330";

/// Accepted `metadata_attachments.target_kind` values.
pub const METADATA_TARGET_KINDS: &[&str] = &[
    "entity",
    "entity_definition",
    "place_graph",
    "place_node",
    "place_edge",
    "item_definition",
    "resource_definition",
    "affordance_template",
    "capability_definition",
    "bundle",
    // ADR-0142: dialogue labels target a prompt definition by id.
    "prompt_definition",
];

/// `metadata_namespaces.audience` is neither a built-in audience nor
/// capability-shaped (ADR-0130 D-3).
pub const MF_VAL_METADATA_AUDIENCE_SHAPE: &str = "MF-VAL-331";

/// Audience tokens that name a relation rather than a capability.
pub const METADATA_AUDIENCE_RELATIONS: &[&str] = &[
    "public",
    "Public",
    "local",
    "Local",
    "hidden",
    "Hidden",
    "internal",
    "Internal",
    "discoverable",
    "Discoverable",
    "owned",
    "Owned",
];

/// Offline half of the audience rule: a built-in word, or a capability id
/// (contains `/` or `.`). Whether that capability exists is checked at live
/// apply, where dependency bundles are present.
pub fn validate_metadata_namespace_audiences(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for (i, row) in manifest.metadata_namespaces.iter().enumerate() {
        let audience = row.audience.as_str();
        if METADATA_AUDIENCE_RELATIONS.contains(&audience)
            || audience.contains('/')
            || audience.contains('.')
        {
            continue;
        }
        diags.push(fatal(
            MF_VAL_METADATA_AUDIENCE_SHAPE,
            &format!("metadata_namespaces[{i}].audience"),
            format!(
                "metadata namespace '{}' audience '{audience}' is neither one of a visibility class or 'owned' nor a capability id",
                row.namespace
            ),
        ));
    }
}

/// Reject unknown `metadata_attachments.target_kind` values.
pub fn validate_metadata_attachment_target_kinds(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for (i, row) in manifest.metadata_attachments.iter().enumerate() {
        if METADATA_TARGET_KINDS.contains(&row.target_kind.as_str()) {
            continue;
        }
        diags.push(fatal(
            MF_VAL_METADATA_TARGET_KIND,
            &format!("metadata_attachments[{i}].target_kind"),
            format!(
                "metadata '{}:{}:{}' uses unknown target_kind '{}'",
                row.target_kind, row.target_ref, row.namespace, row.target_kind
            ),
        ));
    }
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
    use spellkave_schema::{Manifest, MetadataNamespaceRow, MetadataRow};

    fn art_payload() -> String {
        r#"{"path":"srd-art/spells/acid-splash.webp","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","license":"CC-BY-4.0"}"#.into()
    }

    fn attachment(
        target_kind: &str,
        target_ref: &str,
        namespace: &str,
        payload: &str,
    ) -> MetadataRow {
        MetadataRow {
            target_kind: target_kind.into(),
            target_ref: target_ref.into(),
            namespace: namespace.into(),
            payload_json: payload.into(),
            bundle_id: "srd".into(),
            source: "manifest".into(),
        }
    }

    fn manifest_with(rows: Vec<MetadataRow>) -> Manifest {
        Manifest {
            metadata_namespaces: vec![
                MetadataNamespaceRow {
                    namespace: "srd.art.image".into(),
                    audience: "public".into(),
                    bundle_id: "srd".into(),
                    description: Some("Public subject art.".into()),
                    on_demand: false,
                },
                MetadataNamespaceRow {
                    namespace: "srd.text".into(),
                    audience: "public".into(),
                    bundle_id: "srd".into(),
                    description: Some("Verbatim CC-BY SRD text.".into()),
                    on_demand: false,
                },
            ],
            metadata_attachments: rows,
            ..Manifest::default()
        }
    }

    fn codes(diags: &[Diagnostic]) -> Vec<String> {
        diags.iter().map(|d| d.code.clone()).collect()
    }

    #[test]
    fn public_art_on_affordance_and_spell_definition_is_accepted() {
        let manifest = manifest_with(vec![
            attachment(
                "affordance_template",
                "srd/cast_acid_splash/affordance",
                "srd.art.image",
                &art_payload(),
            ),
            attachment(
                "entity_definition",
                "srd/spell/acid-splash",
                "srd.art.image",
                &art_payload(),
            ),
        ]);
        let mut diags = Vec::new();
        validate_metadata_attachment_target_kinds(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "public art on an affordance and a spell row must be accepted: {diags:?}"
        );
    }

    #[test]
    fn public_srd_text_on_a_spell_row_is_accepted() {
        let manifest = manifest_with(vec![attachment(
            "entity_definition",
            "srd/spell/acid-splash",
            "srd.text",
            r#"{"text":"You hurl a bubble of acid.","source":"SRD 5.2.1","license":"CC-BY-4.0"}"#,
        )]);
        let mut diags = Vec::new();
        validate_metadata_attachment_target_kinds(&manifest, &mut diags);
        assert!(diags.is_empty(), "srd.text on a spell row: {diags:?}");
    }

    #[test]
    fn every_admitted_kind_accepts_public_art() {
        let manifest = manifest_with(
            METADATA_TARGET_KINDS
                .iter()
                .map(|kind| attachment(kind, "srd/subject", "srd.art.image", &art_payload()))
                .collect(),
        );
        let mut diags = Vec::new();
        validate_metadata_attachment_target_kinds(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "public art must be legal on every admitted target kind: {diags:?}"
        );
    }

    #[test]
    fn namespace_audience_must_be_a_built_in_word_or_a_capability_id() {
        let mut manifest = manifest_with(Vec::new());
        for (audience, ok) in [
            ("public", true),
            ("hidden", true),
            ("Internal", true),
            ("owned", true),
            ("frontier.apply_manifest", true),
            ("elven_mini/dm/seat", true),
            ("author", false),
            ("dm", false),
            ("observer", false),
            ("driver-of-target", false),
            ("DM", false),
        ] {
            manifest.metadata_namespaces[0].audience = audience.into();
            let mut diags = Vec::new();
            validate_metadata_namespace_audiences(&manifest, &mut diags);
            assert_eq!(
                codes(&diags).contains(&MF_VAL_METADATA_AUDIENCE_SHAPE.to_string()),
                !ok,
                "{audience}: {diags:?}"
            );
        }
    }

    #[test]
    fn subject_less_text_on_the_bundle_is_accepted() {
        let manifest = manifest_with(vec![attachment(
            "bundle",
            "srd",
            "srd.text",
            r#"{"text":"This work includes material from the System Reference Document 5.2.1.","source":"SRD 5.2.1","license":"CC-BY-4.0"}"#,
        )]);
        let diags = crate::validate_manifest(&manifest);
        assert!(
            diags.iter().all(|d| d.code != MF_VAL_METADATA_TARGET_KIND),
            "bundle-level text belongs to no subject and must be admitted: {diags:?}"
        );
    }

    #[test]
    fn validate_manifest_accepts_public_art_on_affordance_and_spell() {
        let manifest = manifest_with(vec![
            attachment(
                "affordance_template",
                "srd/cast_acid_splash/affordance",
                "srd.art.image",
                &art_payload(),
            ),
            attachment(
                "entity_definition",
                "srd/spell/acid-splash",
                "srd.art.image",
                &art_payload(),
            ),
        ]);
        let diags = crate::validate_manifest(&manifest);
        assert!(
            diags
                .iter()
                .all(|d| d.code != MF_VAL_METADATA_TARGET_KIND),
            "full validate_manifest must accept public art on an affordance and a spell row: {diags:?}"
        );
    }

    #[test]
    fn public_art_and_srd_text_on_named_action_capability_is_accepted() {
        let manifest = manifest_with(vec![
            attachment(
                "capability_definition",
                "srd/action/attack",
                "srd.art.image",
                &art_payload(),
            ),
            attachment(
                "capability_definition",
                "srd/action/attack",
                "srd.text",
                r#"{"text":"Attack with a weapon or an Unarmed Strike.","source":"SRD 5.2.1","license":"CC-BY-4.0"}"#,
            ),
        ]);
        let mut diags = Vec::new();
        validate_metadata_attachment_target_kinds(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "public art and srd.text on srd/action/attack must be accepted: {diags:?}"
        );
        let diags = crate::validate_manifest(&manifest);
        assert!(
            diags.iter().all(|d| d.code != MF_VAL_METADATA_TARGET_KIND),
            "full validate_manifest must accept capability_definition attachments: {diags:?}"
        );
    }

    #[test]
    fn unknown_target_kind_is_fatal() {
        let manifest = manifest_with(vec![attachment(
            "spell",
            "srd/spell/acid-splash",
            "srd.art.image",
            &art_payload(),
        )]);
        let mut diags = Vec::new();
        validate_metadata_attachment_target_kinds(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_METADATA_TARGET_KIND.to_owned()));
        assert!(diags[0].message.contains("unknown target_kind"));
    }
}
