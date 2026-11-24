// Row identity and the semantic row hash (ADR-0145 D-2).
//
// A row is one `[[...]]` block of a manifest: one element of one of
// `Manifest`'s row arrays. Its identity is the row kind (the manifest field
// name) plus its key from `row_kinds.rs`: the single natural-key field, or the
// composite key for junction rows. A kind with neither is *untrackable* — it
// is reported, never identified by position or content.
//
// The hash is taken over the PARSED, TYPED row, so defaults are already
// filled in (an omitted field and its explicit default hash equal) and
// comments, whitespace, key order and the file holding the block do not
// matter. Object keys are sorted explicitly before hashing instead of relying
// on `serde_json`'s map type: a workspace build that enables the
// `preserve_order` feature must still produce the same hash on the server
// (native) and in the CLI (wasm).
//
// Both the server (`r_apply_bundle`, plan-phase check) and the CLI (through
// `spellkave-authoring-wasm`) call this module, so they agree on "the same
// row" and "a changed row" by construction.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::row_kinds::ROW_KIND_NATURAL_KEY;
use crate::Manifest;
/// Ordered storage-key fields for manifest kinds with composite identities.
/// The authoring identity can use an authored reference, such as
/// `object_state.entity_key`, where the manifest has no runtime id.
/// [`crate::row_kinds::ROW_KIND_COMPOSITE_KEY`] describes the narrower
/// composite keys used by the row-kind query interface.
pub const ROW_KIND_STORAGE_KEY: &[(&str, &[&str])] = &[
    ("role_aggregations", &["role_id", "bundle_id"]),
    (
        "actor_capabilities",
        &[
            "actor_entity_key",
            "capability_id",
            "source",
            "source_args_hash",
        ],
    ),
    ("object_state", &["entity_key", "state_id"]),
    (
        "entity_containment",
        &["container_entity_key", "entity_key"],
    ),
    ("actor_equipment_slots", &["actor_entity_key", "slot_id"]),
    ("actor_spatial_locations", &["entity_key"]),
    // manifest-natural-keys 5.1: widened from the single-column `entity_key`
    // (former `ROW_KIND_NATURAL_KEY` entry) to match the server's real
    // `(actor_id, identity)` key — `entity_key` is the authored name for
    // `actor_id` (D2's authored-name rule); two different identities can
    // legitimately bind the same entity_key over its lifetime (a re-login, a
    // body swap), which the old single-column key could not distinguish.
    ("actor_identities", &["entity_key", "identity"]),
    (
        "metadata_attachments",
        &["target_kind", "target_ref", "namespace", "bundle_id"],
    ),
    (
        "entity_definition_initial_state",
        &["entity_def_id", "state_id"],
    ),
    (
        "entity_definition_initial_capability",
        &["entity_def_id", "capability_id"],
    ),
    (
        "entity_definition_initial_condition",
        &["entity_def_id", "condition_definition_id"],
    ),
    (
        "entity_definition_initial_containment",
        &["entity_def_id", "contained_entity_def_id"],
    ),
    (
        "entity_definition_initial_equipment",
        &["entity_def_id", "slot_id"],
    ),
    (
        "entity_definition_item_dsl_source",
        &["entity_def_id", "slot"],
    ),
    (
        "entity_definition_equip_modifier",
        &["entity_def_id", "modifier_template_id"],
    ),
    ("metadata_namespaces", &["namespace", "bundle_id"]),
    (
        "place_edge_affordance",
        &["edge_key", "affordance_template_id", "mode_id"],
    ),
    ("place_node_traces", &["node_id", "trace_kind_id"]),
    ("location_traces", &["location_def_id", "trace_kind_id"]),
    (
        "movement_defaults",
        &[
            "bundle_id",
            "place_graph_id",
            "mode_id",
            "affordance_template_id",
        ],
    ),
    (
        "spatial_niche_attachments",
        &["bundle_id", "place_node_id", "place_edge_id", "niche_id"],
    ),
    (
        "spatial_environment_state",
        &[
            "bundle_id",
            "place_node_id",
            "place_edge_id",
            "place_edge_key",
            "location_def_id",
            "state_id",
        ],
    ),
    (
        "entity_resource_bound",
        &["entity_key", "resource_definition_id"],
    ),
    (
        "affordance_template_participants",
        &["template_id", "role_id"],
    ),
    (
        "target_conditions",
        &["target_kind", "target_id", "condition_definition_id"],
    ),
    ("tag_capability_grant", &["tag", "capability_id"]),
    (
        "condition_capability_extent_grants",
        &[
            "condition_definition_id",
            "capability_id",
            "extent_kind_ref",
            "extent_ref",
        ],
    ),
    (
        "entity_definition_tag",
        &["entity_def_id", "tag", "bundle_id", "source"],
    ),
    (
        "object_state_definition_tag",
        &["state_id", "tag", "bundle_id", "source"],
    ),
    (
        "capability_definition_tag",
        &["capability_id", "tag", "bundle_id", "source"],
    ),
    (
        "niche_definition_tag",
        &["niche_id", "tag", "bundle_id", "source"],
    ),
    (
        "outcome_template_tag",
        &["outcome_template_id", "tag", "bundle_id", "source"],
    ),
    (
        "action_definition_tag",
        &["action_id", "tag", "bundle_id", "source"],
    ),
    (
        "resource_definition_tag",
        &["resource_definition_id", "tag", "bundle_id", "source"],
    ),
    (
        "roll_table_tag",
        &["roll_table_id", "tag", "bundle_id", "source"],
    ),
    (
        "roll_table_entry_tag",
        &["roll_table_entry_id", "tag", "bundle_id", "source"],
    ),
    (
        "modifier_template_tag",
        &["modifier_template_id", "tag", "bundle_id", "source"],
    ),
    (
        "condition_definition_tag",
        &["condition_definition_id", "tag", "bundle_id", "source"],
    ),
    ("entity_tag", &["entity_key", "tag", "bundle_id", "source"]),
];

