use serde::Deserialize;
use spellkave_schema::{Manifest, MetadataValue};
use std::collections::BTreeMap;

const SCHEMA_DIRECTIVE_PREFIX: &str = "#:schema ";
pub const WORLD_MANIFEST_SCHEMA_URL: &str =
    "https://spellkave.dev/schema/world-manifest-1.0.0.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedManifest {
    pub manifest: Manifest,
    pub schema_directive: Option<String>,
    pub spans: BTreeMap<String, SourceSpan>,
}

#[derive(Debug)]
pub enum ParseError {
    Toml(toml::de::Error),
    MissingSchemaDirective,
    UnsupportedMetadataValue(String),
    /// ADR-0145 D-1: only a root manifest names its applyset; an included
    /// fragment that sets `applyset` would let a fragment rename the set.
    FragmentApplyset,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toml(err) => write!(f, "{err}"),
            Self::MissingSchemaDirective => write!(f, "missing schema directive"),
            Self::UnsupportedMetadataValue(err) => write!(f, "unsupported metadata value: {err}"),
            Self::FragmentApplyset => write!(
                f,
                "an included fragment must not declare `applyset`; only the root manifest names its applyset (ADR-0145)"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Deserialize)]
struct SchemaVersionProbe {
    schema_version: String,
}

pub fn parse_manifest(source: &str) -> Result<ParsedManifest, ParseError> {
    let schema_directive =
        parse_schema_directive(source).ok_or(ParseError::MissingSchemaDirective)?;
    let raw: toml::Value = toml::from_str(source).map_err(ParseError::Toml)?;
    if let Some(metadata) = raw.get("metadata") {
        validate_json_compatible_metadata(metadata, "metadata")?;
    }
    let manifest: Manifest = raw.try_into().map_err(ParseError::Toml)?;
    let spans = collect_spans(source);

    Ok(ParsedManifest {
        manifest,
        schema_directive: Some(schema_directive),
        spans,
    })
}

/// Parse an included manifest fragment through the same strict parser as a
/// root manifest. Fragments intentionally omit the root-only schema header;
/// the synthetic header makes metadata validation and duplicate-key handling
/// identical without promoting fragment metadata into runtime manifest state.
pub fn parse_manifest_fragment(source: &str) -> Result<Manifest, ParseError> {
    let source = format!(
        "{SCHEMA_DIRECTIVE_PREFIX}{WORLD_MANIFEST_SCHEMA_URL}\nschema_version = \"1.0.0\"\n{source}"
    );
    let manifest = parse_manifest(&source)?.manifest;
    if manifest.applyset.is_some() {
        return Err(ParseError::FragmentApplyset);
    }
    Ok(manifest)
}

