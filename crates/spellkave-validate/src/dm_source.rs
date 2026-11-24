//! Complete Job source envelope and canonical reference facets. Deterministic
//! artifact rederivation, catalog artifact equality and read authority remain
//! mandatory companion checks of the single public compiler/parser.
use crate::{
    dm_document_authority::{unique_canonical_worlds_json, unique_json, PrincipalPlan, Reads},
    dm_duties::{parse_source_shape_json, DutySource},
    dm_duty_references::CanonicalWorlds,
    dm_presentation::Presentation,
    dm_reference_locators::ReferenceLocators,
};
use serde::Deserialize;
use serde_json::{json, Value};
use spellkave_schema::Manifest;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub schema_version: u32,
    pub bundle_id: String,
    pub body_key: String,
    pub principal_plan: PrincipalPlan,
    pub reads: Reads,
    pub catalog: CatalogArtifact,
    pub presentation: Presentation,
    pub duties: DutySource,
    pub reference_locators: ReferenceLocators,
}

/// Existing quest artifact carrier, not an extensible configuration slot.
/// Its source is checked by the existing authoritative quest-source schema;
/// world-author's existing parser rederives its full rows/hashes/diagnostics.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogArtifact {
    pub schema_version: u32,
    pub artifact_digest: String,
    pub source_content_hash: String,
    pub catalog_content_hash: String,
    pub source: Value,
    pub manifest: Manifest,
    pub mounts: Vec<CatalogMount>,
    pub reference_diagnostics: Vec<CatalogDiagnostic>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogMount {
    pub actor_key: String,
    pub npc_kind: String,
    pub schema_version: u32,
    pub document_namespace: String,
    pub catalog_namespace: String,
    pub catalog_content_hash: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogDiagnostic {
    pub severity: String,
    pub code: String,
    pub reference_kind: String,
    pub reference_id: String,
    #[serde(default, deserialize_with = "present_string")]
    pub bead: Option<String>,
}
fn present_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}

pub fn parse_source_json(input: &str) -> Result<Source, Vec<String>> {
    let raw = unique_json(input)?;
    let source: Source = serde_json::from_value(raw.clone())
        .map_err(|error| vec![format!("dm-source: invalid shape: {error}")])?;
    let mut errors = vec![];
    if source.schema_version != 1
        || source.catalog.schema_version != 1
        || source.bundle_id != source.principal_plan.bundle_id
        || source.body_key != source.principal_plan.body_key
        || source.bundle_id != source.duties.bundle_id
        || source.body_key != source.duties.body_key
        || source.catalog.source["bundle_id"] != source.bundle_id
        || source.catalog.source["catalog_read_capability_id"]
            != source.reads.catalog_read_capability_id
    {
        errors.push("dm-source: source/plan/duty/catalog identity mismatch".into());
    }
    if let Err(shape) = parse_source_shape_json(&raw["duties"].to_string()) {
        errors.extend(shape);
    }
    errors.extend(crate::dm_presentation::validate_presentation_json(
        &raw["presentation"].to_string(),
    ));
    errors.extend(crate::npc_documents::validate_quest_source_json(
        &source.catalog.source.to_string(),
    ));
    for check in [
        crate::dm_quest_references::validate_quest_board(
            &source.duties.quest_board,
            &source.body_key,
            &source.catalog.source,
        ),
        crate::dm_quest_references::validate_quest_board_lines(
            &source.duties.quest_board,
            &source.presentation.quest_board,
        ),
    ] {
        if let Err(closure) = check {
            errors.extend(closure);
        }
    }
    // Both exports originate in the same authored quest source. Run this
    // parity guard only after shape, ownership and exact key closure succeeded.
    if errors.is_empty() {
        for id in &source.duties.quest_board.quest_refs {
            let quest = source.catalog.source["quests"]
                .as_array()
                .and_then(|quests| quests.iter().find(|quest| quest["id"] == *id));
            if quest.is_some_and(|quest| {
                raw["presentation"]["quest_board"]["lines"][id] != quest["prompt_lines"]
            }) {
                errors.push(format!(
                    "dm-quest-board: presentation differs from catalog prompt lines for {id}"
                ));
            }
        }
    }
    for mount in &source.catalog.mounts {
        if mount.schema_version != 1
            || mount.npc_kind != "quest-giver"
            || mount.catalog_namespace != source.catalog.source["catalog_namespace"]
            || mount.catalog_content_hash != source.catalog.catalog_content_hash
        {
            errors.push("dm-source: invalid existing quest mount shape/link".into());
        }
    }
    if errors.is_empty() {
        Ok(source)
    } else {
        Err(errors)
    }
}

