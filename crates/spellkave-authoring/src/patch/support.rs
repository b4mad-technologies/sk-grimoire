//! Shared helpers for the `patch_manifest_json` op dispatcher (#1968 R7,
//! ADR-0087, task section 3). Ported verbatim from
//! `@spellkave/world-author`'s `patch.ts` — same field-guard error text, same
//! upsert-by-natural-key semantics — so callers observe zero behavior change.
//!
//! Every op reads the FULL manifest JSON (needed for `first_bundle_id` and
//! for the existing rows of whichever table(s) it upserts against) but
//! returns ONLY the table(s) it actually mutated, as a JSON object
//! `{"<table>": [...], ...}`. This is deliberate: a whole-manifest
//! round-trip through `serde_json::Value` would re-serialize every OTHER
//! table too — not lossy on its own (a JS `number` already lost any
//! precision beyond `Number.MAX_SAFE_INTEGER` before it ever became JSON
//! text), but returning only the changed table(s) means an untouched
//! table's bytes never pass through this crate at all, the simplest possible
//! parity argument. `patch.ts`'s TS wrapper merges the returned table(s)
//! back onto the live manifest object.
//!
//! Numeric fields are always carried as the ORIGINAL `serde_json::Number`
//! parsed from the payload — never rebuilt from an `f64` — so an integral
//! JSON literal (`45`) can never drift into `45.0` on the way back out
//! (`serde_json::Number` is tagged int-vs-float at parse time from the exact
//! source text and its `Display`/serialize impl reproduces that tag
//! faithfully; reconstructing from `f64` would lose the tag and could emit a
//! spurious decimal point a downstream typed-integer field cannot parse).

use serde_json::{Map, Value};

/// A patch op's own error text always mirrors `patch.ts`'s (same substrings
/// so existing `toThrow(/regex/)` assertions keep matching unchanged).
pub type PatchResult<T> = Result<T, String>;

