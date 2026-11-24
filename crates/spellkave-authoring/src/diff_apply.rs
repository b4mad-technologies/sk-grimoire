//! `sk diff` and `sk apply` classification and delta building (ADR-0145).
//!
//! The caller (`@spellkave/tool-authoring`, through `spellkave-authoring-wasm`)
//! reads the root, the include files [`applyset_root_info`] names, and the
//! applyset memory the server holds. [`classify_and_build_delta`] then does
//! everything else in Rust, with the same row identity and hash the server
//! uses (`spellkave_schema::row_identity`):
//!
//! - classification against the memory, kubectl client-side apply style:
//!   `create` (not in memory), `configure` (in memory, other hash — also when
//!   the row moved), `move` (same hash, other part), `unchanged`, and `prune`
//!   (in memory, in no part of the files);
//! - the part lists to send: only the parts whose resident rows differ from
//!   the memory. Without `prune`, a prune candidate stays recorded in its old
//!   part, so a later `--prune` can still retract it;
//! - the delta TOML: the `create` and `configure` rows as the files hold them,
//!   plus keyed `[[retractions]]` for `prune` rows when `prune` is requested;
//! - refusals before anything is sent: every fatal diagnostic of
//!   `spellkave-validate` over the whole built root
//!   (dependencies included, so references to unchanged rows resolve) plus
//!   the built retractions. `--prune` is refused while the base is unknown.
//!
//! Only the root's own rows are classified and sent; dependency bundles are
//! used for validation only (D2).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use spellkave_schema::row_identity::{
    canonical_json, row_hash, row_key_of, row_kind_key_fields, SET_LEVEL_FIELD,
};
use spellkave_schema::{Manifest, RetractionRow};
use spellkave_validate::{Diagnostic, Severity};

use crate::applyset::{build_applyset_parts, ApplysetSources, PartEntry, PartSource};

/// What the caller needs before it can read the rest: the applyset name and
/// the include paths in declaration order (the order residence follows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplysetRootInfo {
    pub applyset: Option<String>,
    pub includes: Vec<String>,
    /// Bundles the root declares: the ones the caller exports for drift.
    pub bundles: Vec<String>,
}

pub fn applyset_root_info(root_toml: &str) -> Result<ApplysetRootInfo, String> {
    let manifest = spellkave_format::parse_manifest(root_toml)
        .map_err(|err| err.to_string())?
        .manifest;
    Ok(ApplysetRootInfo {
        applyset: manifest.applyset,
        includes: manifest.includes.into_iter().map(|i| i.path).collect(),
        bundles: manifest.bundles.into_iter().map(|b| b.bundle_id).collect(),
    })
}

/// Whether the memory describes the world (the server's `ApplysetBaseState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BaseState {
    Known,
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryPart {
    pub path: String,
    pub entries: Vec<PartEntry>,
}

/// The server's applyset memory as read by the caller.
#[derive(Debug, Clone, Deserialize)]
pub struct Memory {
    #[serde(default)]
    pub export_hashes: Vec<ExportHash>,
    pub root_path: String,
    pub generation: u64,
    pub base_state: BaseState,
    pub parts: Vec<MemoryPart>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiffInput {
    /// The applying principal resolves the authored $sender macro for comparison.
    #[serde(default)]
    pub applying_identity: Option<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub not_exported: Vec<NotExported>,
    #[serde(default)]
    pub places_read: bool,
    /// `root.path` is the root's file name (the root part's path); the
    /// include paths are relative to its directory.
    pub root: PartSource,
    /// The root path the server records (`r_apply_bundle` `root_path`): the
    /// root file relative to the worktree root, as `sk dev seed` passes it.
    pub root_path: String,
    /// Include files keyed by the path the root declares.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// `None` for an applyset the world has never seen.
    #[serde(default)]
    pub memory: Option<Memory>,
    #[serde(default)]
    pub prune: bool,
    /// Dependency bundles (root plus includes), for validation only.
    #[serde(default)]
    pub dependencies: Vec<ApplysetSources>,
    /// Live exports (`r_export_manifest` `manifest_json`) of the bundles the
    /// caller could export, for adoption, comparable-world conflict checks,
    /// informational drift and prune place reconstruction.
    #[serde(default)]
    pub live: Vec<LiveExport>,
    /// Owner-subscription places the caller read for prune reconstruction
    /// (`entity_location` / `actor_spatial_location`). Absent from the list
    /// means no live location value was available.
    #[serde(default)]
    pub live_places: Vec<LivePlace>,
    #[serde(default)]
    pub extent_scope: Option<crate::diff_apply_extent::ExtentScope>,
    #[serde(default)]
    pub authority: Option<crate::diff_apply_authority::AuthoritySnapshot>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LiveExport {
    pub bundle_id: String,
    pub manifest_json: String,
}

/// Live place of one `entities` or `actor_spatial_locations` row, from the
/// owner subscription for drift and prune. `place_node_id` is empty when a containment row
/// places the entity (the file form).
#[derive(Debug, Clone, Deserialize)]
pub struct LivePlace {
    pub kind: String,
    pub key: String,
    pub place_node_id: String,
}

/// What the live export says about a row (information only: never changes
/// the class or the exit code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveState {
    /// The exported row hashes like the file row (configure/unchanged), or
    /// a reconstructed prune place hashes like the memory hash.
    Same,
    /// The exported row differs from the file row, or a reconstructed prune
    /// place differs from the memory hash (the entity was moved).
    Drift,
    /// No exported counterpart: its bundle was not exported, or the export
    /// does not reconstruct this kind (`EXPORT_COVERAGE_GAP`). For a prune
    /// place check, also when no live location value was passed.
    NotComparable,
}

/// A row pair to show as a diff: the live value and the file value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RowValues {
    pub kind: String,
    pub key: String,
    pub part: String,
    pub live: String,
    pub file: String,
}