/// `ROW_KIND_STORAGE_KEY` as a JSON object string (`{"<row_kind>":
/// ["<field>", ...], ...}`) — same shape and discipline as
/// `row_kinds::row_kind_composite_key_json`/`row_kind_natural_key_json`
/// (sk-jay0.7.174.30): the wasm-exported counterpart TS callers
/// (`@spellkave/world-author`'s `row-kinds.ts`) read instead of hand-
/// carrying a copy of this table.
pub fn row_kind_storage_key_json() -> String {
    let mut map = serde_json::Map::with_capacity(ROW_KIND_STORAGE_KEY.len());
    for (kind, fields) in ROW_KIND_STORAGE_KEY {
        map.insert(
            (*kind).to_owned(),
            Value::Array(
                fields
                    .iter()
                    .map(|field| Value::String((*field).to_owned()))
                    .collect(),
            ),
        );
    }
    serde_json::to_string(&Value::Object(map)).unwrap_or_else(|_| "{}".to_owned())
}

// `DELTA_INSERT_ONLY_ROW_KINDS` and `delta_insert_only_label` (the CLI-side
// `MF-APPLY-021` pre-send refusal for a kind the server could only insert)
// were removed here once the list's last members gained a real server-side
// upsert: `place_node_traces`/`location_traces`/`movement_defaults`/
// `spatial_niche_attachments`/`spatial_environment_state` in
// manifest-natural-keys task 4.2, `actor_capabilities` in task 3.3. Every
// manifest row kind with a registered identity now has a matching
// upsert-in-place path.

/// Manifest fields that are loader or delta directives, not rows.
///
/// `place_graph_instances` joined this list in manifest-natural-keys 1.1: it
/// is a one-shot expansion command, not stored content. Its `instance_id` is
/// optional. Omitting it produces a fresh
/// non-deterministic `<blueprint_id>#<short_hex>` id on every apply — even an
/// UNCHANGED, byte-identical redeclare with no `instance_id` expands a whole
/// new place-graph instance rather than converging on the old one (never an
/// upsert). A row kind with no reproducible identity when a key field is
/// absent cannot be a tracked row; it is a directive, like `retractions`.
const NON_ROW_FIELDS: &[&str] = &["include", "retractions", "place_graph_instances"];