/// Get `manifest[key]` as an array, or an empty one if absent/not an array —
/// mirrors `patch.ts`'s `rowArray` read half. The write half (installing a
/// possibly-new array back onto the manifest) is the dispatcher's job, not
/// this helper's, because ops here only ever return the tables they touch.
pub fn row_array(manifest: &Value, key: &str) -> Vec<Value> {
    manifest
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Drop every row in `rows` whose `field` equals `value` — the substrate of
/// upsert: re-authoring a row by id replaces it rather than appending a
/// duplicate. Mirrors `patch.ts`'s `removeRowsByField`.
pub fn remove_rows_by_field(rows: Vec<Value>, field: &str, value: &str) -> Vec<Value> {
    rows.into_iter()
        .filter(|row| row.get(field).and_then(Value::as_str) != Some(value))
        .collect()
}

/// Drop every row in `rows` matching ALL of `fields` (composite-key
/// upsert/dedup) — mirrors the several ops in `patch.ts` that filter with a
/// multi-field `&&` predicate instead of the single-field
/// `removeRowsByField` (`add-capability-grant`, `add-actor-equipment-slot`,
/// `add-place-edge-affordance`, `add-place-node-trace`, and the roll-table
/// tag ops).
pub fn remove_rows_by_fields(rows: Vec<Value>, fields: &[(&str, &str)]) -> Vec<Value> {
    rows.into_iter()
        .filter(|row| {
            !fields
                .iter()
                .all(|(field, value)| row.get(*field).and_then(Value::as_str) == Some(*value))
        })
        .collect()
}

/// `manifest.bundles[0].bundle_id`, or `"author-cli"` if absent — mirrors
/// `patch.ts`'s `firstBundleId`.
pub fn first_bundle_id(manifest: &Value) -> String {
    manifest
        .get("bundles")
        .and_then(Value::as_array)
        .and_then(|bundles| bundles.first())
        .and_then(|first| first.get("bundle_id"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .unwrap_or("author-cli")
        .to_owned()
}

/// The `bundle_id` an op should stamp on a row it authors: `payload.bundle_id`
/// if present and non-empty, else `first_bundle_id(manifest)`. Mirrors
/// `patch.ts`'s shared `bundleId` local at the top of `patchManifest`.
pub fn resolved_bundle_id(manifest: &Value, payload: &Value) -> String {
    payload
        .get("bundle_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| first_bundle_id(manifest))
}

/// A required non-empty string field — mirrors `patch.ts`'s `stringField`.
pub fn string_field(payload: &Value, key: &str) -> PatchResult<String> {
    match payload.get(key).and_then(Value::as_str) {
        Some(value) if !value.is_empty() => Ok(value.to_owned()),
        _ => Err(format!("patch payload requires string field: {key}")),
    }
}

/// A required finite-number field, carried as the original tagged
/// `serde_json::Number` (see module doc). Mirrors `patch.ts`'s `numberField`.
pub fn number_field(payload: &Value, key: &str) -> PatchResult<Value> {
    match payload.get(key) {
        Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => {
            Ok(Value::Number(n.clone()))
        }
        _ => Err(format!("patch payload requires number field: {key}")),
    }
}

/// A field the schema types as `String` but that an author may reasonably
/// hand over as a number (e.g. a roll-table `result_ref` under
/// `result_kind == "int"`) — accepts either and stringifies via the
/// `Number`'s own `Display` (which reproduces its exact source digits, int
/// or float). Mirrors `patch.ts`'s `stringifiableField` (same error text as
/// `string_field`, deliberately — `patch.ts` reuses the identical message
/// for both).
pub fn stringifiable_field(payload: &Value, key: &str) -> PatchResult<String> {
    match payload.get(key) {
        Some(Value::String(value)) if !value.is_empty() => Ok(value.clone()),
        Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => Ok(n.to_string()),
        _ => Err(format!("patch payload requires string field: {key}")),
    }
}

/// An optional string field, guarded by `typeof === "string"` in `patch.ts`
/// (no non-empty requirement) — returns `None` when absent or malformed.
pub fn optional_string_field(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// An optional string field defaulting to `default` when absent/malformed —
/// mirrors `patch.ts`'s `typeof payload.x === "string" ? payload.x :
/// default` guard shape (used for `source`, `place_node_id`, ...).
pub fn string_field_or(payload: &Value, key: &str, default: &str) -> String {
    optional_string_field(payload, key).unwrap_or_else(|| default.to_owned())
}

/// An optional finite-number field, guarded by `typeof === "number"` in
/// `patch.ts` — `None` when absent or malformed. Carries the original tagged
/// `Number` (see module doc).
pub fn optional_number_field(payload: &Value, key: &str) -> Option<Value> {
    match payload.get(key) {
        Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => {
            Some(Value::Number(n.clone()))
        }
        _ => None,
    }
}

/// An optional boolean field, guarded by `typeof === "boolean"` in `patch.ts`.
pub fn optional_bool_field(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

/// An optional u64-carrying field (H3 cell ids): accepts either a plain JSON
/// number or a decimal-digit JSON string, passed through VERBATIM (never
/// coerced — coercing a big digit string to `number` would defeat the whole
/// #1968 precision fix). Returns `None` when the field is absent or neither
/// shape matches. Mirrors `patch.ts`'s `optionalU64ishField`.
pub fn optional_u64ish_field(payload: &Value, key: &str) -> Option<Value> {
    match payload.get(key) {
        Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => {
            Some(Value::Number(n.clone()))
        }
        Some(Value::String(s)) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
            Some(Value::String(s.clone()))
        }
        _ => None,
    }
}

/// Builds a JSON object row from an explicit, ordered field list. Used
/// instead of the `json!` macro so every field's exact `Value` (string,
/// tagged `Number`, bool) is placed with no re-serialization hop.
pub fn row(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in fields {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

/// A convenience wrapper for `row()`'s string-valued fields.
pub fn s(value: impl Into<String>) -> Value {
    Value::String(value.into())
}

/// Builds the `{"<table>": [...], ...}` output object this crate's ops
/// return, from a list of `(table_name, rows)` pairs.
pub fn patched_tables(tables: impl IntoIterator<Item = (&'static str, Vec<Value>)>) -> Value {
    let mut map = Map::new();
    for (table, rows) in tables {
        map.insert(table.to_owned(), Value::Array(rows));
    }
    Value::Object(map)
}

/// One field's value for `ordered_compact_json`: either a plain
/// scalar/array/object serialized via `serde_json` (`Scalar`), or a nested
/// object whose OWN key order also matters (`Nested`) — built via a
/// recursive `ordered_compact_json` call and embedded RAW (never
/// re-escaped/re-parsed), so a multi-level `JSON.stringify({a, b: {c, d}})`
/// ports with EVERY level's insertion order preserved, not just the
/// outermost.
pub enum OrderedField {
    Scalar(Value),
    Nested(String),
}

/// A compact JSON object string with fields in EXACTLY the given order,
/// matching JS's `JSON.stringify({...})` (no `preserve_order` normalization,
/// no spaces). `row()`'s `Value::Object` is a `serde_json::Map` — with this
/// workspace's `preserve_order` feature OFF (a workspace-wide, deliberate
/// choice, see `spellkave-validate-wasm`'s own doc comment), that Map is
/// BTreeMap-backed, so serializing a `row()` value ALWAYS emits keys
/// alphabetically, never in insertion order. That is harmless for fields the
/// renderer treats as STRUCTURED (`spellkave-format::render_manifest` itself
/// re-serializes through the same non-preserve_order path, so any insertion
/// order is normalized away identically on both the old-TS and new-Rust
/// side). It is NOT harmless for a `String`-typed schema field whose VALUE
/// is itself `JSON.stringify`'d text (`payload_json`, `parameters_schema_json`
/// — `crates/spellkave-schema`) — the renderer treats those as opaque
/// strings and never re-normalizes their content, so the original TS
/// insertion order must be reproduced byte-for-byte, at EVERY nesting level
/// (a nested `Value::Object` built via `row()` would still serialize its OWN
/// keys alphabetically — use `OrderedField::Nested` with a recursive call
/// for a nested object whose order matters, never a bare `row()` value).
/// Use this helper (never `row()` + `serde_json::to_string`) whenever
/// porting a `JSON.stringify` call whose result becomes one of those
/// fields' value.
pub fn ordered_compact_json(fields: &[(&str, OrderedField)]) -> String {
    let mut out = String::from("{");
    for (i, (key, field)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(key).unwrap_or_default());
        out.push(':');
        match field {
            OrderedField::Scalar(value) => {
                out.push_str(&serde_json::to_string(value).unwrap_or_default())
            }
            OrderedField::Nested(json) => out.push_str(json),
        }
    }
    out.push('}');
    out
}