/// Full canonical roots use the publisher's existing 64 MiB total / 32 MiB
/// per-root world ceilings. They never use the 8 MiB authority projection.
/// Intermediate facet, not an admission gate: must not be wired as one until
/// companion catalog/hash rederivation and read-authority checks land.
pub fn validate_source_references_json(input: &str, canonical_worlds: &str) -> Vec<String> {
    match references(input, canonical_worlds) {
        Ok(()) => vec![],
        Err(errors) => errors,
    }
}

fn references(input: &str, canonical_worlds: &str) -> Result<(), Vec<String>> {
    let source = parse_source_json(input)?;
    let raw_worlds = unique_canonical_worlds_json(canonical_worlds)?;
    let values = raw_worlds
        .as_array()
        .filter(|rows| !rows.is_empty())
        .ok_or_else(|| vec!["dm-source: canonical roots are missing".into()])?;
    for root in values {
        if serde_json::to_vec(root)
            .map_err(|error| vec![error.to_string()])?
            .len()
            > 32 * 1024 * 1024
        {
            return Err(vec![
                "dm-source: canonical root exceeds publisher world-root byte bound".into(),
            ]);
        }
    }
    let manifests: Vec<Manifest> = serde_json::from_value(raw_worlds)
        .map_err(|error| vec![format!("dm-source: invalid typed canonical roots: {error}")])?;
    let worlds = CanonicalWorlds::new(&manifests)?;
    // Re-admit every Job locator rather than trusting a typed caller's fields.
    let raw = unique_json(input)?;
    let locators = crate::dm_reference_locators::admit_reference_locators_json(
        &raw["reference_locators"].to_string(),
        &source.duties,
        &worlds,
    )?;
    let metadata =
        crate::dm_reference_metadata::admit_reference_metadata(&locators, &source.duties, &worlds)?;
    let mut errors = vec![];
    for check in [
        crate::dm_consequence_references::validate_consequence_references(&source.duties, &worlds),
        crate::dm_reference_keys::validate_duty_reference_keys(&source.duties, &worlds),
        crate::dm_menu_references::validate_canonical_menus(
            &source.duties,
            &locators,
            &metadata,
            &worlds,
        ),
        crate::dm_training_references::validate_training_references(
            &source.duties,
            &metadata,
            &locators,
            &worlds,
        ),
        crate::dm_kit_references::validate_kit_references(&source.duties, &worlds),
        crate::dm_quest_references::validate_quest_prerequisites(
            &source.duties,
            &source.catalog.source,
        ),
        crate::dm_route_references::validate_duty_routes(
            &source.duties,
            &source.principal_plan,
            &worlds,
        ),
        crate::dm_route_references::validate_dispatched_action_routes(
            &source.duties,
            &source.principal_plan,
            &worlds,
        ),
    ] {
        if let Err(failures) = check {
            errors.extend(failures);
        }
    }
    // The exact independently admitted shared catalog must already be present.
    // Do not compose an alternate nested catalog into the reference world.
    let namespace = source.catalog.source["catalog_namespace"]
        .as_str()
        .ok_or_else(|| vec!["dm-source: missing catalog namespace".into()])?;
    let identity = json!({"target_kind":"bundle","target_ref":source.bundle_id,"namespace":namespace,"bundle_id":source.bundle_id});
    match worlds.keyed_row("metadata_attachments", &identity) {
        Ok(actual) => {
            let expected = source
                .catalog
                .manifest
                .metadata_attachments
                .iter()
                .find(|row| {
                    row.target_kind == "bundle"
                        && row.target_ref == source.bundle_id
                        && row.namespace == namespace
                        && row.bundle_id == source.bundle_id
                });
            if expected.is_none_or(|row| serde_json::to_value(row).ok().as_ref() != Some(actual)) {
                errors.push(
                    "dm-source: embedded catalog differs from independently admitted publication"
                        .into(),
                );
            }
        }
        Err(error) => errors.push(error),
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
