//! Applyset part building (ADR-0145 D-1..D-3).
//!
//! An applyset is one root manifest plus its `[[include]]` fragments. Each file
//! is a *part*; a row's *residence* is the part it lives in. The caller (the
//! `sk` CLI, through `spellkave-authoring-wasm`) reads the files; this module
//! parses them, identifies and hashes every row with the shared
//! `spellkave_schema::row_identity` function, and decides residence: the LAST
//! part in include-flattening order (root first, then includes in declaration
//! order — the order the loaders concatenate) wins, and every row named in more
//! than one part is reported as a duplicate. Duplicates are warnings, not
//! errors (kubectl `apply -f` keeps the last object).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use spellkave_schema::row_identity::{manifest_row_identities, row_hash};

/// The one set-level manifest field (design D3, manifest-natural-keys
/// 1.2/1.4): `manifest_row_identities` already folds every part's OWN
/// `consequence_frame_writes` blocks into one per-template identity, which is
/// correct as long as a template's blocks all live in one part. It cannot see
/// across parts, so [`resolve_split_frame_write_sets`] below re-derives the
/// cross-part case from the raw parsed manifests.
const SET_LEVEL_FIELD: &str = "consequence_frame_writes";

/// One file of the applyset, as read by the caller.
#[derive(Debug, Clone, Deserialize)]
pub struct PartSource {
    /// Path relative to the root's directory (the root's own file name for the root).
    pub path: String,
    pub toml: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplysetSources {
    pub root: PartSource,
    /// Include fragments in the root's declaration order.
    #[serde(default)]
    pub fragments: Vec<PartSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartEntry {
    pub kind: String,
    pub key: String,
    pub bundle_id: Option<String>,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Part {
    pub path: String,
    /// Rows that RESIDE in this part (duplicates resolved to their last part).
    pub entries: Vec<PartEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Duplicate {
    pub kind: String,
    pub key: String,
    /// Every part naming the row, in flattening order; the last one wins.
    pub parts: Vec<String>,
    /// False when the copies differ in content (the last one is applied).
    pub identical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Untrackable {
    pub part: String,
    pub kind: String,
    pub count: usize,
}

/// A set-level row (only `consequence_frame_writes` today) whose blocks sit
/// in more than one part (design D3, manifest-natural-keys 1.4). Distinct
/// from [`Duplicate`]: a split set is not a content clash between two copies
/// of the same row — it is one row correctly assembled from blocks authored
/// in different files. Its residence entry's hash still covers every block,
/// in ordinal order, regardless of which part each one came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SplitSet {
    pub kind: String,
    pub key: String,
    /// Every part holding at least one block, in flattening order; the last
    /// one is the set's residence.
    pub parts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplysetParts {
    pub applyset: String,
    pub root_path: String,
    pub parts: Vec<Part>,
    pub duplicates: Vec<Duplicate>,
    pub untrackable: Vec<Untrackable>,
    #[serde(default)]
    pub split_sets: Vec<SplitSet>,
}

/// Build the part lists of one applyset. Fails when the root does not parse,
/// declares no `applyset`, or a fragment does not parse (including a fragment
/// that declares `applyset`).
pub fn build_applyset_parts(sources: &ApplysetSources) -> Result<ApplysetParts, String> {
    let root = spellkave_format::parse_manifest(&sources.root.toml)
        .map_err(|err| format!("{}: {err}", sources.root.path))?
        .manifest;
    let applyset = root.applyset.clone().ok_or_else(|| {
        format!(
            "{}: the root manifest declares no `applyset`; add a top-level `applyset = \"<name>\"` (ADR-0145)",
            sources.root.path
        )
    })?;

    let mut per_part = vec![(sources.root.path.clone(), root)];
    for fragment in &sources.fragments {
        let manifest = spellkave_format::parse_manifest_fragment(&fragment.toml)
            .map_err(|err| format!("{}: {err}", fragment.path))?;
        per_part.push((fragment.path.clone(), manifest));
    }
    let per_part_ids: Vec<(
        String,
        spellkave_schema::row_identity::ManifestRowIdentities,
    )> = per_part
        .iter()
        .map(|(path, manifest)| (path.clone(), manifest_row_identities(manifest)))
        .collect();

    // (kind, key) -> every (part index, hash) naming it, in flattening order.
    let mut seen: BTreeMap<(String, String), Vec<(usize, String)>> = BTreeMap::new();
    for (index, (_, ids)) in per_part_ids.iter().enumerate() {
        for row in &ids.rows {
            seen.entry((row.kind.clone(), row.key.clone()))
                .or_default()
                .push((index, row.hash.clone()));
        }
    }

    let mut parts = Vec::with_capacity(per_part_ids.len());
    let mut untrackable = Vec::new();
    for (index, (path, ids)) in per_part_ids.iter().enumerate() {
        let mut entries = Vec::new();
        let mut placed = std::collections::BTreeSet::new();
        for row in &ids.rows {
            let key = (row.kind.clone(), row.key.clone());
            let owners = &seen[&key];
            let (last_index, last_hash) = owners.last().expect("row was recorded");
            // Keep one entry per identity: in the last part that names it, the
            // last copy within that part.
            if *last_index != index || !placed.insert(key.clone()) {
                continue;
            }
            entries.push(PartEntry {
                export_hash: None,
                kind: row.kind.clone(),
                key: row.key.clone(),
                bundle_id: row.bundle_id.clone(),
                hash: last_hash.clone(),
            });
        }
        parts.push(Part {
            path: path.clone(),
            entries,
        });
        for (kind, count) in &ids.untrackable {
            untrackable.push(Untrackable {
                part: path.clone(),
                kind: kind.clone(),
                count: *count,
            });
        }
    }

    let duplicates = seen
        .iter()
        .filter(|(key, owners)| key.0 != SET_LEVEL_FIELD && owners.len() > 1)
        .map(|((kind, key), owners)| Duplicate {
            identical: owners.windows(2).all(|w| w[0].1 == w[1].1),
            parts: owners
                .iter()
                .map(|(i, _)| per_part_ids[*i].0.clone())
                .collect(),
            kind: kind.clone(),
            key: key.clone(),
        })
        .collect();

    let split_sets = resolve_split_frame_write_sets(&per_part, &seen, &mut parts);

    Ok(ApplysetParts {
        applyset,
        root_path: sources.root.path.clone(),
        parts,
        duplicates,
        untrackable,
        split_sets,
    })
}

/// Task 1.4 (design D3): a `consequence_frame_writes` template whose
/// blocks sit in more than one part is not a plain duplicate — each part's
/// generic identity above only hashes ITS OWN blocks, so the per-part "last
/// part wins" residence entry `parts` already carries is, for a split set,
/// hashed over an INCOMPLETE subset of the template's blocks. This corrects
/// that: it re-reads every part's raw `consequence_frame_writes` rows,
/// regroups them by `template_id` across ALL parts, and for every template
/// named in more than one part, replaces the residence part's entry with one
/// hashed over every block (ordered by `ordinal`) and reports a [`SplitSet`].
/// A template confined to one part is untouched: that part's own identity
/// already covers every one of its blocks.
fn resolve_split_frame_write_sets(
    per_part: &[(String, spellkave_schema::Manifest)],
    seen: &BTreeMap<(String, String), Vec<(usize, String)>>,
    parts: &mut [Part],
) -> Vec<SplitSet> {
    // template_id -> every (part_index, ordinal, frame_write) across every part.
    let mut blocks_by_template: BTreeMap<String, Vec<(usize, u64, Value)>> = BTreeMap::new();
    for (index, (_, manifest)) in per_part.iter().enumerate() {
        for row in &manifest.consequence_frame_writes {
            let frame_write =
                serde_json::to_value(row).expect("ConsequenceFrameWriteRow serializes");
            let ordinal = frame_write
                .get("ordinal")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            blocks_by_template
                .entry(row.template_id.clone())
                .or_default()
                .push((index, ordinal, frame_write));
        }
    }

    let mut split_sets = Vec::new();
    for (template_id, mut blocks) in blocks_by_template {
        let key = (SET_LEVEL_FIELD.to_owned(), template_id.clone());
        if !seen.contains_key(&key) {
            continue;
        }
        let mut part_indices: Vec<usize> = blocks.iter().map(|(index, ..)| *index).collect();
        part_indices.sort_unstable();
        part_indices.dedup();
        if part_indices.len() <= 1 {
            continue; // confined to one part: that part's own hash already covers it.
        }

        blocks.sort_by_key(|(_, ordinal, _)| *ordinal);
        let combined_hash = row_hash(&Value::Array(
            blocks.iter().map(|(_, _, v)| v.clone()).collect(),
        ));
        let residence_index = *part_indices.last().expect("non-empty");

        // Every non-residence part that the generic pass above placed an
        // (incomplete) entry for loses it; the residence part's entry is
        // replaced with the combined-hash one.
        for &index in &part_indices {
            parts[index]
                .entries
                .retain(|e| !(e.kind == SET_LEVEL_FIELD && e.key == template_id));
        }
        parts[residence_index].entries.push(PartEntry {
            export_hash: None,
            kind: SET_LEVEL_FIELD.to_owned(),
            key: template_id.clone(),
            bundle_id: None,
            hash: combined_hash,
        });

        split_sets.push(SplitSet {
            kind: SET_LEVEL_FIELD.to_owned(),
            key: template_id,
            parts: part_indices
                .iter()
                .map(|&i| per_part[i].0.clone())
                .collect(),
        });
    }
    split_sets
}

/// JSON in (`ApplysetSources`), JSON out (`ApplysetParts`) — the wasm boundary.
pub fn build_applyset_parts_json(sources_json: &str) -> Result<String, String> {
    let sources: ApplysetSources =
        serde_json::from_str(sources_json).map_err(|err| format!("applyset sources: {err}"))?;
    let parts = build_applyset_parts(&sources)?;
    serde_json::to_string(&parts).map_err(|err| err.to_string())
}

/// Identify and hash every row of one manifest fragment or root (JSON out:
/// `ManifestRowIdentities`). For callers that need identities without parts.
pub fn manifest_row_identities_json(toml_source: &str, is_root: bool) -> Result<String, String> {
    let manifest = if is_root {
        spellkave_format::parse_manifest(toml_source).map(|parsed| parsed.manifest)
    } else {
        spellkave_format::parse_manifest_fragment(toml_source)
    }
    .map_err(|err| err.to_string())?;
    serde_json::to_string(&manifest_row_identities(&manifest)).map_err(|err| err.to_string())
}

/// Identify a typed live manifest without a TOML roundtrip. Runtime u64 values
/// may exceed TOML's signed integer range; serde retains their exact JSON token.
pub fn live_manifest_row_identities_json(manifest_json: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(manifest_json).map_err(|err| format!("live manifest JSON: {err}"))?;
    // The schema's general flexible-u64 reader also accepts whole floats.
    // This exact comparison transport accepts integer number tokens or digit
    // strings; reject overflow before a float can saturate to u64::MAX.
    for (kind, fields) in spellkave_schema::U64_FIELDS_BY_ROW_KIND {
        for row in value
            .get(*kind)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for field in *fields {
                if let Some(number) = row.get(*field).filter(|v| v.is_number()) {
                    if number.as_u64().is_none() {
                        return Err(format!(
                            "live manifest JSON: {kind}.{field} requires an exact u64"
                        ));
                    }
                }
            }
        }
    }
    let manifest: spellkave_schema::Manifest =
        serde_json::from_value(value).map_err(|err| format!("live manifest JSON: {err}"))?;
    serde_json::to_string(&manifest_row_identities(&manifest)).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str =
        "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = \"1.0.0\"\n";

    fn def(id: &str, desc: &str) -> String {
        format!(
            "[[entity_definitions]]\nbundle_id = \"demo\"\nentity_def_id = \"{id}\"\ndisplay_name = \"X\"\ndescription = \"{desc}\"\n"
        )
    }

    fn sources(root_rows: &str, fragments: &[(&str, String)]) -> ApplysetSources {
        ApplysetSources {
            root: PartSource {
                path: "manifest.toml".into(),
                toml: format!("{HEADER}applyset = \"demo\"\n{root_rows}"),
            },
            fragments: fragments
                .iter()
                .map(|(path, toml)| PartSource {
                    path: (*path).into(),
                    toml: toml.clone(),
                })
                .collect(),
        }
    }

    #[test]
    fn rows_reside_in_their_part() {
        let out = build_applyset_parts(&sources(
            &def("demo/a", "a"),
            &[("b.toml", def("demo/b", "b"))],
        ))
        .unwrap();
        assert_eq!(out.applyset, "demo");
        assert_eq!(out.parts.len(), 2);
        assert_eq!(out.parts[0].entries[0].key, "demo/a");
        assert_eq!(out.parts[1].path, "b.toml");
        assert_eq!(out.parts[1].entries[0].key, "demo/b");
        assert!(out.duplicates.is_empty());
    }

    #[test]
    fn the_last_part_wins_and_duplicates_are_reported() {
        let out = build_applyset_parts(&sources(
            "",
            &[
                ("a.toml", def("demo/x", "same")),
                ("b.toml", def("demo/x", "same")),
                ("c.toml", def("demo/y", "y")),
            ],
        ))
        .unwrap();
        assert!(out.parts[1].entries.is_empty(), "a.toml loses its copy");
        assert_eq!(out.parts[2].entries[0].key, "demo/x");
        assert_eq!(
            out.duplicates,
            vec![Duplicate {
                kind: "entity_definitions".into(),
                key: "demo/x".into(),
                parts: vec!["a.toml".into(), "b.toml".into()],
                identical: true,
            }]
        );
    }

    #[test]
    fn differing_duplicates_keep_the_last_content() {
        let out = build_applyset_parts(&sources(
            "",
            &[
                ("a.toml", def("demo/x", "old")),
                ("b.toml", def("demo/x", "new")),
            ],
        ))
        .unwrap();
        assert!(!out.duplicates[0].identical);
        let single =
            build_applyset_parts(&sources("", &[("b.toml", def("demo/x", "new"))])).unwrap();
        assert_eq!(
            out.parts[2].entries[0].hash,
            single.parts[1].entries[0].hash
        );
    }

    #[test]
    fn a_root_without_applyset_is_refused() {
        let mut s = sources("", &[]);
        s.root.toml = HEADER.to_owned();
        let err = build_applyset_parts(&s).unwrap_err();
        assert!(err.contains("declares no `applyset`"), "{err}");
    }

    #[test]
    fn a_fragment_with_applyset_is_refused() {
        let err = build_applyset_parts(&sources("", &[("a.toml", "applyset = \"x\"\n".into())]))
            .unwrap_err();
        assert!(
            err.starts_with("a.toml:") && err.contains("applyset"),
            "{err}"
        );
    }

    fn frame_write(template_id: &str, ordinal: u32, marker: &str) -> String {
        format!(
            "[[consequence_frame_writes]]\ntemplate_id = \"{template_id}\"\nordinal = {ordinal}\nframe_write = {{ kind = \"modify_resource\", marker = \"{marker}\" }}\n"
        )
    }

    #[test]
    fn a_frame_write_set_confined_to_one_part_is_not_split() {
        let root = format!(
            "{}{}",
            frame_write("demo/hit/success", 0, "a"),
            frame_write("demo/hit/success", 1, "b"),
        );
        let out = build_applyset_parts(&sources(&root, &[])).unwrap();
        assert!(out.split_sets.is_empty());
        assert_eq!(out.parts[0].entries.len(), 1);
        assert_eq!(out.parts[0].entries[0].kind, "consequence_frame_writes");
        assert_eq!(out.parts[0].entries[0].key, "demo/hit/success");
    }

    #[test]
    fn a_split_frame_write_set_lives_in_its_last_part_and_warns() {
        // Two blocks in the root, a third (later, higher ordinal) in b.toml,
        // included after the root — b.toml is the LAST part in flattening
        // order that names this template.
        let root = format!(
            "{}{}",
            frame_write("demo/hit/success", 0, "a"),
            frame_write("demo/hit/success", 1, "b"),
        );
        let out = build_applyset_parts(&sources(
            &root,
            &[("b.toml", frame_write("demo/hit/success", 2, "c"))],
        ))
        .unwrap();

        assert_eq!(
            out.split_sets,
            vec![SplitSet {
                kind: "consequence_frame_writes".into(),
                key: "demo/hit/success".into(),
                parts: vec!["manifest.toml".into(), "b.toml".into()],
            }]
        );
        // The root's (incomplete, 2-block) entry is gone; b.toml holds the
        // one true entry, hashed over all three blocks.
        assert!(out.parts[0].entries.is_empty());
        assert_eq!(out.parts[1].entries.len(), 1);
        assert_eq!(out.parts[1].entries[0].key, "demo/hit/success");

        // Reassembling the SAME three blocks in one file (no split) must
        // hash identically — the combined hash does not depend on residence.
        let unsplit = format!("{root}{}", frame_write("demo/hit/success", 2, "c"));
        let single = build_applyset_parts(&sources(&unsplit, &[])).unwrap();
        assert_eq!(
            out.parts[1].entries[0].hash,
            single.parts[0].entries[0].hash
        );
    }

    #[test]
    fn an_unrelated_template_in_another_part_is_untouched_by_a_split_elsewhere() {
        let root = frame_write("demo/hit/success", 0, "a");
        let out = build_applyset_parts(&sources(
            &root,
            &[
                ("b.toml", frame_write("demo/hit/success", 1, "b")),
                ("c.toml", frame_write("demo/other/success", 0, "z")),
            ],
        ))
        .unwrap();
        assert_eq!(out.split_sets.len(), 1, "{:?}", out.split_sets);
        assert_eq!(out.split_sets[0].key, "demo/hit/success");
        assert_eq!(out.parts[2].entries.len(), 1);
        assert_eq!(out.parts[2].entries[0].key, "demo/other/success");
    }
    #[test]
    fn live_json_identities_preserve_unsigned_and_signed_extremes() {
        let source = r#"{"schema_version":"1.0.0","entities":[{"entity_key":"demo/chest","bundle_id":"demo","name":"Chest","kind":"item","place_node_id":"","quantity":18446744073709551615}],"object_state":[{"entity_key":"demo/chest","bundle_id":"demo","state_id":"demo/charge","source_template_id":"","value_int":9223372036854775807}]}"#;
        let manifest: spellkave_schema::Manifest = serde_json::from_str(source).unwrap();
        assert_eq!(manifest.entities[0].quantity, u64::MAX);
        assert_eq!(manifest.object_state[0].value_int, Some(i64::MAX));
        let expected = serde_json::to_value(manifest_row_identities(&manifest)).unwrap();
        let actual: Value =
            serde_json::from_str(&live_manifest_row_identities_json(source).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert!(live_manifest_row_identities_json(
            &source.replace("18446744073709551615", "18446744073709551616")
        )
        .is_err());
        assert!(live_manifest_row_identities_json(
            &source.replace("9223372036854775807", "9223372036854775808")
        )
        .is_err());
    }
}