/// A live row this applyset never applied and the files do not name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveOnlyRow {
    pub kind: String,
    pub key: String,
    pub bundle_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RowClass {
    Create,
    Adopt,
    Configure,
    Move,
    Prune,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClassifiedRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world_changed: Option<WorldChanged>,
    pub kind: String,
    pub key: String,
    pub bundle_id: Option<String>,
    pub class: RowClass,
    /// The part the row resides in; for `prune`, the part the memory names.
    pub part: String,
    /// For `move` and a moved `configure`: the part the memory names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_part: Option<String>,
    /// The file hash (for `prune`, the recorded hash).
    pub hash: String,
    /// Live-export comparison for `configure` and `unchanged` rows, and the
    /// reconstructed-place comparison for prune `entities` /
    /// `actor_spatial_locations` rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<LiveState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Warning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SendPart {
    pub path: String,
    pub entries: Vec<PartEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiffOutput {
    pub conflicts: Vec<Conflict>,
    pub not_checked: Vec<NotChecked>,
    /// Seal upserts whose live drift was not checked; the client requires --force.
    pub seal_drift_not_checked: Vec<String>,
    pub applyset: String,
    pub root_path: String,
    /// The generation read (0 for no memory): `expected_generation` of the call.
    pub generation: u64,
    pub base_state: Option<BaseState>,
    pub rows: Vec<ClassifiedRow>,
    /// Changed parts, in the `r_apply_bundle` `parts_json` shape.
    pub parts: Vec<SendPart>,
    pub delta_toml: String,
    /// Whether an apply would call the server (rows to send or parts changed).
    pub send: bool,
    /// Fatal findings: an apply refuses to send while any is present.
    pub diagnostics: Vec<Diagnostic>,
    pub warnings: Vec<Warning>,
    /// Live and file values of every `configure` row the export can read and
    /// of every drifted `unchanged` row, as pretty JSON.
    pub values: Vec<RowValues>,
    pub live_only: Vec<LiveOnlyRow>,
    pub extent_verdicts: Vec<crate::diff_apply_extent::ExtentVerdict>,
    pub placement_server_decides: Vec<LiveOnlyRow>,
    pub authority_verdicts: Vec<crate::diff_apply_authority::AuthorityVerdict>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportHash {
    pub kind: String,
    pub key: String,
    pub export_hash: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct NotExported {
    pub bundle_id: String,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum WorldChanged {
    Yes,
    No,
    Unknown(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    pub kind: String,
    pub key: String,
    pub part: String,
    pub class: RowClass,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotChecked {
    pub kind: String,
    pub key: String,
    pub reason: String,
}

/// Classify the root's rows against the memory and build the delta.
pub fn classify_and_build_delta(input: &DiffInput) -> Result<DiffOutput, String> {
    let info = applyset_root_info(&input.root.toml)
        .map_err(|err| format!("{}: {err}", input.root.path))?;
    let mut fragments = Vec::with_capacity(info.includes.len());
    for path in &info.includes {
        let toml = input
            .files
            .get(path)
            .ok_or_else(|| format!("{}: include `{path}` was not read", input.root.path))?;
        fragments.push(PartSource {
            path: path.clone(),
            toml: toml.clone(),
        });
    }
    let sources = ApplysetSources {
        root: input.root.clone(),
        fragments,
    };
    let built = build_applyset_parts(&sources)?;

    let mut warnings = Vec::new();
    let mut diagnostics = Vec::new();

    // Memory: (kind, key) -> (part, entry).
    let mut recorded: BTreeMap<(String, String), (String, PartEntry)> = BTreeMap::new();
    if let Some(memory) = &input.memory {
        for part in &memory.parts {
            for entry in &part.entries {
                recorded.insert(
                    (entry.kind.clone(), entry.key.clone()),
                    (part.path.clone(), entry.clone()),
                );
            }
        }
        if memory.base_state == BaseState::Unknown {
            warnings.push(warning(
                spellkave_validate::diagnostic_codes::MF_APPLYSET_BASE_UNKNOWN,
                "the last apply of this applyset did not finish or its rows were cleared; the memory may not describe the world",
                None,
            ));
            if input.prune {
                diagnostics.push(fatal(
                    spellkave_validate::diagnostic_codes::MF_APPLYSET_PRUNE_UNKNOWN,
                    "--prune is refused while the applyset base is unknown: the memory may name rows the world no longer holds",
                    None,
                ));
            }
        }
        if memory.root_path != input.root_path {
            warnings.push(warning(
                spellkave_validate::diagnostic_codes::MF_APPLYSET_ROOT_MOVED,
                "the applyset memory names another root path; an apply needs --force to take the name over",
                Some(serde_json::json!({
                    "stored_root_path": memory.root_path,
                    "root_path": input.root_path,
                })),
            ));
        }
    } else {
        warnings.push(warning(
            spellkave_validate::diagnostic_codes::MF_APPLYSET_ADOPT,
            "the world has no memory of this applyset: rows the world already holds are adopted without rewriting them when comparable values agree; unreadable rows are creates (upserts), and rows only the world holds are left alone",
            None,
        ));
    }
    for dup in &built.duplicates {
        warnings.push(warning(
            spellkave_validate::diagnostic_codes::MF_APPLYSET_DUPLICATE,
            &format!(
                "{} `{}` appears in {} parts; the last one wins",
                dup.kind,
                dup.key,
                dup.parts.len()
            ),
            Some(serde_json::json!({ "parts": dup.parts, "identical": dup.identical })),
        ));
    }
    for split in &built.split_sets {
        warnings.push(warning(
            spellkave_validate::diagnostic_codes::MF_APPLYSET_SPLIT_SET,
            &format!(
                "{} `{}` is assembled from blocks in {} parts; its hash covers all of them",
                split.kind,
                split.key,
                split.parts.len()
            ),
            Some(serde_json::json!({ "parts": split.parts })),
        ));
    }
    // One warning per kind, not per part: a big root has thousands of parts.
    let mut untrackable: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for gap in &built.untrackable {
        let entry = untrackable.entry(gap.kind.as_str()).or_default();
        entry.0 += gap.count;
        entry.1 += 1;
    }
    for (kind, (count, parts)) in untrackable {
        warnings.push(warning(
            spellkave_validate::diagnostic_codes::MF_APPLYSET_UNTRACKABLE,
            &format!(
                "{count} {kind} row(s) in {parts} part(s) have no row identity; they are not compared or sent"
            ),
            Some(serde_json::json!({ "kind": kind, "count": count, "parts": parts })),
        ));
    }

    let exported_rows = live_rows(&input.live)?;
    // Classify the resident rows.
    let mut rows = Vec::new();
    let mut in_files = BTreeSet::new();
    let mut to_send = BTreeSet::new();
    for part in &built.parts {
        for entry in &part.entries {
            let id = (entry.kind.clone(), entry.key.clone());
            in_files.insert(id.clone());
            let (class, from_part) = match recorded.get(&id) {
                None if exported_rows.contains_key(&id) => (RowClass::Adopt, None),
                None => (RowClass::Create, None),
                Some((old_part, old)) => {
                    let moved = (old_part != &part.path).then(|| old_part.clone());
                    if old.hash != entry.hash {
                        (RowClass::Configure, moved)
                    } else if moved.is_some() {
                        (RowClass::Move, moved)
                    } else {
                        (RowClass::Unchanged, None)
                    }
                }
            };
            if matches!(class, RowClass::Create | RowClass::Configure) {
                to_send.insert(id);
            }
            rows.push(ClassifiedRow {
                world_changed: None,
                kind: entry.kind.clone(),
                key: entry.key.clone(),
                bundle_id: entry.bundle_id.clone(),
                class,
                part: part.path.clone(),
                from_part,
                hash: entry.hash.clone(),
                live: None,
            });
        }
    }
    let mut prune_rows: Vec<&(String, PartEntry)> = Vec::new();
    for (id, recorded_row) in &recorded {
        if in_files.contains(id) {
            continue;
        }
        let (part, entry) = recorded_row;
        prune_rows.push(recorded_row);
        rows.push(ClassifiedRow {
            world_changed: None,
            kind: entry.kind.clone(),
            key: entry.key.clone(),
            bundle_id: entry.bundle_id.clone(),
            class: RowClass::Prune,
            part: part.clone(),
            from_part: None,
            hash: entry.hash.clone(),
            live: None,
        });
    }

    let retracted_entities: BTreeSet<String> = prune_rows
        .iter()
        .filter(|(_, entry)| entry.kind == "entities")
        .filter_map(|(_, entry)| {
            entry
                .bundle_id
                .as_deref()
                .map(|bundle_id| manifest_entity_key(bundle_id, &entry.key))
        })
        .collect();

    // The delta: resident copies of the rows to send, then retractions.
    // An entity retraction closes over the instance rows whose storage keys
    // name it. A containment move is an upsert of the new containment row,
    // so its old row likewise needs no tombstone.
    let mut retractions = Vec::new();
    if input.prune {
        for (_, entry) in &prune_rows {
            let covered_by_entity = is_instance_kind(&entry.kind)
                && key_entity_manifest_keys(&entry.kind, &entry.key, entry.bundle_id.as_deref())
                    .iter()
                    .any(|entity| retracted_entities.contains(entity));
            let moved_containment = entry.kind == "entity_containment"
                && containment_is_replaced(&entry.key, entry.bundle_id.as_deref(), &rows);
            if covered_by_entity || moved_containment {
                continue;
            }
            if is_instance_kind(&entry.kind) {
                let entity = prune_instance_entity_name(&entry.kind, &entry.key);
                diagnostics.push(fatal(
                    spellkave_validate::diagnostic_codes::MF_APPLYSET_PRUNE_INSTANCE,
                    &format!(
                        "{} row `{}` can be removed only together with its entity `{entity}`",
                        entry.kind, entry.key
                    ),
                    None,
                ));
            }
            retractions.push(retraction_for(
                &entry.kind,
                &entry.key,
                entry.bundle_id.as_deref(),
            ));
        }
    }
    // Compare file/live values through one normalization, keeping actual placement.
    let live_rows = exported_rows;
    let unmasked_live = unmasked_live_rows(&input.live)?;
    let live_places: BTreeMap<RowId, String> = input
        .live_places
        .iter()
        .map(|p| ((p.kind.clone(), p.key.clone()), p.place_node_id.clone()))
        .collect();
    let exported: BTreeSet<&str> = input.live.iter().map(|l| l.bundle_id.as_str()).collect();
    let (file_rows, _) = resident_rows(&sources, &in_files)?;
    let contained: BTreeSet<&str> = file_rows
        .iter()
        .filter(|((kind, _), _)| kind == "entity_containment")
        .filter_map(|(_, blocks)| blocks[0].get("entity_key").and_then(Value::as_str))
        .collect();
    let baselines: BTreeMap<RowId, &str> = input
        .memory
        .iter()
        .flat_map(|m| &m.export_hashes)
        .map(|h| ((h.kind.clone(), h.key.clone()), h.export_hash.as_str()))
        .collect();
    let mut conflicts = Vec::new();
    let mut not_checked = Vec::new();
    let mut want_values = BTreeSet::new();
    let mut comparison_values = BTreeMap::new();
    let mut file_hashes = BTreeMap::new();
    for row in &mut rows {
        let id = (row.kind.clone(), row.key.clone());
        if let Some(file) = file_rows.get(&id) {
            let file_place =
                (row.kind == "entities" && contained.contains(row.key.as_str())).then_some("");
            let file_value = drift_comparable_value(
                &row.kind,
                file,
                file_place,
                input.applying_identity.as_deref(),
            );
            let file_hash = row_hash(&file_value);
            file_hashes.insert(id.clone(), file_hash.clone());
            let live_value = live_rows.get(&id).map(|(_, value)| {
                drift_comparable_value(
                    &row.kind,
                    std::slice::from_ref(unmasked_live.get(&id).unwrap_or(value)),
                    live_places.get(&id).map(String::as_str),
                    input.applying_identity.as_deref(),
                )
            });
            let reason = if !row
                .bundle_id
                .as_deref()
                .is_some_and(|b| exported.contains(b))
            {
                Some(
                    input
                        .not_exported
                        .iter()
                        .find(|e| Some(e.bundle_id.as_str()) == row.bundle_id.as_deref())
                        .map(|e| e.reason.clone())
                        .unwrap_or_else(|| "bundle not exported".into()),
                )
            } else if spellkave_schema::export_round_trip::EXPORT_COVERAGE_GAP
                .contains(&row.kind.as_str())
            {
                Some(format!("the export does not reconstruct {}", row.kind))
            } else if row.kind == "role_bindings"
                && file[0].get("subject").and_then(Value::as_str) == Some("$sender")
                && input.applying_identity.is_none()
            {
                Some("applying identity not read".to_owned())
            } else if matches!(row.kind.as_str(), "entities" | "actor_spatial_locations")
                && !input.places_read
            {
                Some("live place not read".into())
            } else if row.class == RowClass::Configure && !baselines.contains_key(&id) {
                Some("no comparable hash recorded (applied before diff-drift-decision)".into())
            } else {
                None
            };
            if matches!(
                row.class,
                RowClass::Configure | RowClass::Adopt | RowClass::Create
            ) {
                let changed = if let Some(reason) = reason {
                    not_checked.push(NotChecked {
                        kind: row.kind.clone(),
                        key: row.key.clone(),
                        reason: reason.clone(),
                    });
                    WorldChanged::Unknown(reason)
                } else if row.class == RowClass::Create {
                    WorldChanged::No
                } else {
                    let baseline = if row.class == RowClass::Adopt {
                        file_hash.as_str()
                    } else {
                        baselines[&id]
                    };
                    if live_value.as_ref().is_some_and(|v| row_hash(v) == baseline) {
                        WorldChanged::No
                    } else {
                        WorldChanged::Yes
                    }
                };
                if changed == WorldChanged::Yes {
                    conflicts.push(Conflict {
                        kind: row.kind.clone(),
                        key: row.key.clone(),
                        part: row.part.clone(),
                        class: row.class,
                    });
                    if input.force {
                        to_send.insert(id.clone());
                    } else {
                        to_send.remove(&id);
                    }
                    want_values.insert(id.clone());
                }
                row.world_changed = Some(changed);
            }
            if matches!(
                row.class,
                RowClass::Configure | RowClass::Unchanged | RowClass::Adopt
            ) {
                let state = match &live_value {
                    Some(value) if row_hash(value) == file_hash => LiveState::Same,
                    Some(_) => LiveState::Drift,
                    None => LiveState::NotComparable,
                };
                row.live = Some(state);
                if state != LiveState::NotComparable
                    && (row.class == RowClass::Configure || state == LiveState::Drift)
                {
                    want_values.insert(id.clone());
                }
            }
            comparison_values.insert(id, (live_value, file_value));
        } else if row.class == RowClass::Prune
            && matches!(row.kind.as_str(), "entities" | "actor_spatial_locations")
        {
            row.live = Some(prune_place_live(
                &row.kind,
                &row.hash,
                unmasked_live.get(&id),
                live_places.get(&id).map(String::as_str),
            ));
        }
    }
    let live_only = live_rows
        .iter()
        .filter(|(id, _)| !in_files.contains(*id) && !recorded.contains_key(*id))
        .filter(|(_, (_, row))| {
            row.get("bundle_id")
                .and_then(Value::as_str)
                .is_none_or(|b| exported.contains(b))
        })
        .map(|((kind, key), (_, row))| LiveOnlyRow {
            kind: kind.clone(),
            key: key.clone(),
            bundle_id: row
                .get("bundle_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
        .collect();

    // Parts to send: the files' parts, plus (without prune) every prune
    // candidate kept in the part the memory names.
    let mut desired: BTreeMap<String, Vec<PartEntry>> = built
        .parts
        .iter()
        .map(|p| (p.path.clone(), p.entries.clone()))
        .collect();
    if !input.prune {
        for (part, entry) in &prune_rows {
            desired.entry(part.clone()).or_default().push(entry.clone());
        }
    }
    let mut current: BTreeMap<String, Vec<PartEntry>> = BTreeMap::new();
    if let Some(memory) = &input.memory {
        for part in &memory.parts {
            current.insert(part.path.clone(), part.entries.clone());
        }
    }
    let mut parts = Vec::new();
    let all_paths: BTreeSet<&String> = desired.keys().chain(current.keys()).collect();
    for path in all_paths {
        let want = desired.get(path).cloned().unwrap_or_default();
        let have = current.get(path).cloned().unwrap_or_default();
        if sorted_entries(&want) != sorted_entries(&have) {
            parts.push(SendPart {
                path: path.clone(),
                entries: want,
            });
        }
    }

    let adopted_ids: BTreeSet<RowId> = rows
        .iter()
        .filter(|row| row.class == RowClass::Adopt)
        .map(|row| (row.kind.clone(), row.key.clone()))
        .collect();
    for part in &mut parts {
        for entry in &mut part.entries {
            let id = (entry.kind.clone(), entry.key.clone());
            if to_send.contains(&id) || adopted_ids.contains(&id) {
                entry.export_hash = file_hashes.get(&id).cloned();
            }
        }
    }
    let (mut resident, order) = resident_rows(&sources, &to_send)?;
    let values = rows
        .iter()
        .filter_map(|row| {
            let id = (row.kind.clone(), row.key.clone());
            if !want_values.contains(&id) {
                return None;
            }
            comparison_values.get(&id).map(|(live, file)| RowValues {
                kind: row.kind.clone(),
                key: row.key.clone(),
                part: row.part.clone(),
                live: live
                    .as_ref()
                    .map(pretty)
                    .unwrap_or_else(|| "(absent in the world)".into()),
                file: pretty(file),
            })
        })
        .collect();
    let delta = build_delta(&mut resident, &order, &to_send, &retractions)?;
    let delta_toml = spellkave_format::render_manifest(&delta)
        .map_err(|err| format!("delta render failed: {err}"))?;

    // Validate the whole built root (plus dependencies) with the retractions.
    diagnostics.extend(validate_built_root(
        &sources,
        &input.dependencies,
        &retractions,
    )?);

    let live_keys: BTreeSet<_> = input
        .live
        .iter()
        .filter_map(|export| serde_json::from_str::<Manifest>(&export.manifest_json).ok())
        .flat_map(|manifest| {
            spellkave_schema::row_identity::manifest_row_identities(&manifest).rows
        })
        .map(|row| {
            crate::diff_apply_extent::ownership_key(&row.kind, &row.key, row.bundle_id.as_deref())
        })
        .collect();
    let extent_verdicts = crate::diff_apply_extent::memory_verdicts(
        input.extent_scope.as_ref(),
        &built.applyset,
        &info.bundles,
        &rows,
        &live_keys,
        input.prune,
    );
    diagnostics.extend(extent_verdicts.iter().map(|row| {
        fatal(
            &row.code,
            "refused by extent",
            Some(serde_json::json!({"kind": row.kind, "key": row.key, "owners": row.owners})),
        )
    }));
    let authority_verdicts = crate::diff_apply_authority::authority_verdicts(
        input.authority.as_ref(),
        &delta,
        &rows,
        input.prune,
    );
    for row in &authority_verdicts {
        if let Some(code) = &row.code {
            diagnostics.push(fatal(
                code,
                &row.reason,
                Some(serde_json::to_value(row).map_err(|e| e.to_string())?),
            ));
        } else {
            warnings.push(warning(
                "AUTHORITY_SERVER_DECIDES",
                &row.reason,
                Some(serde_json::to_value(row).map_err(|e| e.to_string())?),
            ));
        }
    }
    let placement_server_decides = rows
        .iter()
        .filter(|row| {
            matches!(row.class, RowClass::Create | RowClass::Configure)
                || (input.prune && row.class == RowClass::Prune)
        })
        .map(|row| LiveOnlyRow {
            kind: row.kind.clone(),
            key: row.key.clone(),
            bundle_id: row.bundle_id.clone(),
        })
        .collect();
    // Explicit scoped apply also lists ownership. Legacy seed memory with
    // identical hashes still needs this one metadata-only publisher step.
    let ownership_changed = input.extent_scope.as_ref().is_some_and(|scope| {
        scope.publisher
            && !info.bundles.is_empty()
            && scope
                .memory
                .iter()
                .find(|root| root.applyset == built.applyset)
                .is_some_and(|root| root.bundle_ids.as_ref() != Some(&info.bundles))
    });
    let send =
        !to_send.is_empty() || !retractions.is_empty() || !parts.is_empty() || ownership_changed;
    rows.sort_by(|a, b| {
        (a.class, &a.part, &a.kind, &a.key).cmp(&(b.class, &b.part, &b.kind, &b.key))
    });
    let seal_drift_not_checked = not_checked
        .iter()
        .filter(|row| {
            row.kind == "place_seals" && to_send.contains(&(row.kind.clone(), row.key.clone()))
        })
        .map(|row| row.key.clone())
        .collect();
    Ok(DiffOutput {
        seal_drift_not_checked,
        applyset: built.applyset,
        root_path: input.root_path.clone(),
        generation: input.memory.as_ref().map_or(0, |m| m.generation),
        base_state: input.memory.as_ref().map(|m| m.base_state),
        rows,
        conflicts,
        not_checked,
        parts,
        delta_toml,
        send,
        diagnostics,
        warnings,
        values,
        live_only,
        extent_verdicts,
        placement_server_decides,
        authority_verdicts,
    })
}

/// JSON in (`DiffInput`), JSON out (`DiffOutput`) — the wasm boundary.
pub fn classify_and_build_delta_json(input_json: &str) -> Result<String, String> {
    let input: DiffInput =
        serde_json::from_str(input_json).map_err(|err| format!("diff input: {err}"))?;
    let output = classify_and_build_delta(&input)?;
    serde_json::to_string(&output).map_err(|err| err.to_string())
}

pub fn applyset_root_info_json(root_toml: &str) -> Result<String, String> {
    serde_json::to_string(&applyset_root_info(root_toml)?).map_err(|err| err.to_string())
}

fn sorted_entries(entries: &[PartEntry]) -> Vec<(&str, &str, &str)> {
    let mut out: Vec<_> = entries
        .iter()
        .map(|e| (e.kind.as_str(), e.key.as_str(), e.hash.as_str()))
        .collect();
    out.sort();
    out
}

/// Retraction vocabulary for supported row kinds. Other kinds keep their
/// manifest field name and fail validation with `MF_VAL_RETRACTION_ROW_KIND`.
///
/// Most supported composite keys are JSON pairs in the row-identity registry
/// and colon-joined pairs in a retraction. [`colon_join_two_element_key`]
/// converts the shared shape.
fn retraction_for(kind: &str, key: &str, bundle_id: Option<&str>) -> RetractionRow {
    let row_kind = match kind {
        "role_definitions" => "role_definition",
        "role_aggregations" => "role_aggregation",
        "entities" => "entity",
        "entity_definitions" => "entity_definition",
        "outcome_graphs" => "outcome_graph",
        "outcome_graph_nodes" => "outcome_graph_node",
        "outcome_graph_edges" => "outcome_graph_edge",
        "consequence_templates" => "consequence_template",
        "roll_tables" => "roll_table",
        "roll_table_entries" => "roll_table_entry",
        // Class beta (design D3): the identity IS the template id already
        // (no JSON array to join); the server's cascade clears the whole
        // ordered set for that template.
        SET_LEVEL_FIELD => "consequence_template_frame_write",
        other => other,
    };
    let authored_key = if kind == "entities" {
        match bundle_id {
            Some(bundle_id) => format!("{bundle_id}:{key}"),
            // The row's bundle id is mandatory in the manifest schema. Keep a
            // plain fallback only for direct unit callers of this private helper.
            None => key.to_owned(),
        }
    } else if kind == "role_aggregations" {
        // This new table stores the schema tuple verbatim, avoiding colon collisions.
        key.to_owned()
    } else {
        colon_join_two_element_key(key).unwrap_or_else(|| key.to_owned())
    };
    RetractionRow {
        row_kind: row_kind.to_owned(),
        authored_key,
    }
}

const INSTANCE_KINDS: &[&str] = &[
    "entity_containment",
    "object_state",
    "entity_resource_bound",
    "entity_tag",
    "actor_equipment_slots",
    "actors",
    "actor_spatial_locations",
    "actor_identities",
    "actor_capabilities",
];

fn is_instance_kind(kind: &str) -> bool {
    INSTANCE_KINDS.contains(&kind)
}

/// The entity a 1.5 diagnostic names. Containment keys list the container
/// first; the row can be removed only with the contained entity.
fn prune_instance_entity_name(kind: &str, key: &str) -> String {
    let refs = key_entity_refs(kind, key);
    let named = if kind == "entity_containment" {
        refs.get(1)
    } else {
        refs.first()
    };
    named.cloned().unwrap_or_else(|| "<unknown entity>".into())
}

/// The entity-bearing members of an instance row's storage key. This derives
/// field positions from the schema registry rather than duplicating key order.
fn key_entity_refs(kind: &str, key: &str) -> Vec<String> {
    let Some(fields) = row_kind_key_fields(kind) else {
        return Vec::new();
    };
    let values = key_values(key, fields.len());
    fields
        .iter()
        .enumerate()
        .filter(|(_, field)| {
            matches!(
                **field,
                "entity_key" | "actor_entity_key" | "container_entity_key"
            )
        })
        .filter_map(|(index, _)| values.get(index).cloned())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Resolve an authored entity reference exactly as the server does:
/// `resolve_manifest_entity_id` uses the row bundle when present, while
/// `live_authored_entity_id` derives a no-bundle containment key from its
/// leading path segment. That containment path is never treated as an
/// explicitly-qualified `bundle:key`.
fn manifest_entity_key(bundle_id: &str, entity_key: &str) -> String {
    format!("{bundle_id}:{entity_key}")
}

fn resolve_entity_ref(entity_key: &str, row_bundle_id: Option<&str>) -> String {
    if let Some(bundle_id) = row_bundle_id {
        if entity_key.contains(':') {
            entity_key.to_owned()
        } else {
            manifest_entity_key(bundle_id, entity_key)
        }
    } else {
        let bundle_id = entity_key.split('/').next().unwrap_or_default();
        manifest_entity_key(bundle_id, entity_key)
    }
}

fn key_entity_manifest_keys(kind: &str, key: &str, row_bundle_id: Option<&str>) -> Vec<String> {
    key_entity_refs(kind, key)
        .into_iter()
        .map(|entity_key| resolve_entity_ref(&entity_key, row_bundle_id))
        .collect()
}

fn containment_is_replaced(key: &str, row_bundle_id: Option<&str>, rows: &[ClassifiedRow]) -> bool {
    let Some(entity) = key_entity_manifest_keys("entity_containment", key, row_bundle_id)
        .get(1)
        .cloned()
    else {
        return false;
    };
    rows.iter().any(|row| {
        row.kind == "entity_containment"
            && row.class != RowClass::Prune
            && key_entity_manifest_keys(&row.kind, &row.key, row.bundle_id.as_deref()).get(1)
                == Some(&entity)
    })
}

fn key_values(key: &str, expected: usize) -> Vec<String> {
    if expected == 1 {
        return vec![key.to_owned()];
    }
    serde_json::from_str::<Vec<String>>(key).unwrap_or_default()
}

/// `key` parsed as a JSON array of exactly two strings, joined
/// `"{first}:{second}"` — the composite-key convention every two-column
/// `row_identity` entry that also has a server retraction cascade shares
/// (`tag_capability_grant`, the `entity_definition_initial_*` satellites,
/// `entity_definition_item_dsl_source`, `entity_definition_equip_modifier`).
/// `None` for a plain string key (a single-column identity, or the
/// already-joined [`row_identity::SET_LEVEL_FIELD`] template id) or a wider
/// array (a kind with no retraction cascade today, so its shape has never
/// been verified against one — deliberately left untransformed rather than
/// guessed).
fn colon_join_two_element_key(key: &str) -> Option<String> {
    let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(key) else {
        return None;
    };
    let [Value::String(first), Value::String(second)] = parts.as_slice() else {
        return None;
    };
    Some(format!("{first}:{second}"))
}

type RowId = (String, String);
/// A row id's resident blocks: one element for an ordinary kind, every block
/// of the set for [`SET_LEVEL_FIELD`].
type ResidentBlocks = BTreeMap<RowId, Vec<Value>>;

/// The resident copies of every row in `wanted`, plus the order the rows
/// first appear in. Ordinarily one row per id — the last one seen in
/// flattening order wins, same as the applyset's own residence rule.
/// [`SET_LEVEL_FIELD`] is different: every block sharing one template id, in
/// EVERY part, is a piece of the same whole set (design D3) and
/// accumulates rather than overwrites — mirrored from
/// `applyset::resolve_split_frame_write_sets` — then sorts by `ordinal` to
/// match the hash `row_identity`'s set-level identity computes.
fn resident_rows(
    sources: &ApplysetSources,
    wanted: &BTreeSet<RowId>,
) -> Result<(ResidentBlocks, Vec<RowId>), String> {
    let mut chosen: ResidentBlocks = BTreeMap::new();
    let mut order: Vec<RowId> = Vec::new();
    for manifest in parsed_parts(sources)? {
        let Value::Object(fields) = serde_json::to_value(&manifest).map_err(|e| e.to_string())?
        else {
            unreachable!("Manifest serializes as an object")
        };
        for (kind, rows) in fields {
            let Value::Array(rows) = rows else { continue };
            for row in rows {
                let Some(key) = row_key_of(&kind, &row) else {
                    continue;
                };
                let id = (kind.clone(), key);
                if !wanted.contains(&id) {
                    continue;
                }
                let first_seen = !chosen.contains_key(&id);
                let blocks = chosen.entry(id.clone()).or_default();
                if kind == SET_LEVEL_FIELD {
                    blocks.push(row);
                } else {
                    *blocks = vec![row];
                }
                if first_seen {
                    order.push(id);
                }
            }
        }
    }
    for (id, blocks) in chosen.iter_mut() {
        if id.0 == SET_LEVEL_FIELD {
            blocks.sort_by_key(|row| row.get("ordinal").and_then(Value::as_u64).unwrap_or(0));
        }
    }
    Ok((chosen, order))
}

/// A manifest holding only the resident copies of the rows in `to_send` — a
/// [`SET_LEVEL_FIELD`] id sends every one of its blocks, not just one.
fn build_delta(
    resident: &mut ResidentBlocks,
    order: &[RowId],
    to_send: &BTreeSet<RowId>,
    retractions: &[RetractionRow],
) -> Result<Manifest, String> {
    let mut value = serde_json::to_value(Manifest::default()).map_err(|e| e.to_string())?;
    let fields = value
        .as_object_mut()
        .expect("Manifest serializes as an object");
    for id in order.iter().filter(|id| to_send.contains(*id)) {
        let blocks = resident.remove(id).expect("resident row");
        fields
            // Optional row arrays can be omitted by Manifest's empty-value
            // serialization. Row identity already admitted this field; the
            // final typed decode rejects an unknown manifest field.
            .entry(id.0.clone())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| format!("row kind {} is not a manifest row array", id.0))?
            .extend(blocks);
    }
    let mut delta: Manifest = serde_json::from_value(value).map_err(|e| e.to_string())?;
    delta.retractions = retractions.to_vec();
    Ok(delta)
}

/// `blocks` as the one value a live/file comparison hashes or renders: for
/// [`SET_LEVEL_FIELD`], every block, ordinal-sorted (independent of caller
/// order, so an unsorted `live_rows` accumulation and a pre-sorted
/// `resident_rows` one compare consistently); the single comparable row
/// otherwise.
fn comparable_value(kind: &str, blocks: &[Value]) -> Value {
    if kind == SET_LEVEL_FIELD {
        let mut blocks: Vec<Value> = blocks.iter().map(|b| comparable(kind, b)).collect();
        blocks.sort_by_key(|row| row.get("ordinal").and_then(Value::as_u64).unwrap_or(0));
        Value::Array(blocks)
    } else {
        comparable(kind, &blocks[0])
    }
}

fn drift_comparable_value(
    kind: &str,
    blocks: &[Value],
    place: Option<&str>,
    applying_identity: Option<&str>,
) -> Value {
    let mut value = comparable_value(kind, blocks);
    if kind == "role_bindings" && value.get("subject").and_then(Value::as_str) == Some("$sender") {
        if let Some(identity) = applying_identity {
            value["subject"] = Value::String(identity.to_owned());
        }
    }
    if matches!(kind, "entities" | "actor_spatial_locations") {
        if let Some(fields) = value.as_object_mut() {
            if let Some(place) =
                place.or_else(|| blocks[0].get("place_node_id").and_then(Value::as_str))
            {
                fields.insert("place_node_id".into(), Value::String(place.into()));
            }
            if kind == "entities" {
                let surrogate = format!(
                    "{}:{}",
                    fields
                        .get("bundle_id")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                    fields
                        .get("entity_key")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                );
                if fields.get("entity_def_id").and_then(Value::as_str) == Some(surrogate.as_str()) {
                    fields.insert("entity_def_id".into(), Value::String(String::new()));
                }
            }
        }
    }
    value
}

pub use spellkave_schema::export_round_trip::EXPORT_NOT_ROUND_TRIPPED;

/// `row` without the fields [`EXPORT_NOT_ROUND_TRIPPED`] names for `kind`.
fn comparable(kind: &str, row: &Value) -> Value {
    let mut row = row.clone();
    if let Value::Object(fields) = &mut row {
        for (k, names) in EXPORT_NOT_ROUND_TRIPPED {
            if *k == "*" || *k == kind {
                for name in *names {
                    fields.remove(*name);
                }
            }
        }
    }
    row
}

/// Identity -> (comparable hash, comparable value) of every identified row in
/// the live exports. [`SET_LEVEL_FIELD`] accumulates every block sharing one
/// template id — even within a single export, several blocks share that id —
/// into one comparable array, ordinal-sorted, the same whole-set treatment
/// [`resident_rows`] gives the file side.
fn live_rows(exports: &[LiveExport]) -> Result<BTreeMap<RowId, (String, Value)>, String> {
    let mut blocks: ResidentBlocks = BTreeMap::new();
    for export in exports {
        let manifest: Manifest = serde_json::from_str(&export.manifest_json)
            .map_err(|err| format!("export of {}: {err}", export.bundle_id))?;
        let Value::Object(fields) = serde_json::to_value(&manifest).map_err(|e| e.to_string())?
        else {
            unreachable!("Manifest serializes as an object")
        };
        for (kind, rows) in fields {
            let Value::Array(rows) = rows else { continue };
            for row in rows {
                let Some(key) = row_key_of(&kind, &row) else {
                    continue;
                };
                let row = comparable(&kind, &row);
                let id = (kind.clone(), key);
                if kind == SET_LEVEL_FIELD {
                    blocks.entry(id).or_default().push(row);
                } else {
                    blocks.insert(id, vec![row]);
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for (id, rows) in blocks {
        let value = comparable_value(&id.0, &rows);
        out.insert(id, (row_hash(&value), value));
    }
    Ok(out)
}

/// Unmasked live export objects (no `comparable` strip). Prune place
/// reconstruction hashes these after writing the live `place_node_id`.
fn unmasked_live_rows(exports: &[LiveExport]) -> Result<BTreeMap<RowId, Value>, String> {
    let mut out = BTreeMap::new();
    for export in exports {
        let manifest: Manifest = serde_json::from_str(&export.manifest_json)
            .map_err(|err| format!("export of {}: {err}", export.bundle_id))?;
        let Value::Object(fields) = serde_json::to_value(&manifest).map_err(|e| e.to_string())?
        else {
            unreachable!("Manifest serializes as an object")
        };
        for (kind, rows) in fields {
            if kind != "entities" && kind != "actor_spatial_locations" {
                continue;
            }
            let Value::Array(rows) = rows else { continue };
            for row in rows {
                let Some(key) = row_key_of(&kind, &row) else {
                    continue;
                };
                out.insert((kind.clone(), key), row);
            }
        }
    }
    Ok(out)
}

/// Reconstruct a pruned `entities` / `actor_spatial_locations` row: unmasked
/// export + live `place_node_id`. A mismatch fails closed (listed, needs
/// `--force`): Same only when a reconstructed hash equals the memory hash.
fn prune_place_live(
    kind: &str,
    memory_hash: &str,
    unmasked: Option<&Value>,
    place: Option<&str>,
) -> LiveState {
    let (Some(export), Some(place)) = (unmasked, place) else {
        return LiveState::NotComparable;
    };
    // Strip export-only snapshot fields, then put back the live place the
    // file hash includes (even "" for a contained entity). comparable()
    // removes `place_node_id` for entities; omitting it would Drift against
    // a file that serialized "".
    let stripped = comparable(kind, export);
    let Some(fields) = stripped.as_object() else {
        return LiveState::NotComparable;
    };
    let mut reconstructed = Value::Object(fields.clone());
    reconstructed
        .as_object_mut()
        .expect("cloned object")
        .insert("place_node_id".to_owned(), Value::String(place.to_owned()));
    if row_hash(&reconstructed) == memory_hash {
        return LiveState::Same;
    }
    // entities only: the export writes `entity_def_id` as the surrogate
    // `manifest_key(bundle_id, entity_key)` when the file had "". Hash that
    // variant with "" as well. No other normalization.
    if kind == "entities" {
        if let Some(obj) = reconstructed.as_object_mut() {
            let bundle = obj
                .get("bundle_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let entity_key = obj
                .get("entity_key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let surrogate = format!("{bundle}:{entity_key}");
            if obj.get("entity_def_id").and_then(Value::as_str) == Some(surrogate.as_str()) {
                obj.insert("entity_def_id".to_owned(), Value::String(String::new()));
                if row_hash(&reconstructed) == memory_hash {
                    return LiveState::Same;
                }
            }
        }
    }
    LiveState::Drift
}

fn pretty(row: &Value) -> String {
    // Re-parse the canonical form so keys come out sorted whatever map type
    // serde_json was built with.
    let canonical: Value =
        serde_json::from_str(&canonical_json(row)).expect("canonical JSON parses");
    serde_json::to_string_pretty(&canonical).expect("JSON value renders")
}

fn parsed_parts(sources: &ApplysetSources) -> Result<Vec<Manifest>, String> {
    let mut out = vec![
        spellkave_format::parse_manifest(&sources.root.toml)
            .map_err(|err| format!("{}: {err}", sources.root.path))?
            .manifest,
    ];
    for fragment in &sources.fragments {
        out.push(
            spellkave_format::parse_manifest_fragment(&fragment.toml)
                .map_err(|err| format!("{}: {err}", fragment.path))?,
        );
    }
    Ok(out)
}

fn validate_built_root(
    sources: &ApplysetSources,
    dependencies: &[ApplysetSources],
    retractions: &[RetractionRow],
) -> Result<Vec<Diagnostic>, String> {
    let mut parts = parsed_parts(sources)?;
    let mut root = parts.remove(0);
    let mut rest = parts;
    for dependency in dependencies {
        rest.extend(parsed_parts(dependency)?);
    }
    spellkave_format::append_fragment_rows(&mut root, rest);
    root.retractions = retractions.to_vec();
    let mut diags: Vec<Diagnostic> = spellkave_validate::validate_manifest(&root)
        .into_iter()
        .filter(|d| d.severity == Severity::Fatal)
        .map(|d| name_retraction(d, retractions))
        .collect();
    spellkave_validate::canonicalize_diagnostics(&mut diags);
    Ok(diags)
}

/// A diagnostic on `retractions[i]` names only the index; name the row too.
fn name_retraction(mut diag: Diagnostic, retractions: &[RetractionRow]) -> Diagnostic {
    let index = diag
        .path
        .strip_prefix("retractions[")
        .and_then(|rest| rest.split(']').next())
        .and_then(|n| n.parse::<usize>().ok());
    if let Some(row) = index.and_then(|i| retractions.get(i)) {
        diag.message = format!(
            "retraction of {} `{}`: {}",
            row.row_kind, row.authored_key, diag.message
        );
        let mut details = match diag.details.take() {
            Some(Value::Object(map)) => map,
            Some(other) => {
                let mut map = serde_json::Map::new();
                map.insert("validator".into(), other);
                map
            }
            None => serde_json::Map::new(),
        };
        details.insert(
            "retraction".into(),
            serde_json::json!({ "row_kind": row.row_kind, "authored_key": row.authored_key }),
        );
        diag.details = Some(Value::Object(details));
    }
    diag
}

fn warning(code: &str, message: &str, details: Option<Value>) -> Warning {
    Warning {
        code: code.to_owned(),
        message: message.to_owned(),
        details,
    }
}

fn fatal(code: &str, message: &str, details: Option<Value>) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        message: message.to_owned(),
        path: String::new(),
        span: None,
        hint: None,
        details,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = \"1.0.0\"\napplyset = \"demo\"\n";
    const BUNDLE: &str = "[[bundles]]\nbundle_id = \"demo\"\ndisplay_name = \"Demo\"\nversion = \"1.0.0\"\nauthor = \"t\"\ndescription = \"d\"\n";

    fn def(id: &str, desc: &str) -> String {
        format!(
            "[[entity_definitions]]\nbundle_id = \"demo\"\nentity_def_id = \"{id}\"\ndisplay_name = \"X\"\ndescription = \"{desc}\"\n"
        )
    }

    fn tag(id: &str, tag: &str) -> String {
        format!(
            "[[entity_definition_tag]]\nentity_def_id = \"{id}\"\ntag = \"{tag}\"\nbundle_id = \"demo\"\nsource = \"manifest\"\n"
        )
    }

    fn entity(key: &str, kind: &str) -> String {
        format!(
            "[[entities]]\nentity_key = \"{key}\"\nbundle_id = \"demo\"\nname = \"{key}\"\nkind = \"{kind}\"\nplace_node_id = \"\"\n"
        )
    }

    fn containment(container: &str, item: &str) -> String {
        format!(
            "[[entity_containment]]\ncontainer_entity_key = \"{container}\"\nentity_key = \"{item}\"\n"
        )
    }

    fn spatial_state(id: &str, desc: &str) -> String {
        format!(
            "[[spatial_state_definitions]]\nstate_id = \"{id}\"\nbundle_id = \"demo\"\nvalue_type = \"bool\"\nwriter_policy = \"any\"\nfreshness_rule = \"none\"\nmerge_rule = \"replace\"\nlod_policy = \"none\"\ndescription = \"{desc}\"\n"
        )
    }

    const GRAPH: &str = "[[outcome_graphs]]\noutcome_graph_id = \"demo/g\"\nbundle_id = \"demo\"\ndisplay_name = \"G\"\nroot_node_id = \"start\"\n";

    fn node(id: &str, node_id: &str) -> String {
        format!(
            "[[outcome_graph_nodes]]\noutcome_graph_node_id = \"{id}\"\noutcome_graph_id = \"demo/g\"\nnode_id = \"{node_id}\"\nterminal = true\n"
        )
    }

    /// Root with the given includes (path, rows) and extra root rows.
    fn input(root_rows: &str, includes: &[(&str, String)]) -> DiffInput {
        let mut root = format!("{HEADER}{BUNDLE}{root_rows}");
        let mut files = BTreeMap::new();
        for (path, rows) in includes {
            root.push_str(&format!("\n[[include]]\npath = \"{path}\"\n"));
            files.insert((*path).to_owned(), rows.clone());
        }
        DiffInput {
            applying_identity: None,
            force: false,
            not_exported: Vec::new(),
            places_read: true,
            root: PartSource {
                path: "manifest.toml".into(),
                toml: root,
            },
            root_path: "fixtures/demo/manifest.toml".into(),
            files,
            memory: None,
            prune: false,
            dependencies: Vec::new(),
            live: Vec::new(),
            live_places: Vec::new(),
            extent_scope: None,
            authority: None,
        }
    }

    /// The memory an apply of `input` would leave behind.
    fn memory_after(input: &DiffInput) -> Memory {
        let out = classify_and_build_delta(input).unwrap();
        Memory {
            export_hashes: out
                .parts
                .iter()
                .flat_map(|p| &p.entries)
                .filter_map(|e| {
                    e.export_hash.as_ref().map(|h| ExportHash {
                        kind: e.kind.clone(),
                        key: e.key.clone(),
                        export_hash: h.clone(),
                    })
                })
                .collect(),
            root_path: out.root_path,
            generation: 1,
            base_state: BaseState::Known,
            parts: out
                .parts
                .into_iter()
                .filter(|p| !p.entries.is_empty())
                .map(|p| MemoryPart {
                    path: p.path,
                    entries: p.entries,
                })
                .collect(),
        }
    }

    fn classes(out: &DiffOutput) -> Vec<(RowClass, &str, &str)> {
        out.rows
            .iter()
            .map(|r| (r.class, r.kind.as_str(), r.key.as_str()))
            .collect()
    }

    fn base() -> DiffInput {
        input(
            &def("demo/a", "a"),
            &[
                ("b.toml", def("demo/b", "b")),
                ("c.toml", def("demo/c", "c")),
            ],
        )
    }

    // K4: an omitted optional manifest array must survive the applyset delta wire.
    #[test]
    fn first_condition_extent_rule_builds_a_typed_delta_when_default_omits_its_array() {
        let source = "[[condition_capability_extent_grants]]\nbundle_id=\"demo\"\ncondition_definition_id=\"demo/bond\"\ncapability_id=\"demo/reach\"\nextent_kind_ref=\"party\"\nextent_ref=\"demo.members\"\n";
        let next = input(source, &[]);
        let out = classify_and_build_delta(&next).unwrap();
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(delta.condition_capability_extent_grants.len(), 1);
        assert_eq!(
            delta.condition_capability_extent_grants[0].extent_ref,
            "demo.members"
        );
    }

    fn codes(out: &DiffOutput) -> Vec<&str> {
        out.diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn comparable_normalizes_export_default_graph_links_and_sender_without_masking_subject() {
        let graph = serde_json::json!({"parent_graph_id":"region", "anchor_node_id":"room", "display_name":"Graph"});
        assert_eq!(
            drift_comparable_value("place_graphs", &[graph], None, None),
            serde_json::json!({"display_name":"Graph"})
        );
        let binding = serde_json::json!({"subject":"$sender", "role_id":"author"});
        let value = drift_comparable_value("role_bindings", &[binding], None, Some("principal"));
        assert_eq!(value["subject"], "principal");
        assert_ne!(
            value,
            drift_comparable_value(
                "role_bindings",
                &[serde_json::json!({"subject":"someone-else", "role_id":"author"})],
                None,
                Some("principal")
            )
        );
    }
    #[test]
    fn comparable_containment_overrides_an_entities_declared_place() {
        let row = serde_json::json!({"entity_key":"item", "bundle_id":"demo", "place_node_id":"room", "entity_def_id":"demo:item"});
        let value = drift_comparable_value("entities", &[row], Some(""), None);
        assert_eq!(value["place_node_id"], "");
        assert_eq!(value["entity_def_id"], "");
    }
    #[test]
    fn seeded_rows_adopt_and_changed_rows_require_force() {
        let old = input(&def("demo/a", "old"), &[]);
        let mut next = input(&def("demo/a", "file"), &[]);
        let export = spellkave_format::parse_manifest(&old.root.toml)
            .unwrap()
            .manifest;
        next.live = vec![LiveExport {
            bundle_id: "demo".into(),
            manifest_json: serde_json::to_string(&export).unwrap(),
        }];
        let out = classify_and_build_delta(&next).unwrap();
        let value = serde_json::to_value(&out).unwrap();
        assert_eq!(
            value["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["key"] == "demo/a")
                .unwrap()["class"],
            "adopt"
        );
        assert_eq!(value["conflicts"].as_array().unwrap().len(), 1);
        assert!(!out.delta_toml.contains("demo/a"));
    }

    #[test]
    fn unchecked_seal_upsert_requires_an_explicit_drift_decision() {
        let rows = "[[place_graphs]]\nplace_graph_id = \"demo/g\"\nbundle_id = \"demo\"\nsubstrate_id = \"indoor\"\ndisplay_name = \"G\"\n[[place_seals]]\nplace_graph_id = \"demo/g\"\nbundle_id = \"demo\"\n";
        let mut next = input(rows, &[]);
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(out.seal_drift_not_checked, vec!["demo/g"]);
        next.force = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(out.seal_drift_not_checked, vec!["demo/g"]);
        assert!(out.delta_toml.contains("[[place_seals]]"));
        next.live = vec![export(rows)];
        assert!(classify_and_build_delta(&next)
            .unwrap()
            .seal_drift_not_checked
            .is_empty());
    }

    fn drift_input(before: &str, file: &str, live: &str) -> DiffInput {
        let first = input(before, &[]);
        let mut next = input(file, &[]);
        next.memory = Some(memory_after(&first));
        next.live = vec![export(live)];
        next
    }

    #[test]
    fn changed_state_conflicts_against_recorded_world_value_and_force_records_file_hash() {
        let state = object_state("demo/hero", "demo/charges");
        let mut next = drift_input(
            &state,
            &state.replace("value_int = 1", "value_int = 3"),
            &state.replace("value_int = 1", "value_int = 2"),
        );
        let out = classify_and_build_delta(&next).unwrap();
        let row = out.rows.iter().find(|r| r.kind == "object_state").unwrap();
        assert_eq!(row.world_changed, Some(WorldChanged::Yes));
        assert_eq!(row.class, RowClass::Configure);
        assert_eq!(out.conflicts.len(), 1);
        assert!(!out.delta_toml.contains("[[object_state]]"));
        next.force = true;
        let forced = classify_and_build_delta(&next).unwrap();
        assert!(forced.delta_toml.contains("value_int = 3"));
        let entry = forced
            .parts
            .iter()
            .flat_map(|p| &p.entries)
            .find(|e| e.kind == "object_state")
            .unwrap();
        assert_eq!(entry.export_hash.as_ref().unwrap().len(), 64);
        assert_ne!(
            entry.export_hash.as_ref().unwrap(),
            &next
                .memory
                .as_ref()
                .unwrap()
                .export_hashes
                .iter()
                .find(|e| e.kind == "object_state")
                .unwrap()
                .export_hash
        );
    }

    #[test]
    fn moved_live_entity_conflicts_even_when_export_omits_place() {
        let before = entity_at("demo/hero", "npc", "demo/start");
        let mut next = drift_input(
            &before,
            &before.replace("name = \"demo/hero\"", "name = \"Renamed\""),
            &entity_at("demo/hero", "npc", ""),
        );
        next.live_places = vec![place_of("entities", "demo/hero", "demo/end")];
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(out.conflicts.len(), 1);
        assert!(out
            .values
            .iter()
            .any(|v| v.live.contains("demo/end") && v.file.contains("demo/start")));
        assert!(!out.delta_toml.contains("[[entities]]"));
    }

    #[test]
    fn absent_row_of_exported_bundle_is_a_conflict_with_an_absent_value() {
        let mut next = drift_input(&def("demo/a", "before"), &def("demo/a", "file"), "");
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(out.conflicts.len(), 1);
        assert_eq!(out.values[0].live, "(absent in the world)");
        next.force = true;
        assert!(classify_and_build_delta(&next)
            .unwrap()
            .delta_toml
            .contains("demo/a"));
    }

    #[test]
    fn unchanged_drifting_rows_are_information_and_never_conflicts_or_writes() {
        let next = drift_input(
            &def("demo/a", "before"),
            &def("demo/a", "before"),
            &def("demo/a", "played"),
        );
        let out = classify_and_build_delta(&next).unwrap();
        let row = out.rows.iter().find(|r| r.key == "demo/a").unwrap();
        assert_eq!(
            (row.class, row.live, row.world_changed.clone()),
            (RowClass::Unchanged, Some(LiveState::Drift), None)
        );
        assert!(out.conflicts.is_empty());
        assert!(!out.send);
        assert!(out.values.iter().any(|v| v.live.contains("played")));
    }

    #[test]
    fn seeded_equal_rows_adopt_record_comparable_hashes_and_leave_live_only_rows_alone() {
        let mut next = input(&def("demo/a", "before"), &[]);
        next.live = vec![export(&format!(
            "{BUNDLE}{}{}",
            def("demo/a", "before"),
            def("demo/live-only", "keep")
        ))];
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.rows.iter().all(|r| r.class == RowClass::Adopt));
        assert!(out.conflicts.is_empty());
        assert!(!out.delta_toml.contains("[[entity_definitions]]"));
        assert!(out.send, "listing memory is a metadata apply");
        assert!(out
            .parts
            .iter()
            .flat_map(|p| &p.entries)
            .all(|e| e.export_hash.as_ref().is_some_and(|h| h.len() == 64)));
        assert!(out.live_only.iter().any(|r| r.key == "demo/live-only"));
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == "MF-APPLYSET-ADOPT" && w.message.contains("adopt")));
    }

    #[test]
    fn missing_hash_and_unexported_bundle_are_explicitly_not_checked() {
        let mut next = drift_input(
            &def("demo/a", "before"),
            &def("demo/a", "file"),
            &def("demo/a", "played"),
        );
        next.memory.as_mut().unwrap().export_hashes.clear();
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.conflicts.is_empty());
        assert!(out
            .not_checked
            .iter()
            .any(|r| r.key == "demo/a" && r.reason.contains("no comparable hash recorded")));
        assert!(out.delta_toml.contains("demo/a"));
        next.live.clear();
        next.not_exported.push(NotExported {
            bundle_id: "demo".into(),
            reason: "publisher-only".into(),
        });
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out
            .not_checked
            .iter()
            .any(|r| r.key == "demo/a" && r.reason == "publisher-only"));
        assert!(out.conflicts.is_empty());
    }

    #[test]
    fn unread_placement_and_uncovered_kind_are_not_checked() {
        let before = entity_at("demo/hero", "npc", "demo/start");
        let mut next = drift_input(
            &before,
            &before.replace("name = \"demo/hero\"", "name = \"Renamed\""),
            &before,
        );
        next.places_read = false;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out
            .not_checked
            .iter()
            .any(|r| r.kind == "entities" && r.reason == "live place not read"));
        assert!(out.conflicts.is_empty());
        let condition = "[[condition_definitions]]\ncondition_definition_id = \"demo/condition\"\nbundle_id = \"demo\"\ndisplay_name = \"Before\"\ngate_predicate_source = \"true\"\n";
        let next = drift_input(condition, &condition.replace("Before", "File"), condition);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.not_checked.iter().any(
            |r| r.kind == "condition_definitions" && r.reason.contains("does not reconstruct")
        ));
        assert!(out.conflicts.is_empty());
    }

    #[test]
    fn role_retractions_preserve_owned_tuple_identity() {
        let aggregation = retraction_for(
            "role_aggregations",
            r#"["house/gm","weather"]"#,
            Some("weather"),
        );
        assert_eq!(aggregation.row_kind, "role_aggregation");
        assert_eq!(aggregation.authored_key, r#"["house/gm","weather"]"#);
        let definition = retraction_for("role_definitions", "house/gm", Some("house"));
        assert_eq!(definition.row_kind, "role_definition");
        assert_eq!(definition.authored_key, "house/gm");
    }

    #[test]
    fn a_new_applyset_creates_every_row_and_warns_about_adoption() {
        let out = classify_and_build_delta(&base()).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        assert_eq!(out.generation, 0);
        assert!(out.rows.iter().all(|r| r.class == RowClass::Create));
        assert!(out.warnings.iter().any(|w| w.code == "MF-APPLYSET-ADOPT"));
        assert_eq!(out.parts.len(), 3, "root, b.toml, c.toml");
        assert!(out.send);
        assert!(out.delta_toml.contains("demo/b"));
    }

    #[test]
    fn nothing_changed_is_unchanged_and_sends_nothing() {
        let mut next = base();
        next.memory = Some(memory_after(&base()));
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.rows.iter().all(|r| r.class == RowClass::Unchanged));
        assert!(out.parts.is_empty());
        assert!(!out.send);
        assert_eq!(out.generation, 1);
    }

    #[test]
    fn publisher_lists_legacy_ownership_once_without_rewriting_unchanged_rows() {
        use crate::diff_apply_extent::{ExtentMemory, ExtentScope};
        let mut next = base();
        next.memory = Some(memory_after(&base()));
        next.extent_scope = Some(ExtentScope {
            publisher: true,
            bundle_extents: vec![],
            memory: vec![ExtentMemory {
                applyset: "demo".into(),
                bundle_ids: None,
                entries: vec![],
            }],
        });
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.rows.iter().all(|r| r.class == RowClass::Unchanged));
        assert!(out.parts.is_empty());
        assert!(
            out.send,
            "ownership qualification is a real metadata-only apply"
        );
        next.extent_scope.as_mut().unwrap().memory[0].bundle_ids = Some(vec!["demo".into()]);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(!out.send, "listed reseeding is a true no-op");

        let scope = next.extent_scope.as_mut().unwrap();
        scope.publisher = false;
        scope.bundle_extents = vec!["demo".into()];
        scope.memory[0].bundle_ids = None;
        assert!(
            !classify_and_build_delta(&next).unwrap().send,
            "non-publisher no-op must not attempt a legacy ownership claim"
        );
    }

    #[test]
    fn empty_bundle_root_reseeding_does_not_force_an_ownership_claim() {
        use crate::diff_apply_extent::{ExtentMemory, ExtentScope};
        let mut next = input("", &[]);
        next.root.toml = HEADER.into();
        next.memory = Some(memory_after(&next));
        next.extent_scope = Some(ExtentScope {
            publisher: true,
            bundle_extents: vec![],
            memory: vec![ExtentMemory {
                applyset: "demo".into(),
                bundle_ids: None,
                entries: vec![],
            }],
        });
        assert!(!classify_and_build_delta(&next).unwrap().send);
        next.extent_scope.as_mut().unwrap().memory[0].bundle_ids = Some(vec![]);
        assert!(!classify_and_build_delta(&next).unwrap().send);
    }

    #[test]
    fn comments_and_block_order_do_not_create_differences() {
        let memory = memory_after(&base());
        let mut next = input(
            &format!("# a comment\n{}", def("demo/a", "a")),
            &[
                ("b.toml", format!("# note\n\n{}", def("demo/b", "b"))),
                ("c.toml", def("demo/c", "c")),
            ],
        );
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(!out.send, "{:?}", classes(&out));
    }

    #[test]
    fn a_changed_row_is_configure_and_only_it_is_sent() {
        let memory = memory_after(&base());
        let mut next = input(
            &def("demo/a", "a"),
            &[
                ("b.toml", def("demo/b", "B!")),
                ("c.toml", def("demo/c", "c")),
            ],
        );
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        let changed: Vec<_> = out
            .rows
            .iter()
            .filter(|r| r.class != RowClass::Unchanged)
            .collect();
        assert_eq!(changed.len(), 1);
        assert_eq!(
            (changed[0].class, changed[0].key.as_str()),
            (RowClass::Configure, "demo/b")
        );
        assert_eq!(out.parts.len(), 1);
        assert_eq!(out.parts[0].path, "b.toml");
        assert!(out.delta_toml.contains("B!"));
        assert!(!out.delta_toml.contains("demo/c"));
        // The delta parses and holds exactly the one row.
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(delta.entity_definitions.len(), 1);
        assert!(delta.bundles.is_empty());
    }

    #[test]
    fn the_sent_row_hashes_like_the_file_row() {
        // The server checks every part entry against the delta AS SENT.
        let memory = memory_after(&base());
        let mut next = input(
            &def("demo/a", "a"),
            &[
                ("b.toml", def("demo/b", "B!")),
                ("c.toml", def("demo/c", "c")),
            ],
        );
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        let sent = spellkave_schema::row_identity::manifest_row_identities(&delta).rows;
        let entry = out.parts[0]
            .entries
            .iter()
            .find(|e| e.key == "demo/b")
            .unwrap();
        assert_eq!(sent[0].hash, entry.hash);
    }

    #[test]
    fn a_moved_row_is_move_and_sends_an_empty_delta_with_both_parts() {
        let memory = memory_after(&base());
        let mut next = input(
            &def("demo/a", "a"),
            &[
                ("b.toml", String::new()),
                (
                    "c.toml",
                    format!("{}{}", def("demo/c", "c"), def("demo/b", "b")),
                ),
            ],
        );
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        let moved: Vec<_> = out
            .rows
            .iter()
            .filter(|r| r.class == RowClass::Move)
            .collect();
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].from_part.as_deref(), Some("b.toml"));
        assert_eq!(moved[0].part, "c.toml");
        let paths: Vec<_> = out.parts.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(paths, vec!["b.toml", "c.toml"]);
        assert!(out.parts[0].entries.is_empty(), "b.toml is emptied");
        assert!(out.send);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert!(
            spellkave_schema::row_identity::manifest_row_identities(&delta)
                .rows
                .is_empty()
        );
    }

    #[test]
    fn a_moved_and_changed_row_is_configure_with_its_old_part() {
        let memory = memory_after(&base());
        let mut next = input(
            &def("demo/a", "a"),
            &[
                ("b.toml", String::new()),
                (
                    "c.toml",
                    format!("{}{}", def("demo/c", "c"), def("demo/b", "new")),
                ),
            ],
        );
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        let row = out.rows.iter().find(|r| r.key == "demo/b").unwrap();
        assert_eq!(row.class, RowClass::Configure);
        assert_eq!(row.from_part.as_deref(), Some("b.toml"));
        assert!(out.delta_toml.contains("new"));
    }

    #[test]
    fn a_removed_include_lists_prune_candidates_and_keeps_them_recorded() {
        let memory = memory_after(&base());
        let mut next = input(&def("demo/a", "a"), &[("b.toml", def("demo/b", "b"))]);
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        let pruned: Vec<_> = out
            .rows
            .iter()
            .filter(|r| r.class == RowClass::Prune)
            .collect();
        assert_eq!(pruned.len(), 1);
        assert_eq!(
            (pruned[0].key.as_str(), pruned[0].part.as_str()),
            ("demo/c", "c.toml")
        );
        // Without --prune nothing is retracted and no part changes.
        assert!(!out.delta_toml.contains("retractions"));
        assert!(out.parts.is_empty());
        assert!(!out.send);
    }

    #[test]
    fn prune_retracts_and_drops_the_part() {
        let memory = memory_after(&base());
        let mut next = input(&def("demo/a", "a"), &[("b.toml", def("demo/b", "b"))]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(
            delta.retractions,
            vec![RetractionRow {
                row_kind: "entity_definition".into(),
                authored_key: "demo/c".into(),
            }]
        );
        assert_eq!(out.parts.len(), 1);
        assert_eq!(out.parts[0].path, "c.toml");
        assert!(out.parts[0].entries.is_empty());
    }

    #[test]
    fn an_unsupported_retraction_comes_back_as_a_validator_diagnostic() {
        let with_tag = input(
            &def("demo/a", "a"),
            &[("b.toml", tag("demo/a", "demo.lamp"))],
        );
        let memory = memory_after(&with_tag);
        let mut next = input(&def("demo/a", "a"), &[("b.toml", String::new())]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(
            codes(&out),
            vec![spellkave_validate::MF_VAL_RETRACTION_ROW_KIND],
            "{:?}",
            out.diagnostics
        );
        assert!(out.diagnostics[0].message.contains("entity_definition_tag"));
        // manifest-natural-keys 1.1: the tag kinds' key widened to the
        // server's real key `(anchor, tag, bundle_id, source)`.
        assert!(
            out.diagnostics[0].message.starts_with(
                r#"retraction of entity_definition_tag `["demo/a","demo.lamp","demo","manifest"]`"#
            ),
            "{}",
            out.diagnostics[0].message
        );
    }

    #[test]
    fn pruning_an_entity_covers_its_containment_and_reports_both_rows() {
        let first = input(
            &format!(
                "{}{}",
                entity("demo/chest", "item"),
                entity("demo/torch", "item")
            ),
            &[("items.toml", containment("demo/chest", "demo/torch"))],
        );
        let memory = memory_after(&first);
        let mut next = input(
            &entity("demo/chest", "item"),
            &[("items.toml", String::new())],
        );
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(
            delta.retractions,
            vec![RetractionRow {
                row_kind: "entity".into(),
                authored_key: "demo:demo/torch".into(),
            }]
        );
        assert_eq!(
            out.rows
                .iter()
                .filter(|row| row.class == RowClass::Prune)
                .count(),
            2
        );
    }

    #[test]
    fn a_moved_containment_needs_no_retraction() {
        let first = input(
            &format!(
                "{}{}{}",
                entity("demo/a", "item"),
                entity("demo/b", "item"),
                entity("demo/torch", "item")
            ),
            &[("items.toml", containment("demo/a", "demo/torch"))],
        );
        let memory = memory_after(&first);
        let mut next = input(
            &format!(
                "{}{}{}",
                entity("demo/a", "item"),
                entity("demo/b", "item"),
                entity("demo/torch", "item")
            ),
            &[("items.toml", containment("demo/b", "demo/torch"))],
        );
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert!(delta.retractions.is_empty());
        assert_eq!(
            out.rows
                .iter()
                .filter(|row| row.class == RowClass::Prune)
                .count(),
            1
        );
    }

    #[test]
    fn a_containment_prune_without_its_entity_has_a_builder_diagnostic() {
        let first = input(
            &format!(
                "{}{}",
                entity("demo/chest", "item"),
                entity("demo/torch", "item")
            ),
            &[("items.toml", containment("demo/chest", "demo/torch"))],
        );
        let memory = memory_after(&first);
        let mut next = input(
            &format!(
                "{}{}",
                entity("demo/chest", "item"),
                entity("demo/torch", "item")
            ),
            &[("items.toml", String::new())],
        );
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        let message = out
            .diagnostics
            .iter()
            .find(|diag| {
                diag.message
                    .contains("can be removed only together with its entity")
            })
            .map(|diag| diag.message.as_str())
            .unwrap_or("");
        assert!(
            message.contains("`demo/torch`"),
            "containment prune names the contained entity, got {message}"
        );
        assert!(
            !message.contains("`demo/chest`"),
            "containment prune must not name the container, got {message}"
        );
    }

    fn sent_has(out: &DiffOutput, kind: &str, key: &str) -> bool {
        out.parts.iter().any(|part| {
            part.entries
                .iter()
                .any(|entry| entry.kind == kind && entry.key == key)
        })
    }

    fn object_state(entity: &str, state: &str) -> String {
        format!(
            "[[object_state]]\nbundle_id = \"demo\"\nentity_key = \"{entity}\"\nstate_id = \"{state}\"\nvalue_int = 1\nsource_template_id = \"manifest\"\n"
        )
    }

    fn entity_tag(entity: &str, tag: &str) -> String {
        format!(
            "[[entity_tag]]\nbundle_id = \"demo\"\nentity_key = \"{entity}\"\ntag = \"{tag}\"\nsource = \"manifest\"\n"
        )
    }

    fn resource_bound(entity: &str, resource: &str) -> String {
        format!(
            "[[entity_resource_bound]]\nbundle_id = \"demo\"\nentity_key = \"{entity}\"\nresource_definition_id = \"{resource}\"\nmin_value = 0\nmax_value = 5\n"
        )
    }

    fn capability_grant(entity: &str) -> String {
        format!(
            "[[actor_capabilities]]\nactor_entity_key = \"{entity}\"\ncapability_id = \"demo/cap\"\nsource = \"manifest\"\nbundle_id = \"demo\"\n"
        )
    }

    fn actor_body(entity: &str) -> String {
        format!(
            "[[actors]]\nbundle_id = \"demo\"\nentity_key = \"{entity}\"\nname = \"{entity}\"\n[[actor_spatial_locations]]\nbundle_id = \"demo\"\nentity_key = \"{entity}\"\nplace_node_id = \"\"\n[[actor_identities]]\nbundle_id = \"demo\"\nentity_key = \"{entity}\"\nidentity = \"$sender\"\nrole = \"primary\"\n"
        )
    }

    fn equipment_slot(wearer: &str, item: &str) -> String {
        format!(
            "[[actor_equipment_slots]]\nactor_entity_key = \"{wearer}\"\nslot_id = \"demo/hand\"\nitem_key = \"{item}\"\n"
        )
    }

    #[test]
    fn pruning_a_container_and_its_content_retracts_each_entity_once() {
        let first = input(
            &format!(
                "{}{}",
                entity("demo/chest", "item"),
                entity("demo/torch", "item")
            ),
            &[("items.toml", containment("demo/chest", "demo/torch"))],
        );
        let memory = memory_after(&first);
        let mut next = input("", &[("items.toml", String::new())]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        let mut keys: Vec<_> = delta
            .retractions
            .iter()
            .map(|row| (row.row_kind.as_str(), row.authored_key.as_str()))
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![("entity", "demo:demo/chest"), ("entity", "demo:demo/torch")]
        );
        assert!(!sent_has(&out, "entities", "demo/chest"));
        assert!(!sent_has(&out, "entities", "demo/torch"));
        assert!(!out.parts.iter().any(|part| part
            .entries
            .iter()
            .any(|entry| entry.kind == "entity_containment")));
    }

    #[test]
    fn pruning_an_entity_covers_state_tag_capability_and_resource_bound() {
        let extras = format!(
            "{}{}{}{}",
            object_state("demo/hero", "demo/charges"),
            entity_tag("demo/hero", "demo.probe"),
            capability_grant("demo/hero"),
            resource_bound("demo/hero", "demo/charges")
        );
        let first = input(&entity("demo/hero", "npc"), &[("extra.toml", extras)]);
        let memory = memory_after(&first);
        let mut next = input("", &[("extra.toml", String::new())]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(
            delta.retractions,
            vec![RetractionRow {
                row_kind: "entity".into(),
                authored_key: "demo:demo/hero".into(),
            }]
        );
        for kind in [
            "object_state",
            "entity_tag",
            "actor_capabilities",
            "entity_resource_bound",
            "entities",
        ] {
            assert!(
                !out.parts
                    .iter()
                    .any(|part| { part.entries.iter().any(|entry| entry.kind == kind) }),
                "{kind} still in sent parts"
            );
        }
    }

    #[test]
    fn pruning_an_actor_covers_its_position_and_identity() {
        let first = input(
            &entity("demo/hero", "npc"),
            &[("actor.toml", actor_body("demo/hero"))],
        );
        let memory = memory_after(&first);
        let mut next = input("", &[("actor.toml", String::new())]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(
            delta.retractions,
            vec![RetractionRow {
                row_kind: "entity".into(),
                authored_key: "demo:demo/hero".into(),
            }]
        );
        for kind in [
            "actors",
            "actor_spatial_locations",
            "actor_identities",
            "entities",
        ] {
            assert!(
                !out.parts
                    .iter()
                    .any(|part| { part.entries.iter().any(|entry| entry.kind == kind) }),
                "{kind} still in sent parts"
            );
        }
    }

    #[test]
    fn pruning_a_wearer_covers_its_equipment_slots() {
        let first = input(
            &format!(
                "{}{}",
                entity("demo/hero", "npc"),
                entity("demo/sword", "item")
            ),
            &[("gear.toml", equipment_slot("demo/hero", "demo/sword"))],
        );
        let memory = memory_after(&first);
        let mut next = input(
            &entity("demo/sword", "item"),
            &[("gear.toml", String::new())],
        );
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(
            delta.retractions,
            vec![RetractionRow {
                row_kind: "entity".into(),
                authored_key: "demo:demo/hero".into(),
            }]
        );
        assert!(!out.parts.iter().any(|part| part
            .entries
            .iter()
            .any(|entry| entry.kind == "actor_equipment_slots")));
    }

    #[test]
    fn a_moved_containment_later_prune_is_not_refused() {
        let bodies = format!(
            "{}{}{}",
            entity("demo/a", "item"),
            entity("demo/b", "item"),
            entity("demo/torch", "item")
        );
        let first = input(
            &bodies,
            &[("items.toml", containment("demo/a", "demo/torch"))],
        );
        let memory = memory_after(&first);
        let mut moved = input(
            &bodies,
            &[("items.toml", containment("demo/b", "demo/torch"))],
        );
        moved.memory = Some(memory.clone());
        moved.prune = true;
        let out = classify_and_build_delta(&moved).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        assert_eq!(
            out.rows
                .iter()
                .filter(|row| row.class == RowClass::Create)
                .count(),
            1
        );
        assert_eq!(
            out.rows
                .iter()
                .filter(|row| row.class == RowClass::Prune)
                .count(),
            1
        );
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert!(delta.retractions.is_empty());
        let mut parts = memory.parts;
        for part in out.parts {
            if let Some(slot) = parts.iter_mut().find(|p| p.path == part.path) {
                slot.entries = part.entries;
            }
        }
        let mut later = input(
            &bodies,
            &[("items.toml", containment("demo/b", "demo/torch"))],
        );
        later.memory = Some(Memory {
            export_hashes: Vec::new(),
            root_path: memory.root_path,
            generation: memory.generation,
            base_state: memory.base_state,
            parts,
        });
        later.prune = true;
        let later_out = classify_and_build_delta(&later).unwrap();
        assert!(codes(&later_out).is_empty(), "{:?}", later_out.diagnostics);
    }

    #[test]
    fn a_tag_prune_without_its_entity_has_a_builder_diagnostic() {
        let first = input(
            &entity("demo/hero", "npc"),
            &[("tags.toml", entity_tag("demo/hero", "demo.probe"))],
        );
        let memory = memory_after(&first);
        let mut next = input(&entity("demo/hero", "npc"), &[("tags.toml", String::new())]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.diagnostics.iter().any(|diag| diag
            .message
            .contains("can be removed only together with its entity `demo/hero`")));
    }

    #[test]
    fn a_state_prune_without_its_entity_has_a_builder_diagnostic() {
        let first = input(
            &entity("demo/hero", "npc"),
            &[("state.toml", object_state("demo/hero", "demo/charges"))],
        );
        let memory = memory_after(&first);
        let mut next = input(
            &entity("demo/hero", "npc"),
            &[("state.toml", String::new())],
        );
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.diagnostics.iter().any(|diag| diag
            .message
            .contains("can be removed only together with its entity `demo/hero`")));
    }

    #[test]
    fn a_slot_prune_of_a_removed_item_while_the_wearer_stays_is_refused() {
        let first = input(
            &format!(
                "{}{}",
                entity("demo/hero", "npc"),
                entity("demo/sword", "item")
            ),
            &[("gear.toml", equipment_slot("demo/hero", "demo/sword"))],
        );
        let memory = memory_after(&first);
        let mut next = input(&entity("demo/hero", "npc"), &[("gear.toml", String::new())]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out.diagnostics.iter().any(|diag| diag
            .message
            .contains("can be removed only together with its entity `demo/hero`")));
    }

    #[test]
    fn a_changed_actor_capability_is_sent() {
        // manifest-natural-keys task 3.3: the server upserts actor capabilities
        // on the same 4-column key as their row identity, so a changed grant
        // is sent as a configure instead of refused (MF-APPLY-021).
        let grant = |visibility: &str| {
            format!(
                "[[actor_capabilities]]\nactor_entity_key = \"demo/hero\"\ncapability_id = \"demo/cap\"\nsource = \"manifest\"\nbundle_id = \"demo\"\nvisibility = \"{visibility}\"\n"
            )
        };
        let first = input(&grant("public"), &[]);
        let memory = memory_after(&first);
        let mut next = input(&grant("owner"), &[]);
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(
            !codes(&out).contains(&"MF-APPLY-021"),
            "{:?}",
            out.diagnostics
        );
        assert!(out.send);
    }

    // ADR-0146 D-3: spatial_state_definitions left the insert-only list in
    // manifest-natural-keys task 2.4; a changed one is sent as a configure.
    #[test]
    fn a_changed_spatial_state_definition_is_sent() {
        let first = input(&spatial_state("demo/wet", "Wet"), &[]);
        let memory = memory_after(&first);
        let mut next = input(&spatial_state("demo/wet", "Soaked"), &[]);
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        assert!(out.send);
    }

    /// manifest-natural-keys task 4.1 (PR #3747): the server now upserts
    /// `place_edge` on `edge_key`, so a changed row is sent as a configure
    /// instead of refused (the former `MF-APPLYSET-NOT-UPDATABLE`).
    #[test]
    fn a_changed_place_edge_is_sent() {
        let place_graph = "[[place_graphs]]\nplace_graph_id = \"demo/g\"\nbundle_id = \"demo\"\nsubstrate_id = \"indoor\"\ndisplay_name = \"G\"\nanchor_node_id = \"demo/g/a\"\n";
        let place_node = |local_id: &str| {
            format!(
                "[[place_nodes]]\nnode_id = \"demo/g/{local_id}\"\nbundle_id = \"demo\"\nplace_graph_id = \"demo/g\"\nlocal_id = \"{local_id}\"\nsubstrate_id = \"indoor\"\ndisplay_name = \"{local_id}\"\n"
            )
        };
        let base = format!("{place_graph}{}{}", place_node("a"), place_node("b"));
        let edge = |elevation: f32| {
            format!(
                "{base}[[place_edges]]\nedge_key = \"demo/e\"\nbundle_id = \"demo\"\nplace_graph_id = \"demo/g\"\nfrom_node_id = \"demo/g/a\"\nto_node_id = \"demo/g/b\"\npose_dt_elevation = {elevation}\n"
            )
        };
        let first = input(&edge(0.0), &[]);
        let memory = memory_after(&first);
        let mut next = input(&edge(1.5), &[]);
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        assert!(out.send);
    }

    #[test]
    fn a_reference_to_an_unchanged_row_resolves() {
        // A new graph node in b.toml points at the unchanged graph in the
        // root. Validated alone, the delta would not see the graph.
        let root_rows = format!("{GRAPH}{}", node("demo/g/start", "start"));
        let first = input(&root_rows, &[("b.toml", String::new())]);
        let memory = memory_after(&first);
        let mut next = input(&root_rows, &[("b.toml", node("demo/g/end", "end"))]);
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(codes(&out).is_empty(), "{:?}", out.diagnostics);
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(delta.outcome_graph_nodes.len(), 1);
        assert!(delta.outcome_graphs.is_empty());
        let bare: Vec<_> = spellkave_validate::validate_manifest(&delta)
            .into_iter()
            .filter(|d| d.severity == Severity::Fatal)
            .map(|d| d.code)
            .collect();
        assert!(
            bare.iter()
                .any(|c| c == spellkave_validate::refs::MF_VAL_OUTCOME_GRAPH_NODE_REF),
            "the bare delta alone does not validate ({bare:?}), so the whole-root check matters"
        );
    }

    #[test]
    fn prune_is_refused_while_the_base_is_unknown() {
        let mut memory = memory_after(&base());
        memory.base_state = BaseState::Unknown;
        let mut next = base();
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(codes(&out), vec!["MF-APPLYSET-PRUNE-UNKNOWN"]);
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == "MF-APPLYSET-BASE-UNKNOWN"));
    }

    /// A live export of bundle `demo` holding the given rows.
    fn export(rows: &str) -> LiveExport {
        let manifest = spellkave_format::parse_manifest(&format!("{HEADER}{rows}"))
            .unwrap()
            .manifest;
        LiveExport {
            bundle_id: "demo".into(),
            manifest_json: serde_json::to_string(&manifest).unwrap(),
        }
    }

    #[test]
    fn a_drifted_row_is_flagged_but_keeps_its_class() {
        let memory = memory_after(&base());
        let mut next = base();
        next.memory = Some(memory);
        // Live, demo/b was changed behind the files' back; demo/z exists only live.
        next.live = vec![export(&format!(
            "{}{}{}{}",
            def("demo/a", "a"),
            def("demo/b", "edited live"),
            def("demo/c", "c"),
            def("demo/z", "live only"),
        ))];
        let out = classify_and_build_delta(&next).unwrap();
        let b = out.rows.iter().find(|r| r.key == "demo/b").unwrap();
        assert_eq!(
            (b.class, b.live),
            (RowClass::Unchanged, Some(LiveState::Drift))
        );
        let a = out.rows.iter().find(|r| r.key == "demo/a").unwrap();
        assert_eq!(a.live, Some(LiveState::Same));
        assert!(!out.send, "drift never changes what is sent");
        assert_eq!(out.values.len(), 1);
        assert!(out.values[0].live.contains("edited live"));
        assert!(out.values[0].file.contains("\"b\""));
        let live_only: Vec<_> = out.live_only.iter().map(|r| r.key.as_str()).collect();
        assert!(live_only.contains(&"demo/z"), "{live_only:?}");
        assert!(!live_only.contains(&"demo/b"));
    }

    #[test]
    fn a_configure_row_carries_its_live_and_file_values() {
        let memory = memory_after(&base());
        let mut next = input(
            &def("demo/a", "a"),
            &[
                ("b.toml", def("demo/b", "B!")),
                ("c.toml", def("demo/c", "c")),
            ],
        );
        next.memory = Some(memory);
        next.live = vec![export(&format!(
            "{}{}",
            def("demo/b", "b"),
            def("demo/c", "c")
        ))];
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(out.values.len(), 1);
        assert_eq!(out.values[0].key, "demo/b");
        assert_eq!(out.values[0].part, "b.toml");
        assert!(out.values[0].file.contains("B!"));
        // demo/a is missing from the export: not comparable, not live-only.
        let a = out.rows.iter().find(|r| r.key == "demo/a").unwrap();
        assert_eq!(a.live, Some(LiveState::NotComparable));
    }

    #[test]
    fn fields_the_export_does_not_round_trip_are_not_drift() {
        let masked = comparable(
            "affordance_templates",
            &serde_json::json!({"locality_class": "local", "predicate_bool": false, "visibility": "x", "keep": 1}),
        );
        assert_eq!(masked, serde_json::json!({"keep": 1}));
        let other = comparable(
            "entity_definitions",
            &serde_json::json!({"locality_class": "local"}),
        );
        assert_eq!(
            other,
            serde_json::json!({"locality_class": "local"}),
            "only the named kind"
        );
    }

    fn entity_at(key: &str, kind: &str, place: &str) -> String {
        format!(
            "[[entities]]\nentity_key = \"{key}\"\nbundle_id = \"demo\"\nname = \"{key}\"\nkind = \"{kind}\"\nplace_node_id = \"{place}\"\n"
        )
    }

    fn asl(key: &str, place: &str) -> String {
        format!(
            "[[actor_spatial_locations]]\nbundle_id = \"demo\"\nentity_key = \"{key}\"\nplace_node_id = \"{place}\"\nselection_path = \"manifest:{key}\"\n"
        )
    }

    fn place_of(kind: &str, key: &str, place: &str) -> LivePlace {
        LivePlace {
            kind: kind.into(),
            key: key.into(),
            place_node_id: place.into(),
        }
    }

    fn export_patched(rows: &str, patch: impl FnOnce(&mut Value)) -> LiveExport {
        let manifest = spellkave_format::parse_manifest(&format!("{HEADER}{rows}"))
            .unwrap()
            .manifest;
        let mut value = serde_json::to_value(&manifest).unwrap();
        patch(&mut value);
        LiveExport {
            bundle_id: "demo".into(),
            manifest_json: value.to_string(),
        }
    }

    fn prune_after(first_rows: &str) -> DiffInput {
        let first = input(first_rows, &[]);
        let memory = memory_after(&first);
        let mut next = input("", &[]);
        next.memory = Some(memory);
        next.prune = true;
        next
    }

    #[test]
    fn a_pruned_entity_with_the_same_place_is_same() {
        let mut next = prune_after(&entity_at("demo/item", "item", "demo/cell"));
        next.live = vec![export(&entity_at("demo/item", "item", "demo/cell"))];
        next.live_places = vec![place_of("entities", "demo/item", "demo/cell")];
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "entities" && r.key == "demo/item")
            .unwrap();
        assert_eq!(
            (row.class, row.live),
            (RowClass::Prune, Some(LiveState::Same))
        );
    }

    #[test]
    fn a_pruned_entity_moved_to_another_place_is_drift() {
        let mut next = prune_after(&entity_at("demo/item", "item", "demo/cell"));
        next.live = vec![export(&entity_at("demo/item", "item", "demo/other"))];
        next.live_places = vec![place_of("entities", "demo/item", "demo/other")];
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "entities" && r.key == "demo/item")
            .unwrap();
        assert_eq!(
            (row.class, row.live),
            (RowClass::Prune, Some(LiveState::Drift))
        );
    }

    #[test]
    fn a_pruned_entity_with_the_export_surrogate_def_id_is_same() {
        let mut next = prune_after(&entity_at("demo/item", "item", "demo/cell"));
        next.live = vec![export_patched(
            &entity_at("demo/item", "item", "demo/cell"),
            |value| {
                value["entities"][0]["entity_def_id"] = Value::String("demo:demo/item".into());
            },
        )];
        next.live_places = vec![place_of("entities", "demo/item", "demo/cell")];
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "entities" && r.key == "demo/item")
            .unwrap();
        assert_eq!(
            (row.class, row.live),
            (RowClass::Prune, Some(LiveState::Same))
        );
    }

    #[test]
    fn a_pruned_contained_entity_with_empty_place_is_same() {
        let mut next = prune_after(&entity_at("demo/item", "item", ""));
        next.live = vec![export(&entity_at("demo/item", "item", ""))];
        next.live_places = vec![place_of("entities", "demo/item", "")];
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "entities" && r.key == "demo/item")
            .unwrap();
        assert_eq!(
            (row.class, row.live),
            (RowClass::Prune, Some(LiveState::Same))
        );
    }

    #[test]
    fn a_pruned_entity_without_export_or_place_is_not_comparable() {
        let next = prune_after(&entity_at("demo/item", "item", "demo/cell"));
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "entities" && r.key == "demo/item")
            .unwrap();
        assert_eq!(
            (row.class, row.live),
            (RowClass::Prune, Some(LiveState::NotComparable))
        );

        let mut no_place = prune_after(&entity_at("demo/item", "item", "demo/cell"));
        no_place.live = vec![export(&entity_at("demo/item", "item", "demo/cell"))];
        let out = classify_and_build_delta(&no_place).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "entities" && r.key == "demo/item")
            .unwrap();
        assert_eq!(row.live, Some(LiveState::NotComparable));
    }

    #[test]
    fn a_pruned_spatial_row_with_the_same_place_is_same() {
        let rows = format!(
            "{}{}",
            entity_at("demo/body", "npc", "demo/cell"),
            asl("demo/body", "demo/cell")
        );
        let mut next = prune_after(&rows);
        next.live = vec![export(&rows)];
        next.live_places = vec![place_of(
            "actor_spatial_locations",
            "demo/body",
            "demo/cell",
        )];
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "actor_spatial_locations" && r.key == "demo/body")
            .unwrap();
        assert_eq!(
            (row.class, row.live),
            (RowClass::Prune, Some(LiveState::Same))
        );
    }

    #[test]
    fn a_pruned_spatial_row_moved_to_another_place_is_drift() {
        let rows = format!(
            "{}{}",
            entity_at("demo/body", "npc", "demo/cell"),
            asl("demo/body", "demo/cell")
        );
        let mut next = prune_after(&rows);
        next.live = vec![export(&format!(
            "{}{}",
            entity_at("demo/body", "npc", "demo/other"),
            asl("demo/body", "demo/other")
        ))];
        next.live_places = vec![place_of(
            "actor_spatial_locations",
            "demo/body",
            "demo/other",
        )];
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "actor_spatial_locations" && r.key == "demo/body")
            .unwrap();
        assert_eq!(
            (row.class, row.live),
            (RowClass::Prune, Some(LiveState::Drift))
        );
    }

    #[test]
    fn a_pruned_spatial_row_without_export_or_place_is_not_comparable() {
        let rows = format!(
            "{}{}",
            entity_at("demo/body", "npc", "demo/cell"),
            asl("demo/body", "demo/cell")
        );
        let next = prune_after(&rows);
        let out = classify_and_build_delta(&next).unwrap();
        let row = out
            .rows
            .iter()
            .find(|r| r.kind == "actor_spatial_locations" && r.key == "demo/body")
            .unwrap();
        assert_eq!(row.live, Some(LiveState::NotComparable));
    }

    #[test]
    fn without_an_export_every_row_is_not_comparable() {
        let memory = memory_after(&base());
        let mut next = base();
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out
            .rows
            .iter()
            .all(|r| r.live == Some(LiveState::NotComparable)));
        assert!(out.values.is_empty() && out.live_only.is_empty());
    }

    #[test]
    fn another_root_path_is_warned_about() {
        let memory = memory_after(&base());
        assert_eq!(memory.root_path, "fixtures/demo/manifest.toml");
        let mut next = base();
        next.memory = Some(memory);
        next.root_path = "elsewhere/manifest.toml".into();
        let out = classify_and_build_delta(&next).unwrap();
        assert!(out
            .warnings
            .iter()
            .any(|w| w.code == "MF-APPLYSET-ROOT-MOVED"));
        assert_eq!(out.root_path, "elsewhere/manifest.toml");
    }

    #[test]
    fn a_missing_include_file_is_an_error() {
        let mut next = base();
        next.files.remove("c.toml");
        let err = classify_and_build_delta(&next).unwrap_err();
        assert!(err.contains("c.toml"), "{err}");
    }

    #[test]
    fn root_info_names_the_applyset_and_includes_in_order() {
        let info = applyset_root_info(&base().root.toml).unwrap();
        assert_eq!(info.applyset.as_deref(), Some("demo"));
        assert_eq!(info.includes, vec!["b.toml", "c.toml"]);
    }

    // manifest-natural-keys 5.1: retraction_for's generic two-element join,
    // and the frame-write rename, exercised directly (retraction_for is
    // private, reachable here through `use super::*`).

    #[test]
    fn entity_reference_resolution_matches_server_bundle_rules() {
        assert_eq!(
            resolve_entity_ref("a/item", Some("a")),
            "a:a/item",
            "a row with a bundle uses that bundle"
        );
        assert_eq!(
            resolve_entity_ref("b/item", None),
            "b:b/item",
            "containment derives its bundle from the path"
        );
        assert_eq!(
            resolve_entity_ref("b:b/item", Some("a")),
            "b:b/item",
            "an explicit manifest key wins over the row bundle"
        );
        assert_eq!(
            resolve_entity_ref("b:item/item", None),
            "b:item:b:item/item",
            "a containment path derives its leading segment even when it contains a colon"
        );
    }

    #[test]
    fn retraction_for_entity_uses_the_manifest_bundle_key() {
        let r = retraction_for("entities", "demo/item", Some("demo"));
        assert_eq!(r.row_kind, "entity");
        assert_eq!(r.authored_key, "demo:demo/item");
    }

    #[test]
    fn retraction_for_entity_definitions_renames_the_kind() {
        let r = retraction_for("entity_definitions", "demo/def", Some("demo"));
        assert_eq!(r.row_kind, "entity_definition");
        assert_eq!(r.authored_key, "demo/def");
    }

    #[test]
    fn retraction_for_outcome_graphs_renames_the_kind() {
        let r = retraction_for("outcome_graphs", "demo/g", None);
        assert_eq!(r.row_kind, "outcome_graph");
        assert_eq!(r.authored_key, "demo/g");
    }

    #[test]
    fn retraction_for_outcome_graph_nodes_renames_the_kind() {
        let r = retraction_for("outcome_graph_nodes", "demo/g/start", None);
        assert_eq!(r.row_kind, "outcome_graph_node");
        assert_eq!(r.authored_key, "demo/g/start");
    }

    #[test]
    fn retraction_for_outcome_graph_edges_renames_the_kind() {
        let r = retraction_for("outcome_graph_edges", "demo/g/e1", None);
        assert_eq!(r.row_kind, "outcome_graph_edge");
        assert_eq!(r.authored_key, "demo/g/e1");
    }

    #[test]
    fn retraction_for_consequence_templates_renames_the_kind() {
        let r = retraction_for("consequence_templates", "demo/hit/success", None);
        assert_eq!(r.row_kind, "consequence_template");
        assert_eq!(r.authored_key, "demo/hit/success");
    }

    #[test]
    fn retraction_for_roll_tables_renames_the_kind() {
        let r = retraction_for("roll_tables", "demo/d20", None);
        assert_eq!(r.row_kind, "roll_table");
        assert_eq!(r.authored_key, "demo/d20");
    }

    #[test]
    fn retraction_for_roll_table_entries_renames_the_kind() {
        let r = retraction_for("roll_table_entries", "demo/d20:1", None);
        assert_eq!(r.row_kind, "roll_table_entry");
        assert_eq!(r.authored_key, "demo/d20:1");
    }

    #[test]
    fn retraction_for_joins_an_entity_definition_satellite_key_with_a_colon() {
        let key = canonical_json(&serde_json::json!([
            "srd/item/assassins-blood",
            "srd/cost_cp"
        ]));
        let r = retraction_for("entity_definition_initial_state", &key, None);
        assert_eq!(r.row_kind, "entity_definition_initial_state");
        assert_eq!(r.authored_key, "srd/item/assassins-blood:srd/cost_cp");
    }

    #[test]
    fn retraction_for_entity_definition_equip_modifier_joins_the_key() {
        let key = canonical_json(&serde_json::json!(["srd/item/sword", "srd/modifier/plus1"]));
        let r = retraction_for("entity_definition_equip_modifier", &key, None);
        assert_eq!(r.row_kind, "entity_definition_equip_modifier");
        assert_eq!(r.authored_key, "srd/item/sword:srd/modifier/plus1");
    }

    #[test]
    fn retraction_for_tag_capability_grant_still_joins_the_key() {
        let key = canonical_json(&serde_json::json!(["srd.spell", "srd/cap/cast"]));
        let r = retraction_for("tag_capability_grant", &key, None);
        assert_eq!(r.row_kind, "tag_capability_grant");
        assert_eq!(r.authored_key, "srd.spell:srd/cap/cast");
    }

    #[test]
    fn retraction_for_consequence_frame_writes_renames_the_kind_and_keeps_the_template_id() {
        let r = retraction_for(SET_LEVEL_FIELD, "demo/hit/success", None);
        assert_eq!(r.row_kind, "consequence_template_frame_write");
        assert_eq!(r.authored_key, "demo/hit/success");
    }

    #[test]
    fn retraction_for_a_wider_composite_key_is_left_untransformed() {
        // metadata_attachments (4 columns) has no retraction cascade today;
        // the generic two-element join must not misfire on it.
        let key = canonical_json(&serde_json::json!([
            "entity_definition",
            "srd/x",
            "srd.text",
            "srd"
        ]));
        let r = retraction_for("metadata_attachments", &key, None);
        assert_eq!(r.row_kind, "metadata_attachments");
        assert_eq!(r.authored_key, key);
    }

    fn frame_write_template(template_id: &str) -> String {
        format!(
            "[[consequence_templates]]\ntemplate_id = \"{template_id}\"\nbundle_id = \"demo\"\neffect_refs_csv = \"\"\ntraces_json = \"[]\"\ndescription = \"\"\n"
        )
    }

    fn frame_write(template_id: &str, ordinal: u32, marker: &str) -> String {
        format!(
            "[[consequence_frame_writes]]\ntemplate_id = \"{template_id}\"\nordinal = {ordinal}\nframe_write = {{ ModifyResource = {{ actor = \"Actor\", resource = \"demo/marker-{marker}\", delta_source = \"1\" }} }}\n"
        )
    }

    #[test]
    fn a_changed_frame_write_set_sends_every_block_not_just_the_changed_one() {
        let template = frame_write_template("demo/hit/success");
        let first = input(
            &format!(
                "{template}{}{}",
                frame_write("demo/hit/success", 0, "a"),
                frame_write("demo/hit/success", 1, "b"),
            ),
            &[],
        );
        let memory = memory_after(&first);
        let mut next = input(
            &format!(
                "{template}{}{}",
                frame_write("demo/hit/success", 0, "a"),
                frame_write("demo/hit/success", 1, "changed"),
            ),
            &[],
        );
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert_eq!(
            classes(&out)
                .into_iter()
                .filter(|(_, kind, _)| *kind == SET_LEVEL_FIELD)
                .collect::<Vec<_>>(),
            vec![(RowClass::Configure, SET_LEVEL_FIELD, "demo/hit/success")],
            "the whole set is one changed row, not two"
        );
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(
            delta.consequence_frame_writes.len(),
            2,
            "both blocks of the changed template must be sent, not just the one that changed: {:?}",
            delta.consequence_frame_writes
        );
        let ordinals: Vec<u32> = delta
            .consequence_frame_writes
            .iter()
            .map(|row| row.ordinal)
            .collect();
        assert_eq!(ordinals, vec![0, 1]);
    }

    #[test]
    fn a_split_frame_write_set_warns_naming_its_parts() {
        let template = frame_write_template("demo/hit/success");
        let out = classify_and_build_delta(&input(
            &format!("{template}{}", frame_write("demo/hit/success", 0, "a")),
            &[("b.toml", frame_write("demo/hit/success", 1, "b"))],
        ))
        .unwrap();
        let split = out
            .warnings
            .iter()
            .find(|w| w.code == "MF-APPLYSET-SPLIT-SET")
            .unwrap_or_else(|| panic!("no split-set warning in {:?}", out.warnings));
        assert!(split.message.contains("demo/hit/success"), "{split:?}");
        assert_eq!(
            split.details.as_ref().and_then(|d| d.get("parts")),
            Some(&serde_json::json!(["manifest.toml", "b.toml"])),
        );
        // Confined to one part: no split-set warning for it.
        let unsplit = classify_and_build_delta(&input(
            &format!(
                "{template}{}{}",
                frame_write("demo/hit/success", 0, "a"),
                frame_write("demo/hit/success", 1, "b"),
            ),
            &[],
        ))
        .unwrap();
        assert!(
            unsplit
                .warnings
                .iter()
                .all(|w| w.code != "MF-APPLYSET-SPLIT-SET"),
            "{:?}",
            unsplit.warnings
        );
    }

    #[test]
    fn pruning_a_frame_write_set_retracts_it_by_template_id() {
        let template = frame_write_template("demo/hit/success");
        let first = input(
            &format!(
                "{template}{}{}",
                frame_write("demo/hit/success", 0, "a"),
                frame_write("demo/hit/success", 1, "b"),
            ),
            &[],
        );
        let memory = memory_after(&first);
        let mut next = input(&frame_write_template("demo/hit/success"), &[]);
        next.memory = Some(memory);
        next.prune = true;
        let out = classify_and_build_delta(&next).unwrap();
        let delta = spellkave_format::parse_manifest(&out.delta_toml)
            .unwrap()
            .manifest;
        assert_eq!(
            delta.retractions,
            vec![RetractionRow {
                row_kind: "consequence_template_frame_write".into(),
                authored_key: "demo/hit/success".into(),
            }]
        );
    }

    #[test]
    fn an_unchanged_frame_write_set_sends_nothing() {
        let template = frame_write_template("demo/hit/success");
        let manifest = format!(
            "{template}{}{}",
            frame_write("demo/hit/success", 0, "a"),
            frame_write("demo/hit/success", 1, "b"),
        );
        let first = input(&manifest, &[]);
        let memory = memory_after(&first);
        let mut next = input(&manifest, &[]);
        next.memory = Some(memory);
        let out = classify_and_build_delta(&next).unwrap();
        assert!(!out.send, "{:?}", out.rows);
        assert!(out
            .rows
            .iter()
            .all(|r| r.kind != SET_LEVEL_FIELD || r.class == RowClass::Unchanged));
    }
}
