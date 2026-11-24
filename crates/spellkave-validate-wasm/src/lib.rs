use wasm_bindgen::prelude::*;

pub mod dm_carrier;
pub mod eval_bench;

#[wasm_bindgen]
pub fn evaluate_dsl_expression_json(
    source: &str,
    kind: &str,
    facts_json: &str,
    dice_json: &str,
    manifest_toml: Option<String>,
) -> Result<String, JsValue> {
    eval_bench::evaluate_dsl_expression_json_core(
        source,
        kind,
        facts_json,
        dice_json,
        manifest_toml.as_deref(),
    )
    .map_err(|err| JsValue::from_str(&err))
}

pub fn evaluate_dsl_expression_json_core(
    source: &str,
    kind: &str,
    facts_json: &str,
    dice_json: &str,
    manifest_toml: Option<&str>,
) -> Result<String, String> {
    eval_bench::evaluate_dsl_expression_json_core(
        source,
        kind,
        facts_json,
        dice_json,
        manifest_toml,
    )
}

#[wasm_bindgen]
pub fn manifest_schema_json() -> String {
    spellkave_schema::manifest_json_schema_pretty().unwrap_or_else(|_| "{}".to_owned())
}

#[wasm_bindgen]
pub fn dsl_reference_json() -> String {
    serde_json::to_string_pretty(&spellkave_dsl::dsl_reference())
        .unwrap_or_else(|_| "{\"operators\":[]}".to_owned())
}

#[wasm_bindgen]
pub fn tokenize_dsl_json(source: &str) -> Result<String, JsValue> {
    tokenize_dsl_json_core(source).map_err(|err| JsValue::from_str(&err))
}