/// `consequence_frame_writes` is a Class-β kind (manifest-natural-keys 1.2,
/// design D3): the server replaces the WHOLE ordered list per template on
/// every apply path, so no per-row key exists. It is handled by its own
/// branch in [`manifest_row_identities`], not the generic per-row loop, and
/// is therefore excluded there the same way [`NON_ROW_FIELDS`] excludes a
/// directive — but it IS a row (a set-level one), so it does not go in that
/// list; `manifest_row_identities` special-cases this one field name instead.
pub const SET_LEVEL_FIELD: &str = "consequence_frame_writes";

/// One identified row: `(kind, key)` is the identity, `hash` the content.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RowIdentity {
    pub kind: String,
    /// The natural-key value, or for a composite key the JSON array of its
    /// values in registry order (e.g. `["srd/spell/fireball","srd.spell"]`).
    pub key: String,
    /// The row's own `bundle_id`, when the row kind carries one.
    pub bundle_id: Option<String>,
    /// Lower-case hex SHA-256 of the canonical row JSON.
    pub hash: String,
}

/// Rows of a kind with no registered key: counted per kind, never identified.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestRowIdentities {
    pub rows: Vec<RowIdentity>,
    /// `(kind, row count)` for kinds without a natural or composite key.
    pub untrackable: Vec<(String, usize)>,
}

/// The key fields of a row kind, or `None` when the kind has no registered
/// key. Returns `None` for [`SET_LEVEL_FIELD`]: its identity is not a list of
/// row-field names (see [`manifest_row_identities`]'s dedicated branch).
pub fn row_kind_key_fields(kind: &str) -> Option<Vec<&'static str>> {
    if let Some((_, field)) = ROW_KIND_NATURAL_KEY.iter().find(|(k, _)| *k == kind) {
        return Some(vec![*field]);
    }
    ROW_KIND_STORAGE_KEY
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, fields)| fields.to_vec())
}

