//! Early Job artifact shape refusal, before expensive companion rederivation.
//! Deserializing this public data shape is not artifact or authority admission.
use crate::{
    dm_document_authority::PrincipalPlan,
    dm_source::{CatalogDiagnostic, Source},
};
use serde::Deserialize;
use spellkave_schema::{
    ActorCapabilityRow, CapabilityDefinitionRow, CapabilityDefinitionTagRow, MetadataNamespaceRow,
    MetadataRow, RoleDefinitionRow,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub schema_version: u32,
    pub npc_kind: NpcKind,
    pub artifact_digest: String,
    pub source_content_hash: String,
    pub source: Source,
    pub manifest: Contribution,
    pub principal_plan: PrincipalPlan,
    pub locator: Locator,
    pub read_grants: Vec<ReadGrant>,
    pub reference_diagnostics: Vec<CatalogDiagnostic>,
}
#[derive(Debug, Deserialize)]
pub enum NpcKind {
    #[serde(rename = "dm")]
    Dm,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub schema_version: String,
    pub capability_definitions: Vec<CapabilityDefinitionRow>,
    pub capability_definition_tag: Vec<CapabilityDefinitionTagRow>,
    pub role_definitions: Vec<RoleDefinitionRow>,
    pub metadata_namespaces: Vec<MetadataNamespaceRow>,
    pub actor_capabilities: Vec<ActorCapabilityRow>,
    pub metadata_attachments: Vec<MetadataRow>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Locator {
    pub catalog_namespace: String,
    pub reader_binding_id: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SocketRole {
    Principal,
    Body,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Bundle,
    Entity,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadGrant {
    pub socket_role: SocketRole,
    pub target_kind: TargetKind,
    pub target_ref: String,
    pub namespace: String,
    pub read_capability_id: String,
    pub content_hash: String,
    pub publication_id: String,
}

pub fn validate_artifact_shape_json(input: &str) -> Vec<String> {
    let raw = match crate::dm_document_authority::unique_json(input) {
        Ok(raw) => raw,
        Err(errors) => return errors,
    };
    let artifact: Artifact = match serde_json::from_value(raw) {
        Ok(artifact) => artifact,
        Err(error) => return vec![format!("dm-artifact: invalid shape: {error}")],
    };
    if artifact.schema_version != 1
        || artifact.manifest.schema_version != "1.0.0"
        || !crate::dm_document_envelope::valid_hash(&artifact.artifact_digest)
        || !crate::dm_document_envelope::valid_hash(&artifact.source_content_hash)
    {
        return vec!["dm-artifact: invalid version or hash shape".into()];
    }
    vec![]
}