/// Report whether a fragment explicitly authors a top-level `metadata` value.
/// This is structural TOML parsing: an empty table or inline table is present
/// even though both deserialize to an empty manifest metadata map.
pub fn manifest_fragment_has_metadata(source: &str) -> Result<bool, ParseError> {
    let raw: toml::Value = toml::from_str(source).map_err(ParseError::Toml)?;
    Ok(raw.get("metadata").is_some())
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplitManifestFragment {
    pub metadata: BTreeMap<String, MetadataValue>,
    pub row_source: String,
    pub has_includes: bool,
}

/// Split fragment-local metadata from otherwise context-dependent TOML.
///
/// Some legacy includes contain partial nested tables (for example only
/// `[[bundles.tag_prefixes]]`) that become valid when concatenated after the
/// root's parent row. Therefore this helper parses a generic TOML tree, not a
/// standalone `Manifest`. Metadata-free input is returned byte-for-byte.
pub fn split_manifest_fragment_source(source: &str) -> Result<SplitManifestFragment, ParseError> {
    let mut raw: toml::Value = toml::from_str(source).map_err(ParseError::Toml)?;
    let table = raw
        .as_table_mut()
        .expect("a TOML document always parses as a top-level table");
    if table.contains_key("applyset") {
        return Err(ParseError::FragmentApplyset);
    }
    let has_includes = match table.get("include") {
        None => false,
        Some(toml::Value::Array(values)) => !values.is_empty(),
        Some(_) => true,
    };
    let Some(metadata_value) = table.remove("metadata") else {
        return Ok(SplitManifestFragment {
            metadata: BTreeMap::new(),
            row_source: source.to_owned(),
            has_includes,
        });
    };
    validate_json_compatible_metadata(&metadata_value, "metadata")?;
    let metadata = metadata_value.try_into().map_err(ParseError::Toml)?;
    let row_source = toml::to_string_pretty(&raw).map_err(|err| {
        ParseError::UnsupportedMetadataValue(format!("fragment TOML render failed: {err}"))
    })?;
    Ok(SplitManifestFragment {
        metadata,
        row_source,
        has_includes,
    })
}

/// Append only row-array fields from an included fragment to a root manifest.
/// Root scalar/map state (notably `schema_version` and `metadata`) remains
/// authoritative, and loader directives are consumed rather than projected.
pub fn append_fragment_rows(root: &mut Manifest, fragments: impl IntoIterator<Item = Manifest>) {
    let mut root_json = serde_json::to_value(&*root).expect("Manifest should serialize");
    let root_fields = root_json
        .as_object_mut()
        .expect("Manifest should serialize as an object");
    for fragment in fragments {
        let fragment_json = serde_json::to_value(fragment).expect("Manifest should serialize");
        let fragment_fields = fragment_json
            .as_object()
            .expect("Manifest should serialize as an object");
        for (key, fragment_value) in fragment_fields {
            if key == "include" {
                continue;
            }
            let Some(fragment_rows) = fragment_value.as_array() else {
                continue;
            };
            let root_rows = root_fields
                // Empty optional arrays may be absent from the root's JSON.
                // Both operands are typed Manifests, so a fragment row array
                // can safely introduce its field into the root.
                .entry(key.clone())
                .or_insert_with(|| serde_json::Value::Array(Vec::new()))
                .as_array_mut()
                .expect("Manifest row field should serialize as an array");
            root_rows.extend(fragment_rows.iter().cloned());
        }
    }

    root_fields.insert("include".to_owned(), serde_json::Value::Array(Vec::new()));
    *root = serde_json::from_value(root_json).expect("merged Manifest should deserialize");
}

#[cfg(test)]
mod optional_fragment_tests {
    // K4: an optional typed row array can first appear in an include fragment.
    #[test]
    fn fragment_introduces_condition_extent_array_omitted_by_empty_root() {
        let mut root = super::Manifest::default();
        let fragment = super::parse_manifest_fragment(
            "[[condition_capability_extent_grants]]\nbundle_id='demo'\ncondition_definition_id='demo/bond'\ncapability_id='demo/reach'\nextent_kind_ref='party'\nextent_ref='demo.members'\n",
        ).unwrap();
        super::append_fragment_rows(&mut root, [fragment]);
        assert_eq!(root.condition_capability_extent_grants.len(), 1);
        assert_eq!(
            root.condition_capability_extent_grants[0].extent_ref,
            "demo.members"
        );
    }
}

fn validate_json_compatible_metadata(value: &toml::Value, path: &str) -> Result<(), ParseError> {
    match value {
        toml::Value::Integer(value)
            if value.unsigned_abs() > MetadataValue::MAX_SAFE_INTEGER as u64 =>
        {
            Err(ParseError::UnsupportedMetadataValue(format!(
                "{path}: integer {value} exceeds JavaScript's safe range ±{}",
                MetadataValue::MAX_SAFE_INTEGER
            )))
        }
        toml::Value::Float(value) if !value.is_finite() => Err(
            ParseError::UnsupportedMetadataValue(format!("{path}: floats must be finite")),
        ),
        toml::Value::Datetime(_) => Err(ParseError::UnsupportedMetadataValue(format!(
            "{path}: TOML datetimes are not JSON-compatible"
        ))),
        toml::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                validate_json_compatible_metadata(child, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        toml::Value::Table(values) => {
            for (key, child) in values {
                validate_json_compatible_metadata(child, &format!("{path}.{key}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn parse_schema_directive(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix(SCHEMA_DIRECTIVE_PREFIX)
            .map(|value| value.trim().to_owned())
    })
}

pub fn render_manifest(manifest: &Manifest) -> Result<String, toml::ser::Error> {
    let mut value = serde_json::to_value(manifest).expect("manifest should serialize");
    prune_json_nulls(&mut value);
    if let serde_json::Value::Object(fields) = &mut value {
        fields.retain(|_, value| match value {
            serde_json::Value::Array(values) => !values.is_empty(),
            serde_json::Value::Object(values) => !values.is_empty(),
            _ => true,
        });
    }
    let body = toml::to_string_pretty(&value)?;
    Ok(format!(
        "{SCHEMA_DIRECTIVE_PREFIX}{WORLD_MANIFEST_SCHEMA_URL}\n{body}"
    ))
}

fn prune_json_nulls(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            fields.retain(|_, child| !child.is_null());
            for child in fields.values_mut() {
                prune_json_nulls(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                prune_json_nulls(child);
            }
        }
        _ => {}
    }
}

fn collect_spans(source: &str) -> BTreeMap<String, SourceSpan> {
    let mut spans = BTreeMap::new();
    if let Ok(probe) = toml::from_str::<SchemaVersionProbe>(source) {
        if let Some(span) = find_key_value_span(source, "schema_version", &probe.schema_version) {
            spans.insert("schema_version".to_owned(), span);
        }
    }
    spans
}

fn find_key_value_span(source: &str, key: &str, value: &str) -> Option<SourceSpan> {
    source.lines().enumerate().find_map(|(idx, line)| {
        let key_pos = line.find(key)?;
        let value_pos = line.find(value)?;
        Some(SourceSpan {
            start_line: idx + 1,
            start_col: key_pos + 1,
            end_line: idx + 1,
            end_col: value_pos + value.len() + 1,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> String {
        let mut path = PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR")
                .expect("cargo test must supply the current package directory"),
        );
        path.push("tests");
        path.push("fixtures");
        path.push(name);
        std::fs::read_to_string(path).expect("fixture should load")
    }

    #[test]
    fn parses_schema_directive_and_schema_version_span() {
        let source = fixture("valid_manifest.toml");
        let parsed = parse_manifest(&source).expect("manifest should parse");
        assert_eq!(
            parsed.schema_directive.as_deref(),
            Some("https://spellkave.dev/schema/world-manifest-1.0.0.json")
        );
        assert_eq!(parsed.manifest.schema_version.to_string(), "1.0.0");
        assert!(parsed.spans.contains_key("schema_version"));
    }

    #[test]
    fn missing_directive_is_rejected() {
        let source = fixture("missing_directive.toml");
        let err = parse_manifest(&source).expect_err("directive is required");
        assert!(matches!(err, ParseError::MissingSchemaDirective));
    }

    #[test]
    fn render_manifest_round_trips_through_parser() {
        let source = fixture("valid_manifest.toml");
        let parsed = parse_manifest(&source).expect("manifest should parse");
        let rendered = render_manifest(&parsed.manifest).expect("manifest should render");
        let reparsed = parse_manifest(&rendered).expect("rendered manifest should parse");

        assert_eq!(reparsed.manifest, parsed.manifest);
    }

    #[test]
    fn metadata_attachment_rows_coexist_with_manifest_metadata_header() {
        // `[metadata]` (the open JSON-compatible map) and `[[metadata_attachments]]` are
        // distinct TOML keys, so the file stays valid for any spec-conforming
        // parser — no pre-parse rename pass.
        let source = r#"#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json
schema_version = "1.0.0"

[metadata]
profile = "test"

[[metadata_attachments]]
target_kind = "place_graph"
target_ref = "test/graph"
namespace = "editorial.note"
payload_json = '{"text":"hello"}'
bundle_id = "test"

[[metadata_namespaces]]
namespace = "editorial.note"
audience = "author"
bundle_id = "test"
"#;
        let parsed = parse_manifest(source).expect("manifest with metadata rows should parse");
        assert_eq!(
            parsed.manifest.metadata["profile"],
            MetadataValue::String("test".to_owned())
        );
        assert_eq!(parsed.manifest.metadata_namespaces.len(), 1);
        assert_eq!(parsed.manifest.metadata_attachments.len(), 1);
        assert_eq!(
            parsed.manifest.metadata_attachments[0].namespace,
            "editorial.note"
        );

        let rendered = render_manifest(&parsed.manifest).expect("manifest should render");
        assert!(rendered.contains("[metadata]\nprofile = \"test\""));
        assert!(rendered.contains("[[metadata_namespaces]]"));
        assert!(rendered.contains("[[metadata_attachments]]"));
        let reparsed = parse_manifest(&rendered).expect("rendered manifest should parse");
        assert_eq!(reparsed.manifest, parsed.manifest);

        // A generic TOML parser agrees — the file is spec-valid without help.
        let generic: toml::Value = toml::from_str(source).expect("plain TOML parser accepts it");
        assert!(generic.get("metadata_attachments").is_some());
    }

    #[test]
    fn structured_metadata_round_trips_without_losing_json_shape() {
        let source = r#"#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json
schema_version = "1.0.0"

[metadata]
title = "A small world"
published = true
rating = 4
weight = 0.5
tags = ["forest", "ruin"]

[metadata.provenance]
source = "example"
authors = ["Ada", "Bo"]
"#;
        let parsed = parse_manifest(source).expect("structured metadata should parse");
        assert_eq!(
            parsed.manifest.metadata["provenance"],
            MetadataValue::Object(BTreeMap::from([
                (
                    "authors".to_owned(),
                    MetadataValue::Array(vec![
                        MetadataValue::String("Ada".to_owned()),
                        MetadataValue::String("Bo".to_owned()),
                    ]),
                ),
                (
                    "source".to_owned(),
                    MetadataValue::String("example".to_owned()),
                ),
            ]))
        );
        let rendered = render_manifest(&parsed.manifest).expect("metadata should render");
        assert_eq!(parse_manifest(&rendered).unwrap().manifest, parsed.manifest);
    }

    #[test]
    fn fragment_metadata_is_validated_but_not_merged_into_root() {
        let mut root = parse_manifest(
            "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\n\
             schema_version = \"1.0.0\"\n\
             [metadata]\nlabel = \"root\"\n\
             [[resource_definitions]]\nresource_definition_id = \"root/first\"\nbundle_id = \"root\"\ndisplay_name = \"First\"\nvalue_kind = \"integer\"\nmin_value = 0\nmax_value = 10\nrecovery_rule = \"persistent\"\n",
        )
        .expect("root should parse")
        .manifest;
        let fragment = parse_manifest_fragment(
            "[metadata]\nlabel = \"fragment\"\n\
             summary = \"\"\"nested\nmultiline\"\"\"\n\
             [metadata.provenance]\nsource = \"fixture\"\n\
             [[resource_definitions]]\nresource_definition_id = \"root/second\"\nbundle_id = \"root\"\ndisplay_name = \"Second\"\nvalue_kind = \"integer\"\nmin_value = 0\nmax_value = 10\nrecovery_rule = \"persistent\"\n",
        )
        .expect("structured fragment metadata should parse");

        append_fragment_rows(&mut root, [fragment]);

        assert_eq!(root.metadata["label"], MetadataValue::String("root".into()));
        assert_eq!(root.metadata.len(), 1);
        assert_eq!(root.resource_definitions.len(), 2);
        assert_eq!(
            root.resource_definitions[1].resource_definition_id,
            "root/second"
        );
    }

    #[test]
    fn fragment_duplicate_metadata_keys_are_rejected() {
        let error = parse_manifest_fragment("[metadata]\nlabel = \"one\"\nlabel = \"two\"\n")
            .expect_err("duplicate keys in one fragment must fail");
        assert!(matches!(error, ParseError::Toml(_)));
    }

    #[test]
    fn fragment_cannot_redeclare_root_schema_version() {
        let error = parse_manifest_fragment("schema_version = \"1.0.0\"\n")
            .expect_err("schema headers belong to the root manifest");
        assert!(matches!(error, ParseError::Toml(_)));
    }

    #[test]
    fn fragment_cannot_declare_an_applyset() {
        let source = "applyset = \"srd\"\n";
        assert!(matches!(
            parse_manifest_fragment(source),
            Err(ParseError::FragmentApplyset)
        ));
        assert!(matches!(
            split_manifest_fragment_source(source),
            Err(ParseError::FragmentApplyset)
        ));
    }

    #[test]
    fn root_applyset_round_trips_and_is_optional() {
        let with = parse_manifest(&format!(
            "{SCHEMA_DIRECTIVE_PREFIX}{WORLD_MANIFEST_SCHEMA_URL}\nschema_version = \"1.0.0\"\napplyset = \"srd\"\n"
        ))
        .expect("root applyset parses")
        .manifest;
        assert_eq!(with.applyset.as_deref(), Some("srd"));
        let rendered = render_manifest(&with).expect("renders");
        assert!(rendered.contains("applyset = \"srd\""));
        assert_eq!(
            parse_manifest(&rendered)
                .unwrap()
                .manifest
                .applyset
                .as_deref(),
            Some("srd")
        );

        let without = parse_manifest(&format!(
            "{SCHEMA_DIRECTIVE_PREFIX}{WORLD_MANIFEST_SCHEMA_URL}\nschema_version = \"1.0.0\"\n"
        ))
        .unwrap()
        .manifest;
        assert_eq!(without.applyset, None);
        assert!(!render_manifest(&without).unwrap().contains("applyset"));
    }

    #[test]
    fn fragment_metadata_presence_includes_empty_table_forms() {
        assert!(manifest_fragment_has_metadata("[metadata]\n").unwrap());
        assert!(manifest_fragment_has_metadata("metadata = {}\n").unwrap());
        assert!(!manifest_fragment_has_metadata("# metadata is absent\n").unwrap());
    }

    #[test]
    fn fragment_split_accepts_context_dependent_nested_rows() {
        let source = "# retained\n[[bundles.tag_prefixes]]\nprefix = \"srd\"\n";
        let split = split_manifest_fragment_source(source).unwrap();
        assert_eq!(split.row_source, source);
        assert!(split.metadata.is_empty());
        assert!(!split.has_includes);
    }

    #[test]
    fn fragment_split_removes_explicit_empty_metadata_structurally() {
        for source in ["[metadata]\n", "metadata = {}\n"] {
            let split = split_manifest_fragment_source(source).unwrap();
            assert!(split.metadata.is_empty());
            assert!(!split.row_source.contains("metadata"));
        }
    }

    #[test]
    fn fragment_metadata_uses_json_compatibility_validation() {
        let error = parse_manifest_fragment("[metadata]\nreleased = 2026-09-08\n")
            .expect_err("TOML datetime must not be silently stripped");
        assert!(matches!(error, ParseError::UnsupportedMetadataValue(_)));
    }

    #[test]
    fn metadata_integer_safe_range_accepts_exact_endpoints_only() {
        for value in ["9007199254740991", "-9007199254740991"] {
            split_manifest_fragment_source(&format!("[metadata]\nvalue = {value}\n"))
                .expect("exact JavaScript safe endpoint should parse");
        }
        for value in ["9007199254740992", "-9007199254740992"] {
            let error = split_manifest_fragment_source(&format!("[metadata]\nvalue = {value}\n"))
                .expect_err("integer outside JavaScript safe range should reject");
            assert!(matches!(error, ParseError::UnsupportedMetadataValue(_)));
        }
    }

    #[test]
    fn metadata_free_fragment_matches_legacy_concatenation_rows() {
        let root_source = "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\n\
            schema_version = \"1.0.0\"\n";
        let fragment_source = "[[resource_definitions]]\nresource_definition_id = \"demo/value\"\n\
            bundle_id = \"demo\"\ndisplay_name = \"Value\"\nvalue_kind = \"integer\"\n\
            min_value = 0\nmax_value = 10\nrecovery_rule = \"persistent\"\n";
        let legacy = parse_manifest(&format!("{root_source}{fragment_source}"))
            .expect("legacy concatenation should parse")
            .manifest;
        let mut structural = parse_manifest(root_source).unwrap().manifest;
        append_fragment_rows(
            &mut structural,
            [parse_manifest_fragment(fragment_source).unwrap()],
        );

        assert_eq!(structural, legacy);
    }

    #[test]
    fn metadata_rejects_values_that_cannot_round_trip_through_wasm_json() {
        for (value, expected) in [
            (
                "released = 2026-09-08T12:00:00Z",
                "datetimes are not JSON-compatible",
            ),
            ("weight = nan", "floats must be finite"),
            (
                "catalogue_id = 9007199254740992",
                "exceeds JavaScript's safe range",
            ),
        ] {
            let source = format!(
                "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = \"1.0.0\"\n\n[metadata]\n{value}\n"
            );
            let err = parse_manifest(&source).expect_err("unsupported metadata must reject");
            assert!(err.to_string().contains(expected), "{err}");
        }
    }

    /// #1968 review nit: `[[retractions]]` (ADR-0085 D-6) is a small, easy
    /// field to leave lossy in the generic serde round-trip — cheap,
    /// dedicated coverage rather than relying solely on the broad
    /// `render_round_trips_real_world_fixtures` fixtures (none of which
    /// author a retraction today).
    #[test]
    fn retractions_round_trip_through_render_and_parse() {
        let source = r#"#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json
schema_version = "1.0.0"

[[retractions]]
row_kind = "entity"
authored_key = "testpack:testpack/guard_1"

[[retractions]]
row_kind = "niche_definition"
authored_key = "testpack/hidden_alcove"
"#;
        let parsed = parse_manifest(source).expect("manifest with retractions should parse");
        assert_eq!(parsed.manifest.retractions.len(), 2);
        assert_eq!(parsed.manifest.retractions[0].row_kind, "entity");
        assert_eq!(
            parsed.manifest.retractions[0].authored_key,
            "testpack:testpack/guard_1"
        );
        assert_eq!(parsed.manifest.retractions[1].row_kind, "niche_definition");

        let rendered = render_manifest(&parsed.manifest).expect("manifest should render");
        assert!(rendered.contains("[[retractions]]"));
        let reparsed = parse_manifest(&rendered).expect("rendered manifest should parse");
        assert_eq!(reparsed.manifest, parsed.manifest);
    }

    /// #1968: the new `description` column on every previously-description-less
    /// authored row kind must (a) parse existing TOML that omits it and (b)
    /// render byte-stable — omitting the field from the rendered output
    /// entirely rather than emitting an empty-string line — when absent. This
    /// is the `skip_serializing_if = "String::is_empty"` contract; a row using
    /// plain `#[serde(default)]` without the skip would regress this test by
    /// emitting a spurious `description = ""` line on every re-render.
    #[test]
    fn new_description_columns_omit_from_render_when_absent_and_parse_when_present() {
        let source = r#"#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json
schema_version = "1.0.0"

[[entities]]
entity_key = "testpack/guard_1"
bundle_id = "testpack"
name = "Guard"
kind = "npc"
place_node_id = "testpack/gate"

[[actors]]
entity_key = "testpack/guard_1"
bundle_id = "testpack"
name = "Guard"

[[affordance_templates]]
template_id = "testpack/open_gate"
bundle_id = "testpack"
niche_id = "testpack/gate_niche"
capability_id = "testpack/open"
display_name = "Open Gate"
outcome_template_id = "testpack/open_gate/outcome"
"#;
        let parsed = parse_manifest(source).expect("manifest without descriptions should parse");
        assert_eq!(parsed.manifest.entities[0].description, "");
        assert_eq!(parsed.manifest.actors[0].description, "");
        assert_eq!(parsed.manifest.affordance_templates[0].description, "");

        let rendered = render_manifest(&parsed.manifest).expect("manifest should render");
        assert!(
            !rendered.contains("description"),
            "omitted description fields must not appear in rendered output:\n{rendered}"
        );

        let reparsed = parse_manifest(&rendered).expect("rendered manifest should parse");
        assert_eq!(reparsed.manifest, parsed.manifest);

        // Explicit descriptions round-trip too.
        let described = source
            .replacen(
                "name = \"Guard\"\nkind = \"npc\"",
                "name = \"Guard\"\nkind = \"npc\"\ndescription = \"the guard with the limp\"",
                1,
            )
            .replacen(
                "entity_key = \"testpack/guard_1\"\nbundle_id = \"testpack\"\nname = \"Guard\"\n\n",
                "entity_key = \"testpack/guard_1\"\nbundle_id = \"testpack\"\nname = \"Guard\"\ndescription = \"limps but reliable\"\n\n",
                1,
            );
        let parsed_described =
            parse_manifest(&described).expect("manifest with descriptions should parse");
        assert_eq!(
            parsed_described.manifest.entities[0].description,
            "the guard with the limp"
        );
        assert_eq!(
            parsed_described.manifest.actors[0].description,
            "limps but reliable"
        );
        let rendered_described =
            render_manifest(&parsed_described.manifest).expect("manifest should render");
        assert!(rendered_described.contains("the guard with the limp"));
        assert!(rendered_described.contains("limps but reliable"));
        let reparsed_described =
            parse_manifest(&rendered_described).expect("rendered manifest should parse");
        assert_eq!(reparsed_described.manifest, parsed_described.manifest);
    }
}