/// Identify and hash every row of `manifest`, in manifest field order and
/// then row order. `include` and `retractions` are directives, not rows.
/// `consequence_frame_writes` (`SET_LEVEL_FIELD`) is a set-level kind: it
/// contributes at most one [`RowIdentity`] per distinct `template_id`,
/// instead of one per array element.
pub fn manifest_row_identities(manifest: &Manifest) -> ManifestRowIdentities {
    let value = serde_json::to_value(manifest).expect("Manifest serializes to JSON");
    let Value::Object(fields) = value else {
        unreachable!("Manifest serializes as an object")
    };
    let mut out = ManifestRowIdentities::default();
    for (kind, rows) in fields {
        let Value::Array(rows) = rows else { continue };
        if rows.is_empty() || NON_ROW_FIELDS.contains(&kind.as_str()) {
            continue;
        }
        if kind == SET_LEVEL_FIELD {
            out.rows.extend(frame_write_set_identities(&rows));
            continue;
        }
        let Some(key_fields) = row_kind_key_fields(&kind) else {
            out.untrackable.push((kind, rows.len()));
            continue;
        };
        for row in &rows {
            out.rows.push(RowIdentity {
                kind: kind.clone(),
                key: row_key(row, &key_fields),
                bundle_id: row
                    .get("bundle_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                hash: row_hash(row),
            });
        }
    }
    out
}

/// One [`RowIdentity`] per distinct `template_id` among `rows` (parsed
/// `ConsequenceFrameWriteRow` values): the key is the template id, and the
/// hash covers every block for that template ORDERED BY `ordinal` (design
/// D3) — not by manifest/array order, so reordering two blocks of the same
/// template in one file does not change the hash. `ConsequenceFrameWriteRow`
/// carries no `bundle_id` of its own, so `bundle_id` is always `None` here.
fn frame_write_set_identities(rows: &[Value]) -> Vec<RowIdentity> {
    let mut by_template: std::collections::BTreeMap<String, Vec<&Value>> =
        std::collections::BTreeMap::new();
    for row in rows {
        let template_id = row
            .get("template_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        by_template.entry(template_id).or_default().push(row);
    }
    by_template
        .into_iter()
        .map(|(template_id, mut blocks)| {
            blocks.sort_by_key(|row| row.get("ordinal").and_then(Value::as_u64).unwrap_or(0));
            let ordered = Value::Array(blocks.into_iter().cloned().collect());
            RowIdentity {
                kind: SET_LEVEL_FIELD.to_owned(),
                key: template_id,
                bundle_id: None,
                hash: row_hash(&ordered),
            }
        })
        .collect()
}

/// The identity key of one serialized row of `kind`, or `None` when the kind
/// has no registered key (the same key [`manifest_row_identities`] reports).
pub fn row_key_of(kind: &str, row: &Value) -> Option<String> {
    if kind == SET_LEVEL_FIELD {
        return Some(
            row.get("template_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        );
    }
    row_kind_key_fields(kind).map(|fields| row_key(row, &fields))
}

fn row_key(row: &Value, key_fields: &[&str]) -> String {
    let parts: Vec<Value> = key_fields
        .iter()
        .map(|field| row.get(*field).cloned().unwrap_or(Value::Null))
        .collect();
    match parts.as_slice() {
        [Value::String(single)] => single.clone(),
        [single] => canonical_json(single),
        _ => canonical_json(&Value::Array(parts)),
    }
}

/// SHA-256 over the canonical JSON of one serialized row.
pub fn row_hash(row: &Value) -> String {
    use std::fmt::Write;
    let digest = Sha256::digest(canonical_json(row).as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest.iter() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// JSON with object keys sorted at every level and no insignificant whitespace.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

/// Bundle-qualified entity storage keys shared by apply admission and advisory preflight.
pub fn ownership_key(kind: &str, key: &str, bundle: Option<&str>) -> (String, String) {
    let key = if matches!(kind, "entities" | "actors") {
        bundle.map_or_else(|| key.to_owned(), |bundle| format!("{bundle}:{key}"))
    } else {
        key.to_owned()
    };
    (kind.to_owned(), key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_identity_is_owned_tuple_without_delimiter_collisions() {
        assert_eq!(
            row_kind_key_fields("role_definitions"),
            Some(vec!["role_id"])
        );
        assert_eq!(
            row_kind_key_fields("role_aggregations"),
            Some(vec!["role_id", "bundle_id"])
        );
        let a = serde_json::json!({"role_id": "a:b", "bundle_id": "c"});
        let b = serde_json::json!({"role_id": "a", "bundle_id": "b:c"});
        assert_ne!(
            row_key_of("role_aggregations", &a),
            row_key_of("role_aggregations", &b)
        );
        assert_eq!(
            row_key_of("role_aggregations", &a),
            Some(r#"["a:b","c"]"#.into())
        );
    }

    #[test]
    fn every_manifest_row_field_has_a_key_and_none_is_untrackable() {
        // manifest-natural-keys 1.1's own acceptance bar: walk the ACTUAL
        // `Manifest` struct (not a hand-kept list, which drifts) and check
        // every array field the identity engine would see is either a
        // directive (`NON_ROW_FIELDS`), the one set-level kind
        // (`SET_LEVEL_FIELD`), or has a registered key. manifest-natural-keys
        // 4.2 registered the last three deferred kinds (`movement_defaults`,
        // `spatial_niche_attachments`, `spatial_environment_state`) alongside
        // their server-side upsert, so nothing is excluded here any more.
        let value = serde_json::to_value(Manifest::default()).expect("Manifest serializes");
        let fields = value.as_object().expect("Manifest serializes as an object");
        let mut checked = 0;
        for (kind, rows) in fields {
            if !rows.is_array() || NON_ROW_FIELDS.contains(&kind.as_str()) {
                continue;
            }
            checked += 1;
            if kind == SET_LEVEL_FIELD {
                continue;
            }
            assert!(
                row_kind_key_fields(kind).is_some(),
                "manifest field '{kind}' has no registered row identity"
            );
        }
        // Sanity: the walk actually visited a realistic number of row kinds,
        // so an empty/broken `Manifest::default()` can't pass this test by
        // vacuous truth.
        assert!(checked > 80, "expected 80+ row kinds, walked {checked}");
    }

    #[test]
    fn actor_capabilities_key_is_widened_per_task_3_1_audit() {
        assert_eq!(
            row_kind_key_fields("actor_capabilities"),
            Some(vec![
                "actor_entity_key",
                "capability_id",
                "source",
                "source_args_hash",
            ])
        );
    }

    #[test]
    fn metadata_attachments_key_includes_bundle_id() {
        assert_eq!(
            row_kind_key_fields("metadata_attachments"),
            Some(vec!["target_kind", "target_ref", "namespace", "bundle_id"])
        );
    }

    #[test]
    fn tag_kinds_key_is_widened_to_the_servers_real_key() {
        assert_eq!(
            row_kind_key_fields("entity_definition_tag"),
            Some(vec!["entity_def_id", "tag", "bundle_id", "source"])
        );
        // tag_capability_grant is NOT a `*_tag` kind and keeps its 2-column
        // shape (sk-bwd) — it must not be accidentally widened.
        assert_eq!(
            row_kind_key_fields("tag_capability_grant"),
            Some(vec!["tag", "capability_id"])
        );
    }

    #[test]
    fn place_graph_instances_is_a_directive_not_a_tracked_row() {
        assert!(NON_ROW_FIELDS.contains(&"place_graph_instances"));
        assert!(row_kind_key_fields("place_graph_instances").is_none());
    }

    #[test]
    fn consequence_frame_writes_is_one_identity_per_template_ordered_by_ordinal() {
        let rows = serde_json::json!([
            {"template_id": "demo/hit/success", "ordinal": 1, "frame_write": {"b": 2}},
            {"template_id": "demo/hit/success", "ordinal": 0, "frame_write": {"a": 1}},
            {"template_id": "demo/other", "ordinal": 0, "frame_write": {"c": 3}},
        ]);
        let rows = rows.as_array().expect("array").clone();
        let ids = frame_write_set_identities(&rows);
        assert_eq!(ids.len(), 2, "one identity per distinct template_id");
        let hit = ids
            .iter()
            .find(|r| r.key == "demo/hit/success")
            .expect("hit success set");
        assert_eq!(hit.kind, "consequence_frame_writes");
        assert_eq!(hit.bundle_id, None);

        // Reordering the two blocks of the SAME template in the source must
        // not change the hash: the set is ordered by `ordinal`, not by
        // manifest/array order.
        let reordered = serde_json::json!([
            {"template_id": "demo/hit/success", "ordinal": 0, "frame_write": {"a": 1}},
            {"template_id": "demo/hit/success", "ordinal": 1, "frame_write": {"b": 2}},
        ]);
        let reordered = reordered.as_array().expect("array").clone();
        let reordered_ids = frame_write_set_identities(&reordered);
        assert_eq!(reordered_ids[0].hash, hit.hash);

        // A changed block DOES change the set hash.
        let changed = serde_json::json!([
            {"template_id": "demo/hit/success", "ordinal": 1, "frame_write": {"b": 999}},
            {"template_id": "demo/hit/success", "ordinal": 0, "frame_write": {"a": 1}},
        ]);
        let changed = changed.as_array().expect("array").clone();
        let changed_ids = frame_write_set_identities(&changed);
        assert_ne!(changed_ids[0].hash, hit.hash);
    }

    #[test]
    fn row_key_of_consequence_frame_writes_is_its_template_id() {
        let row =
            serde_json::json!({"template_id": "demo/hit/success", "ordinal": 0, "frame_write": {}});
        assert_eq!(
            row_key_of("consequence_frame_writes", &row),
            Some("demo/hit/success".to_owned())
        );
    }
}
