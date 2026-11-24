//! Role vocabulary is content. Reference closure never realizes authority.
use super::{diag_fatal, diag_warning, BundleScope, LiveVocabulary};
use crate::Diagnostic;
use spellkave_schema::{Manifest, RoleAggregationRow, RoleDefinitionRow};
use std::collections::{BTreeMap, BTreeSet};

pub const MF_VAL_ROLE_CAPABILITY_REF: &str = "MF-VAL-366";
pub const MF_VAL_ROLE_DUPLICATE: &str = "MF-VAL-367";
pub const MF_VAL_ROLE_AGGREGATION_REF: &str = "MF-VAL-368";
pub const STANDARD_ROLE_IDS: &[&str] = &[
    "warden",
    "viewer",
    "timekeeper",
    "dm",
    "adventure-author",
    "system-author",
];

/// Union the selected world's definition and all bundle-owned contributions.
/// Callers select the scope; this reader never binds a role or authorizes an act.
pub fn merged_role_capability_ids(
    role_id: &str,
    definitions: &[RoleDefinitionRow],
    aggregations: &[RoleAggregationRow],
) -> Vec<String> {
    definitions
        .iter()
        .filter(|r| r.role_id == role_id)
        .flat_map(|r| &r.capability_ids)
        .chain(
            aggregations
                .iter()
                .filter(|r| r.role_id == role_id)
                .flat_map(|r| &r.capability_ids),
        )
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn validate(
    manifest: &Manifest,
    live: Option<&LiveVocabulary>,
    scope: &BundleScope,
    diags: &mut Vec<Diagnostic>,
) {
    // Offline validation has bootstrap vocabulary; admission must see actual
    // rows. A not-yet-migrated live world must not acquire fictional roles.
    let bootstrap = if live.is_none() {
        spellkave_format::parse_manifest(spellkave_schema::CORE_BOOTSTRAP_MANIFEST)
            .expect("schema bootstrap parses")
            .manifest
    } else {
        Manifest::default()
    };
    let mut roles: BTreeMap<String, String> = bootstrap
        .role_definitions
        .iter()
        .map(|r| (r.role_id.clone(), r.bundle_id.clone()))
        .collect();
    if let Some(live) = live {
        roles.extend(
            live.roles
                .iter()
                .map(|(id, r)| (id.clone(), r.bundle_id.clone())),
        );
    }
    for row in &manifest.role_definitions {
        if let Some(owner) = roles.get(&row.role_id) {
            if owner != &row.bundle_id {
                diags.push(diag_fatal(
                    MF_VAL_ROLE_DUPLICATE,
                    "role_definitions.role_id",
                    format!(
                        "role '{}' belongs to bundle '{}'; bundle '{}' cannot redeclare it",
                        row.role_id, owner, row.bundle_id
                    ),
                ));
            }
        } else {
            roles.insert(row.role_id.clone(), row.bundle_id.clone());
        }
    }
    let capabilities: BTreeMap<_, _> = manifest
        .capability_definitions
        .iter()
        .map(|r| (r.capability_id.as_str(), r.bundle_id.as_str()))
        .collect();
    let bootstrap_capabilities: BTreeSet<_> = bootstrap
        .capability_definitions
        .iter()
        .map(|r| r.capability_id.as_str())
        .collect();
    for (role_id, bundle_id, ids, kind) in manifest
        .role_definitions
        .iter()
        .map(|r| {
            (
                &r.role_id,
                &r.bundle_id,
                &r.capability_ids,
                "role_definitions",
            )
        })
        .chain(manifest.role_aggregations.iter().map(|r| {
            (
                &r.role_id,
                &r.bundle_id,
                &r.capability_ids,
                "role_aggregations",
            )
        }))
    {
        diags.push(diag_warning(crate::diagnostic_codes::MF_VAL_AUTHORITY_SENSITIVE_ROLE, kind,
            format!("role '{role_id}' is authority-sensitive: scoped writers need holder-or-escalate over bundle '{bundle_id}' and affected realised bindings need old/new bind admission; unknown reserved definitions and new clock authority require publisher/genesis; static validity is not live admission")));
        for id in ids {
            let resolves = capabilities
                .get(id.as_str())
                .is_some_and(|owner| scope.in_scope(bundle_id, owner))
                || bootstrap_capabilities.contains(id.as_str())
                || live.is_some_and(|v| v.capabilities.contains_key(id));
            if !resolves {
                diags.push(diag_fatal(MF_VAL_ROLE_CAPABILITY_REF, &format!("{kind}.capability_ids"),
                format!("role '{role_id}' capability '{id}' does not resolve in bundle '{bundle_id}' scope")));
            }
        }
    }
    for row in &manifest.capability_definitions {
        if row.capability_id.starts_with("frontier.") {
            diags.push(diag_warning(crate::diagnostic_codes::MF_VAL_AUTHORITY_SENSITIVE_ROLE, "capability_definitions",
                format!("reserved definition '{}' needs publisher/genesis for introduction and holder-or-escalate plus ownership for maintenance; a definition grants no authority", row.capability_id)));
        }
    }
    for row in &manifest.role_aggregations {
        let role_exists = roles.get(&row.role_id).is_some_and(|owner| {
            scope.in_scope(&row.bundle_id, owner)
                || live.is_some_and(|v| v.roles.contains_key(&row.role_id))
        });
        if row.capability_ids.is_empty() || !role_exists {
            diags.push(diag_fatal(MF_VAL_ROLE_AGGREGATION_REF, "role_aggregations.role_id",
                format!("aggregation '{}' from '{}' requires an existing role in scope and a nonempty capability list", row.role_id, row.bundle_id)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn role_and_aggregation_warn_that_static_validity_is_not_admission() {
        let manifest: Manifest = toml::from_str(
            r#"
schema_version = "1.0.0"
[[role_definitions]]
role_id = "custom/observer"
bundle_id = "custom"
display_name = "Observer"
description = "Synthetic"
capability_ids = []
[[role_aggregations]]
role_id = "dm"
bundle_id = "custom"
description = "Contribution"
capability_ids = ["frontier.read_hidden"]
"#,
        )
        .unwrap();
        let mut diags = Vec::new();
        validate(&manifest, None, &BundleScope::build(&manifest), &mut diags);
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.code == "MF-VAL-379" && d.severity == crate::Severity::Warning)
                .count(),
            2
        );
        assert!(diags
            .iter()
            .filter(|d| d.code == "MF-VAL-379")
            .all(|d| d.message.contains("holder-or-escalate")));
    }
}