pub fn tokenize_dsl_json_core(source: &str) -> Result<String, String> {
    let tokens = spellkave_dsl::tokenize(source).map_err(|err| {
        err.diagnostics
            .into_iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let json_tokens: Vec<serde_json::Value> = tokens
        .iter()
        .map(|token| {
            serde_json::json!({
                "kind": token_kind_name(&token.kind),
                "start": token.span.start,
                "end": token.span.end,
            })
        })
        .collect();
    serde_json::to_string(&serde_json::json!({ "tokens": json_tokens }))
        .map_err(|err| format!("tokenize JSON serialization failed: {err}"))
}

fn token_kind_name(kind: &spellkave_dsl::TokenKind) -> &'static str {
    use spellkave_dsl::TokenKind::*;
    match kind {
        Ident(_) => "Ident",
        StringLit(_) => "StringLit",
        Number(_) => "Number",
        Bool(_) => "Bool",
        Whitespace => "Whitespace",
        AndAnd => "AndAnd",
        OrOr => "OrOr",
        Dot => "Dot",
        Dollar => "Dollar",
        LParen => "LParen",
        RParen => "RParen",
        Comma => "Comma",
        Equal => "Equal",
        Semicolon => "Semicolon",
        Minus => "Minus",
        Plus => "Plus",
        Star => "Star",
        Slash => "Slash",
        EqEq => "EqEq",
        Ne => "Ne",
        Lt => "Lt",
        Lte => "Lte",
        Gt => "Gt",
        Ge => "Ge",
        Eof => "Eof",
    }
}

/// #1968 B-9 (canon-warden Phase-2 condition 3): the FROZEN closed set of
/// presentation-layer placeholder tokens an authored `outcome_template.description`
/// may reference. `spellkave_validate::OBSERVATION_PLACEHOLDER_TOKENS` is the
/// single source; TS callers (the shared tool runtime `observation.ts` resolver)
/// import THIS export rather than hand-mirroring the list, so the two sides
/// can never drift.
#[wasm_bindgen]
pub fn observation_placeholder_tokens_json() -> String {
    serde_json::to_string(spellkave_validate::OBSERVATION_PLACEHOLDER_TOKENS)
        .unwrap_or_else(|_| "[]".to_owned())
}

/// sk-jay0.3.8: the admitted `metadata_attachments.target_kind` values.
/// `spellkave_validate::METADATA_TARGET_KINDS` is the single source (itself
/// pinned to the server's `TARGET_KINDS` by a drift-guard test there); TS
/// callers compare against THIS export rather than trusting their own
/// hand-written copy, so a kind added on one side cannot sit unnoticed on the
/// other. Same shape as [`observation_placeholder_tokens_json`].
#[wasm_bindgen]
pub fn metadata_target_kinds_json() -> String {
    serde_json::to_string(spellkave_validate::METADATA_TARGET_KINDS)
        .unwrap_or_else(|_| "[]".to_owned())
}

/// Platform document admission; does not make metadata a kernel input.
#[wasm_bindgen]
pub fn validate_npc_quest_source_json(source_json: &str) -> String {
    serde_json::to_string(
        &spellkave_validate::npc_documents::validate_quest_source_json(source_json),
    )
    .expect("NPC validation diagnostics serialize")
}

/// Shared DM runtime table admission; does not admit the privileged carrier.
#[wasm_bindgen]
pub fn validate_dm_srd_tables_json(tables_json: &str) -> String {
    serde_json::to_string(&spellkave_validate::dm_srd::validate_tables_json(
        tables_json,
    ))
    .expect("DM table diagnostics serialize")
}

/// Presentation component only: no grants, publication or mounted-engine admission.
#[wasm_bindgen]
pub fn validate_dm_presentation_json(input: &str) -> String {
    serde_json::to_string(&spellkave_validate::dm_presentation::validate_presentation_json(input))
        .expect("DM presentation diagnostics serialize")
}

/// Canonical compile/publish reference closure, before metadata publication.
#[wasm_bindgen]
pub fn validate_npc_quest_references_json(source_json: &str, worlds_json: &str) -> String {
    serde_json::to_string(
        &spellkave_validate::npc_documents::validate_quest_references_json(
            source_json,
            worlds_json,
        ),
    )
    .expect("reference diagnostics serialize")
}

#[wasm_bindgen]
pub fn parse_manifest_toml_to_json(toml_source: &str) -> Result<String, JsValue> {
    parse_manifest_toml_to_json_core(toml_source).map_err(|err| JsValue::from_str(&err))
}

pub fn parse_manifest_toml_to_json_core(toml_source: &str) -> Result<String, String> {
    let parsed = spellkave_format::parse_manifest(toml_source)
        .map_err(|err| format!("manifest TOML parse failed: {err}"))?;
    let json = serde_json::to_string_pretty(&parsed.manifest)
        .map_err(|err| format!("manifest JSON serialization failed: {err}"))?;
    Ok(stringify_u64_fields(&json))
}

/// Split one direct include into authoring metadata and canonical row TOML.
/// Gameplay integers remain native Rust/TOML values throughout; they never
/// cross JavaScript's lossy number representation. Metadata-free legacy
/// fragments retain their exact source bytes. Metadata-bearing fragments are
/// canonical-rendered because their metadata table must be removed.
#[wasm_bindgen]
pub fn split_manifest_fragment(toml_source: &str) -> Result<String, JsValue> {
    split_manifest_fragment_core(toml_source).map_err(|err| JsValue::from_str(&err))
}

pub fn split_manifest_fragment_core(toml_source: &str) -> Result<String, String> {
    let split = spellkave_format::split_manifest_fragment_source(toml_source)
        .map_err(|err| format!("manifest fragment TOML parse failed: {err}"))?;
    if split.has_includes {
        return Err("nested manifest includes are not supported".to_owned());
    }
    let diagnostics = spellkave_validate::validate_manifest_metadata(&split.metadata);
    if !diagnostics.is_empty() {
        return Err(format!(
            "manifest fragment metadata validation failed: {}",
            spellkave_validate::diagnostics_to_canonical_json(&diagnostics)
                .unwrap_or_else(|_| "[]".to_owned())
        ));
    }
    serde_json::to_string(&serde_json::json!({
        "metadata": split.metadata,
        "rowToml": split.row_source,
    }))
    .map_err(|err| format!("manifest fragment JSON serialization failed: {err}"))
}

/// Render a complete manifest as a direct-include document. Bundle identity
/// and include directives remain root-owned; callers choose whether the
/// include retains its authoring-support metadata.
#[wasm_bindgen]
pub fn render_manifest_include(
    toml_source: &str,
    retain_metadata: bool,
) -> Result<String, JsValue> {
    render_manifest_include_core(toml_source, retain_metadata)
        .map_err(|err| JsValue::from_str(&err))
}

pub fn render_manifest_include_core(
    toml_source: &str,
    retain_metadata: bool,
) -> Result<String, String> {
    let mut manifest = spellkave_format::parse_manifest(toml_source)
        .map_err(|err| format!("manifest TOML parse failed: {err}"))?
        .manifest;
    manifest.bundles.clear();
    manifest.includes.clear();
    if !retain_metadata {
        manifest.metadata.clear();
    }
    render_headerless_manifest(&manifest)
}

fn render_headerless_manifest(manifest: &spellkave_schema::Manifest) -> Result<String, String> {
    let rendered = spellkave_format::render_manifest(manifest)
        .map_err(|err| format!("manifest fragment TOML render failed: {err}"))?;
    let header = format!(
        "#:schema {}\nschema_version = \"1.0.0\"\n",
        spellkave_format::WORLD_MANIFEST_SCHEMA_URL
    );
    rendered
        .strip_prefix(&header)
        .map(str::to_owned)
        .ok_or_else(|| "rendered manifest fragment lost its canonical schema header".to_owned())
}

/// Rewrites every bare JSON number at a known u64-carrying manifest field
/// (`spellkave_schema::U64_FIELDS_BY_ROW_KIND`) to a quoted string of its
/// exact decimal digits — the ONE place this crate's JSON output stops being
/// a plain re-serialization of `Manifest` (whose own `Serialize` impl is
/// untouched, see `wire_u64` in `spellkave-schema`). This is the actual fix
/// for #1968's u64-precision-loss backlog item: a JS caller that
/// `JSON.parse`s this string now sees a digit string, not a rounded float64,
/// for every u64 field — not just H3 cell ids. Never applied to
/// `spellkave-format`'s TOML render path, so the committed TOML's native
/// integers are untouched (the binding invariant).
///
/// Deliberately a TEXT rewrite, not a `serde_json::Value` walk: this crate's
/// `serde_json` dependency does NOT enable `preserve_order` (turning that on
/// would flip `Value::Object`'s iteration from insertion order to `BTreeMap`
/// alphabetical order — and Cargo feature unification would carry that into
/// EVERY crate sharing this `serde_json` version across the `crates/`
/// workspace, including `spellkave-format::render_manifest`'s own
/// `to_value(manifest)` hop, silently reordering every committed TOML row.
/// A round trip through `Value` would need it to know which numbers are u64;
/// operating on the already-pretty-printed text avoids the whole problem).
///
/// Matches on the field's exact `"<name>": ` prefix (the fixed separator
/// `serde_json::to_string_pretty` always emits) followed immediately by an
/// unquoted digit run. A String-typed field sharing the same name elsewhere
/// in the schema (e.g. `modifier_target_definitions.target_id: String` vs
/// `target_conditions.target_id: u64`) is structurally safe: its value is
/// always quoted in the source JSON, so it never matches "digit run right
/// after the prefix" and is left untouched. An absent `Option<u64>` renders
/// as `null`, which likewise has no leading digit and is left untouched.
fn stringify_u64_fields(json: &str) -> String {
    let mut fields: Vec<&str> = spellkave_schema::U64_FIELDS_BY_ROW_KIND
        .iter()
        .flat_map(|(_, fields)| fields.iter().copied())
        .collect();
    fields.sort_unstable();
    fields.dedup();

    let mut out = json.to_owned();
    for field in fields {
        out = quote_bare_digits_after_field(&out, field);
    }
    out
}

/// Single-field pass for [`stringify_u64_fields`]: replaces every `"field":
/// 123` with `"field": "123"` in `json`, leaving every other byte (including
/// `null` and already-quoted values) untouched.
fn quote_bare_digits_after_field(json: &str, field: &str) -> String {
    let needle = format!("\"{field}\": ");
    let mut out = String::with_capacity(json.len());
    let mut remaining = json;
    while let Some(pos) = remaining.find(needle.as_str()) {
        let match_end = pos + needle.len();
        out.push_str(&remaining[..match_end]);
        let after = &remaining[match_end..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            out.push('"');
            out.push_str(&after[..digits]);
            out.push('"');
        }
        remaining = &after[digits..];
    }
    out.push_str(remaining);
    out
}

#[wasm_bindgen]
pub fn render_manifest_json_to_toml(manifest_json: &str) -> Result<String, JsValue> {
    render_manifest_json_to_toml_core(manifest_json).map_err(|err| JsValue::from_str(&err))
}

pub fn render_manifest_json_to_toml_core(manifest_json: &str) -> Result<String, String> {
    let manifest: spellkave_schema::Manifest = serde_json::from_str(manifest_json)
        .map_err(|err| format!("manifest JSON parse failed: {err}"))?;
    spellkave_format::render_manifest(&manifest)
        .map_err(|err| format!("manifest TOML render failed: {err}"))
}

#[wasm_bindgen]
pub fn validate_schema_version(version: &str) -> String {
    let diagnostics = spellkave_validate::validate_manifest_version(version);
    spellkave_validate::diagnostics_to_canonical_json(&diagnostics)
        .unwrap_or_else(|_| "[]".to_owned())
}

#[wasm_bindgen]
pub fn validate_manifest_json(manifest_json: &str) -> String {
    let manifest: spellkave_schema::Manifest = match serde_json::from_str(manifest_json) {
        Ok(manifest) => manifest,
        Err(err) => {
            let diagnostics = vec![spellkave_validate::Diagnostic {
                code: spellkave_validate::diagnostic_codes::MF_VAL_VALIDATION_INPUT.to_owned(),
                severity: spellkave_validate::Severity::Fatal,
                message: "manifest json parse failed".to_owned(),
                path: "$".to_owned(),
                span: None,
                hint: None,
                details: Some(serde_json::json!({ "error": err.to_string() })),
            }];
            return spellkave_validate::diagnostics_to_canonical_json(&diagnostics)
                .unwrap_or_else(|_| "[]".to_owned());
        }
    };
    let diagnostics = spellkave_validate::validate_manifest(&manifest);
    spellkave_validate::diagnostics_to_canonical_json(&diagnostics)
        .unwrap_or_else(|_| "[]".to_owned())
}

/// Validate a manifest from its TOML source — the canonical authoring path.
///
/// Unlike `validate_manifest_json` (which takes the post-parse `Manifest` JSON),
/// this mirrors the CLI `spellkave validate` / the server reducer: parse the TOML
/// via `spellkave-format` (the same parser `r_apply_world_manifest` uses), then
/// run the shared `spellkave-validate` core. This lets a TS authoring SDK validate
/// exactly the TOML it renders, over the identical code path the server admits —
/// so "validator accepts" means "the server would admit it" (minus live-state
/// cross-bundle checks deferred to admission). Returns canonical diagnostics JSON.
#[wasm_bindgen]
pub fn validate_manifest_toml(toml_source: &str) -> String {
    validate_toml(toml_source, None)
}

/// Dependency rows supply existence/ownership facts only. Their DSL is not
/// reconstructed or revalidated, and they never become authored output.
#[wasm_bindgen]
pub fn validate_manifest_toml_with_containment_context(
    toml_source: &str,
    context_json: &str,
) -> String {
    let context: spellkave_validate::containment_policy::ReferenceContext =
        match serde_json::from_str(context_json) {
            Ok(context) => context,
            Err(error) => {
                return single_fatal(
                    spellkave_validate::diagnostic_codes::MF_VAL_VALIDATION_INPUT,
                    format!("containment context: {error}"),
                )
            }
        };
    let vocabulary = context.into_vocabulary();
    validate_toml(toml_source, Some(&vocabulary))
}

fn validate_toml(
    toml_source: &str,
    context: Option<&spellkave_validate::LiveVocabulary>,
) -> String {
    let parsed = match spellkave_format::parse_manifest(toml_source) {
        Ok(parsed) => parsed,
        Err(err) => {
            let (code, message) = match err {
                spellkave_format::ParseError::MissingSchemaDirective => (
                    spellkave_validate::MF_FMT_MISSING_SCHEMA_DIRECTIVE,
                    "missing schema directive".to_owned(),
                ),
                spellkave_format::ParseError::Toml(ref e) => (
                    spellkave_validate::diagnostic_codes::MF_FMT_MANIFEST_PARSE,
                    format!("manifest parse: {e}"),
                ),
                spellkave_format::ParseError::UnsupportedMetadataValue(ref e) => (
                    spellkave_validate::diagnostic_codes::MF_FMT_MANIFEST_PARSE,
                    format!("manifest parse: {e}"),
                ),
                spellkave_format::ParseError::FragmentApplyset => (
                    spellkave_validate::diagnostic_codes::MF_FMT_MANIFEST_PARSE,
                    format!("manifest parse: {err}"),
                ),
            };
            return single_fatal(code, message);
        }
    };
    let mut diagnostics =
        spellkave_validate::validate_manifest_version(&parsed.manifest.schema_version.to_string());
    diagnostics.extend(match context {
        Some(context) => spellkave_validate::validate_manifest_with_containment_context(
            &parsed.manifest,
            context,
        ),
        None => spellkave_validate::validate_manifest(&parsed.manifest),
    });
    spellkave_validate::diagnostics_to_canonical_json(&diagnostics)
        .unwrap_or_else(|_| "[]".to_owned())
}

/// One fatal diagnostic as canonical JSON — the parse-failure shape shared by the
/// JSON and TOML entry points.
fn single_fatal(code: &str, message: String) -> String {
    let diagnostics = vec![spellkave_validate::Diagnostic {
        code: code.to_owned(),
        severity: spellkave_validate::Severity::Fatal,
        message,
        path: "$".to_owned(),
        span: None,
        hint: None,
        details: None,
    }];
    spellkave_validate::diagnostics_to_canonical_json(&diagnostics)
        .unwrap_or_else(|_| "[]".to_owned())
}

/// The exact inline kernel vocabulary; definitions alone grant no authority.
#[wasm_bindgen]
pub fn core_bootstrap_manifest() -> String {
    spellkave_schema::CORE_BOOTSTRAP_MANIFEST.to_owned()
}

/// Public extent vocabulary projected using the existing schema string shape.
#[wasm_bindgen]
pub fn core_extent_kinds_json() -> String {
    serde_json::to_string(
        &spellkave_schema::CORE_EXTENT_KINDS
            .iter()
            .map(|seed| spellkave_schema::ExtentKindDefinitionRow {
                kind_id: seed.kind_id.to_owned(),
                resolution_shape: seed.resolution_shape.to_owned(),
                bundle_id: "core".to_owned(),
                description: seed.description.to_owned(),
            })
            .collect::<Vec<_>>(),
    )
    .expect("core extent rows serialize")
}

/// All readers use the validator's sorted, deduplicated role union.
#[wasm_bindgen]
pub fn merged_role_capability_ids_json(
    role_id: &str,
    definitions_json: &str,
    aggregations_json: &str,
) -> Result<String, JsValue> {
    let definitions: Vec<spellkave_schema::RoleDefinitionRow> =
        serde_json::from_str(definitions_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let aggregations: Vec<spellkave_schema::RoleAggregationRow> =
        serde_json::from_str(aggregations_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    serde_json::to_string(&spellkave_validate::merged_role_capability_ids(
        role_id,
        &definitions,
        &aggregations,
    ))
    .map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_fixture(path: &[&str]) -> String {
        let mut fixture = PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR")
                .expect("cargo test must supply the current package directory"),
        );
        fixture.push("..");
        fixture.push("..");
        for part in path {
            fixture.push(part);
        }
        std::fs::read_to_string(&fixture)
            .unwrap_or_else(|err| panic!("fixture {} should load: {err}", fixture.display()))
    }

    #[test]
    fn wrapper_uses_shared_validation_core() {
        let manifest = r#"{
            "schema_version":"1.0.0",
            "metadata":{
                "nodes":"bad",
                "location_parents":"town->inn,inn->town"
            }
        }"#;
        let output = validate_manifest_json(manifest);
        assert!(output.contains("MF-VAL-104"));
        assert!(output.contains("MF-VAL-108"));
    }

    #[test]
    fn toml_entry_point_parses_then_validates() {
        // A minimal well-formed manifest validates clean (no diagnostics).
        let ok = validate_manifest_toml(
            "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = \"1.0.0\"\n",
        );
        assert_eq!(ok, "[]");
        // Missing the schema directive is a parse-stage finding.
        let missing = validate_manifest_toml("schema_version = \"1.0.0\"\n");
        assert!(missing.contains("MF-FMT-001"));
    }

    /// sk-jay0.3.8: the wasm export is what TS compares against, so it must
    /// carry the native table exactly — not a stale snapshot of it. Mirrors
    /// `row_kind_natural_key_json_matches_native_table` in
    /// `spellkave-authoring-wasm`.
    #[test]
    fn metadata_target_kinds_json_matches_native_table() {
        let kinds: Vec<String> =
            serde_json::from_str(&metadata_target_kinds_json()).expect("valid JSON array");
        assert_eq!(
            kinds,
            spellkave_validate::METADATA_TARGET_KINDS
                .iter()
                .map(|kind| (*kind).to_owned())
                .collect::<Vec<String>>()
        );
    }

    #[test]
    fn reference_bindings_return_structured_json() {
        let schema: serde_json::Value =
            serde_json::from_str(&manifest_schema_json()).expect("schema JSON should parse");
        assert_eq!(schema["title"], "Manifest");

        let dsl: serde_json::Value =
            serde_json::from_str(&dsl_reference_json()).expect("DSL JSON should parse");
        let operators = dsl["operators"]
            .as_array()
            .expect("operators should be an array");
        assert!(operators
            .iter()
            .any(|op| op["operator"].as_str() == Some("HasTag")));
    }

    #[test]
    fn tokenize_dsl_json_core_covers_source_with_whitespace() {
        let json = tokenize_dsl_json_core("a && b").expect("tokenize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("json");
        let tokens = value["tokens"].as_array().expect("tokens array");
        assert_eq!(tokens[0]["kind"], "Ident");
        assert_eq!(tokens[1]["kind"], "Whitespace");
        assert_eq!(tokens[2]["kind"], "AndAnd");
        assert_eq!(tokens[0]["start"], 0);
        assert_eq!(tokens[0]["end"], 1);
        assert!(tokenize_dsl_json_core("'unterminated")
            .unwrap_err()
            .contains("MF-DSL-UNTERMINATED-STRING"));
    }

    // This walkthrough imports ordinary capabilities from core. Its round-trip
    // bytes stay unchanged; offline validation receives the actual owning
    // bundle and capability definitions, rather than inventing local copies.
    fn actors_and_bodies_validation_closure(
        mut manifest: spellkave_schema::Manifest,
    ) -> spellkave_schema::Manifest {
        let core = spellkave_format::parse_manifest(&repo_fixture(&[
            "packages/spellkave-mechanics/fixtures/core/manifest.toml",
        ]))
        .expect("core owner manifest should parse")
        .manifest;
        let definitions = spellkave_format::parse_manifest_fragment(&repo_fixture(&[
            "packages/spellkave-mechanics/fixtures/core/base/affordances.toml",
        ]))
        .expect("core capability source should parse");
        manifest.bundles.extend(core.bundles);
        manifest
            .capability_definitions
            .extend(definitions.capability_definitions);
        manifest
    }

    #[test]
    fn actors_and_bodies_requires_actual_capability_dependency_closure() {
        let manifest = spellkave_format::parse_manifest(&repo_fixture(&[
            "crates/spellkave-validate-wasm/testdata/actors_and_bodies.toml",
        ]))
        .expect("walkthrough should parse")
        .manifest;
        let unresolved = |manifest: &spellkave_schema::Manifest| {
            spellkave_validate::validate_manifest(manifest)
                .into_iter()
                .filter(|d| d.code == spellkave_validate::MF_VAL_ACTOR_CAPABILITY_REF)
                .count()
        };
        assert_eq!(unresolved(&manifest), 3);
        let mut closed = actors_and_bodies_validation_closure(manifest);
        assert_eq!(unresolved(&closed), 0);
        let mut outside_scope = closed.clone();
        // Core is an implicit baseline in the existing resolver. Changing
        // only depends_on would not make core out of scope. Give these exact
        // IDs a different, unrelated owner to exercise ownership rejection.
        let mut unrelated_owner = outside_scope
            .bundles
            .iter()
            .find(|b| b.bundle_id == "core")
            .expect("core owner bundle")
            .clone();
        unrelated_owner.bundle_id = "unrelated_owner".into();
        outside_scope.bundles.push(unrelated_owner);
        for definition in &mut outside_scope.capability_definitions {
            definition.bundle_id = "unrelated_owner".into();
        }
        assert_eq!(unresolved(&outside_scope), 3);
        closed
            .capability_definitions
            .retain(|d| d.capability_id != "core/sense");
        assert_eq!(unresolved(&closed), 1);
    }

    #[test]
    fn parse_render_round_trip_preserves_typed_rows() {
        for path in [
            &[
                "crates",
                "spellkave-format",
                "tests",
                "fixtures",
                "valid_manifest.toml",
            ][..],
            &[
                "packages/spellkave-mechanics",
                "fixtures",
                "core",
                "core_bootstrap.toml",
            ][..],
            &[
                "crates/spellkave-validate-wasm",
                "testdata",
                "actors_and_bodies.toml",
            ][..],
        ] {
            let source = repo_fixture(path);
            let manifest_json =
                parse_manifest_toml_to_json_core(&source).expect("source fixture should parse");
            let rendered = render_manifest_json_to_toml_core(&manifest_json)
                .expect("typed manifest should render");
            // Staged-advisory tag-grammar warnings (MF-VAL-296..298, the
            // tag-vocabulary-declaration change) are expected on legacy
            // fixtures until their burn-down; round-trip only requires the
            // rendered manifest to stay clean of Fatal diagnostics.
            let validation_source = if path.last() == Some(&"actors_and_bodies.toml") {
                let manifest = spellkave_format::parse_manifest(&rendered)
                    .expect("rendered walkthrough should parse")
                    .manifest;
                spellkave_format::render_manifest(&actors_and_bodies_validation_closure(manifest))
                    .expect("dependency-closed validation input should render")
            } else {
                rendered.clone()
            };
            let diagnostics: serde_json::Value =
                serde_json::from_str(&validate_manifest_toml(&validation_source))
                    .expect("diagnostics JSON should parse");
            let fatals: Vec<&serde_json::Value> = diagnostics
                .as_array()
                .expect("diagnostics should be an array")
                .iter()
                .filter(|d| d["severity"] == "fatal")
                .collect();
            assert_eq!(fatals, Vec::<&serde_json::Value>::new());

            let reparsed_json =
                parse_manifest_toml_to_json_core(&rendered).expect("rendered fixture should parse");
            let original: serde_json::Value =
                serde_json::from_str(&manifest_json).expect("original JSON should parse");
            let reparsed: serde_json::Value =
                serde_json::from_str(&reparsed_json).expect("reparsed JSON should parse");
            assert_eq!(reparsed, original);
        }
    }

    #[test]
    fn structured_metadata_round_trips_through_wasm_json_without_number_loss() {
        let source = r#"#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json
schema_version = "1.0.0"

[metadata]
title = "Example"
rating = 4
weight = 0.5
tags = ["forest", "ruin"]

[metadata.provenance]
license = "CC-BY-4.0"
"#;
        let json = parse_manifest_toml_to_json_core(source).expect("metadata TOML should parse");
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("bridge JSON should parse");
        assert_eq!(
            value["metadata"]["tags"],
            serde_json::json!(["forest", "ruin"])
        );
        assert_eq!(value["metadata"]["provenance"]["license"], "CC-BY-4.0");
        let rendered = render_manifest_json_to_toml_core(&json).expect("bridge JSON should render");
        let reparsed =
            parse_manifest_toml_to_json_core(&rendered).expect("rendered TOML should parse");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&reparsed).unwrap(),
            value
        );
    }

    #[test]
    fn fragment_split_preserves_rows_and_large_integers_outside_js() {
        let source = r#"[metadata]
title = "Abilities"

[[bundles]]
bundle_id = "demo"
display_name = "Demo"
author = "Spellkave"
version = "1.0.0"

[[resource_definitions]]
resource_definition_id = "demo/value"
bundle_id = "demo"
display_name = "Value"
value_kind = "integer"
min_value = 0
max_value = 9007199254740993
recovery_rule = "persistent"
"#;
        let split = split_manifest_fragment_core(source).expect("fragment should split");
        let envelope: serde_json::Value = serde_json::from_str(&split).unwrap();
        assert_eq!(envelope["metadata"]["title"], "Abilities");
        let rows = envelope["rowToml"].as_str().expect("rowToml string");
        assert!(rows.contains("[[bundles]]"));
        assert!(rows.contains("max_value = 9007199254740993"));
        assert!(!rows.contains("[metadata]"));
    }

    #[test]
    fn fragment_split_rejects_nested_includes_and_invalid_metadata() {
        assert!(split_manifest_fragment_core("[[include]]\npath = \"nested.toml\"\n").is_err());
        let invalid = split_manifest_fragment_core("[metadata]\nprofile = [\"invalid\"]\n")
            .expect_err("reserved structured metadata should reject");
        assert!(invalid.contains("MF-VAL-362"));
    }

    #[test]
    fn metadata_free_fragment_split_preserves_exact_source_bytes() {
        let source = "# keep this comment\n\n[[bundles]]\nbundle_id = \"demo\"\n\
            display_name = \"Demo\"\nauthor = \"Spellkave\"\nversion = \"1.0.0\"\n";
        let split = split_manifest_fragment_core(source).expect("fragment should split");
        let envelope: serde_json::Value = serde_json::from_str(&split).unwrap();
        assert_eq!(envelope["rowToml"], source);
    }

    #[test]
    fn metadata_free_partial_nested_table_remains_valid_in_context() {
        let source = "[[bundles.tag_prefixes]]\nprefix = \"srd\"\ntarget_kind = \"resource_definitions\"\nnatural_key_field = \"resource_definition_id\"\n";
        let split = split_manifest_fragment_core(source).expect("partial include should split");
        let envelope: serde_json::Value = serde_json::from_str(&split).unwrap();
        assert_eq!(envelope["rowToml"], source);
    }

    #[test]
    fn explicit_empty_fragment_metadata_is_removed_from_rows() {
        for source in ["[metadata]\n", "metadata = {}\n"] {
            let split = split_manifest_fragment_core(source).expect("fragment should split");
            let envelope: serde_json::Value = serde_json::from_str(&split).unwrap();
            assert_eq!(envelope["metadata"], serde_json::json!({}));
            assert!(!envelope["rowToml"].as_str().unwrap().contains("metadata"));
        }
    }

    #[test]
    fn complete_manifest_renders_as_include_with_optional_metadata() {
        let source = "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\n\
            schema_version = \"1.0.0\"\n[metadata]\ntitle = \"Abilities\"\n\
            [[bundles]]\nbundle_id = \"demo\"\ndisplay_name = \"Demo\"\nauthor = \"Spellkave\"\nversion = \"1.0.0\"\n\
            [[resource_definitions]]\nresource_definition_id = \"demo/value\"\nbundle_id = \"demo\"\n\
            display_name = \"Value\"\nvalue_kind = \"integer\"\nmin_value = 0\n\
            max_value = 9007199254740993\nrecovery_rule = \"persistent\"\n";
        let retained = render_manifest_include_core(source, true).expect("include should render");
        assert!(retained.contains("[metadata]"));
        assert!(retained.contains("max_value = 9007199254740993"));
        assert!(!retained.contains("[[bundles]]"));
        assert!(!retained.contains("schema_version"));

        let omitted = render_manifest_include_core(source, false).expect("include should render");
        assert!(!omitted.contains("[metadata]"));
        assert!(omitted.contains("[[resource_definitions]]"));
    }

    #[test]
    fn json_metadata_rejects_null_and_unsafe_numbers() {
        for metadata in [
            r#"{"note":null}"#,
            r#"{"catalogue_id":9007199254740992}"#,
            r#"{"weight":9007199254740992.0}"#,
        ] {
            let manifest = format!(r#"{{"schema_version":"1.0.0","metadata":{metadata}}}"#);
            assert!(
                render_manifest_json_to_toml_core(&manifest).is_err(),
                "metadata {metadata} must not silently render"
            );
        }
    }

    /// #1968 backlog item 2 — the regression this whole change exists to
    /// prevent. `above_2_pow_53` is NOT representable exactly as an f64 (JS's
    /// only number type): `above_2_pow_53 as f64 as u64 != above_2_pow_53`.
    /// Before this fix, `parse_manifest_toml_to_json` emitted a bare JSON
    /// number for `cell_res10`, so any JS `JSON.parse` would silently round
    /// it — the exact failure mode `BIG_INT_FIELD_OVERRIDES` used to paper
    /// over in TS. This test proves the wasm bridge itself now carries the
    /// value bit-exact, with NO override table anywhere in the picture: (1)
    /// the emitted JSON node is a `Value::String` of the exact digits (a
    /// JSON *number* is the only JSON shape a JS `JSON.parse` can corrupt —
    /// a string round-trips unchanged no matter its digit count), and (2)
    /// feeding that JSON straight back through `render_manifest_json_to_toml`
    /// reproduces the identical native TOML integer.
    #[test]
    fn u64_above_2_pow_53_round_trips_bit_exact_without_override_table() {
        let above_2_pow_53: u64 = 622_054_109_320_019_967;
        assert_ne!(
            above_2_pow_53, above_2_pow_53 as f64 as u64,
            "fixture value must actually exceed f64 precision, or this test proves nothing"
        );

        let toml_source = format!(
            "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\n\
             schema_version = \"1.0.0\"\n\n\
             [[place_nodes]]\n\
             node_id = \"village/green\"\n\
             bundle_id = \"core\"\n\
             place_graph_id = \"village\"\n\
             local_id = \"green\"\n\
             substrate_id = \"outdoor\"\n\
             display_name = \"Green\"\n\
             cell_res10 = {above_2_pow_53}\n"
        );

        let manifest_json =
            parse_manifest_toml_to_json_core(&toml_source).expect("fixture TOML should parse");
        let value: serde_json::Value =
            serde_json::from_str(&manifest_json).expect("bridge JSON should parse");
        let cell_res10 = &value["place_nodes"][0]["cell_res10"];
        assert_eq!(
            cell_res10,
            &serde_json::Value::String(above_2_pow_53.to_string()),
            "cell_res10 must cross the wasm/JS boundary as a JSON STRING of its exact digits, \
             never a bare JSON number — a JS JSON.parse would round the latter"
        );

        let rendered = render_manifest_json_to_toml_core(&manifest_json)
            .expect("stringified manifest JSON should render back to TOML");
        assert!(
            rendered.contains(&format!("cell_res10 = {above_2_pow_53}")),
            "rendered TOML must keep cell_res10 as a native (unquoted) integer with the exact \
             digits — got:\n{rendered}"
        );
    }
}
