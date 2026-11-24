//! Structural validation and deterministic indexing for manifest DSL macros.
//!
//! Parsing, call resolution, and expansion belong to the shared DSL compiler.
//! This module owns only the manifest row contract so every compiler caller
//! receives one unambiguous, closure-ready definition set.

use crate::{Diagnostic, Severity};
use spellkave_dsl::{DslMacroDefinition, DslMacroParameter};
use spellkave_schema::{DslMacroDefinitionRow, DslValueType, Manifest};
use std::collections::{BTreeMap, BTreeSet};

pub const MF_VAL_DSL_MACRO_DEFINITION_DUP: &str = "MF-VAL-350";
pub const MF_VAL_DSL_MACRO_DEFINITION_BUNDLE: &str = "MF-VAL-304";
pub const MF_VAL_DSL_MACRO_DEFINITION_ID: &str = "MF-VAL-354";
pub const MF_VAL_DSL_MACRO_DEFINITION_PARAMETER: &str = "MF-VAL-356";
pub const MF_VAL_DSL_MACRO_DEFINITION_RETURN_TYPE: &str = "MF-VAL-307";

/// Return the unique macro definitions in lexical id order.
///
/// Call this only after [`validate_dsl_macro_definitions`]: duplicates are
/// intentionally omitted instead of silently choosing a definition. This is
/// the manifest-to-compiler handoff point; the compiler can resolve calls from
/// this stable index without reimplementing row ordering or collision policy.
pub fn dsl_macro_definition_index(manifest: &Manifest) -> BTreeMap<&str, &DslMacroDefinitionRow> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for row in &manifest.dsl_macro_definitions {
        *counts.entry(row.macro_id.as_str()).or_default() += 1;
    }

    manifest
        .dsl_macro_definitions
        .iter()
        .filter(|row| counts[row.macro_id.as_str()] == 1)
        .map(|row| (row.macro_id.as_str(), row))
        .collect()
}

/// Convert the unique manifest rows to the shared compiler input shape.
/// Callers run structural validation first, so this deliberately never chooses
/// between duplicate definitions.
pub fn dsl_macro_compiler_definitions(manifest: &Manifest) -> Vec<DslMacroDefinition> {
    dsl_macro_definition_index(manifest)
        .into_values()
        .map(|row| DslMacroDefinition {
            macro_id: row.macro_id.clone(),
            return_type: row.return_type,
            parameters: row
                .parameters
                .iter()
                .map(|parameter| DslMacroParameter {
                    name: parameter.name.clone(),
                    value_type: parameter.value_type,
                })
                .collect(),
            body_source: row.body_source.clone(),
        })
        .collect()
}

/// Macro-aware parser adapters retain the existing parser failure shape, so
/// callers keep their stable diagnostic rendering while analysis always sees
/// the expanded AST.
pub fn parse_predicate(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<spellkave_dsl::Expression, spellkave_dsl::ParseFailure> {
    spellkave_dsl::compile_predicate_with_macros(source, definitions).map_err(|failure| {
        spellkave_dsl::ParseFailure {
            diagnostics: failure.diagnostics,
        }
    })
}

pub fn parse_int_expression(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<spellkave_dsl::Expression, spellkave_dsl::ParseFailure> {
    spellkave_dsl::compile_int_expression_with_macros(source, definitions).map_err(|failure| {
        spellkave_dsl::ParseFailure {
            diagnostics: failure.diagnostics,
        }
    })
}

pub fn parse_outcome(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<spellkave_dsl::Expression, spellkave_dsl::ParseFailure> {
    spellkave_dsl::compile_expression_with_macros(
        source,
        spellkave_dsl::ExpressionKind::Outcome,
        definitions,
    )
    .map_err(|failure| spellkave_dsl::ParseFailure {
        diagnostics: failure.diagnostics,
    })
}

pub fn validate_dsl_macro_definitions(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let bundles: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|row| row.bundle_id.as_str())
        .chain(["core"])
        .collect();
    let mut first_by_id = BTreeMap::<&str, usize>::new();

    for (row_index, row) in manifest.dsl_macro_definitions.iter().enumerate() {
        if let Some(first_index) = first_by_id.insert(row.macro_id.as_str(), row_index) {
            diags.push(fatal(
                MF_VAL_DSL_MACRO_DEFINITION_DUP,
                "dsl_macro_definitions.macro_id",
                format!(
                    "duplicate DSL macro id '{}' at rows [{first_index}, {row_index}]; macro ids must resolve exactly once through the bundle dependency closure",
                    row.macro_id
                ),
            ));
        }

        if !bundles.contains(row.bundle_id.as_str()) {
            diags.push(fatal(
                MF_VAL_DSL_MACRO_DEFINITION_BUNDLE,
                "dsl_macro_definitions.bundle_id",
                format!(
                    "DSL macro '{}' references unknown bundle '{}'",
                    row.macro_id, row.bundle_id
                ),
            ));
        }
        if !id_in_bundle_namespace(&row.macro_id, &row.bundle_id) {
            diags.push(fatal(
                MF_VAL_DSL_MACRO_DEFINITION_ID,
                "dsl_macro_definitions.macro_id",
                format!(
                    "DSL macro id '{}' must live in bundle '{}' namespace",
                    row.macro_id, row.bundle_id
                ),
            ));
        }
        if row.body_source.trim().is_empty() {
            diags.push(fatal(
                MF_VAL_DSL_MACRO_DEFINITION_ID,
                "dsl_macro_definitions.body_source",
                format!("DSL macro '{}' has an empty body_source", row.macro_id),
            ));
        }
        if !is_macro_value_type(row.return_type) {
            diags.push(fatal(
                MF_VAL_DSL_MACRO_DEFINITION_RETURN_TYPE,
                "dsl_macro_definitions.return_type",
                format!(
                    "DSL macro '{}' return_type must be Bool, Int, or Text",
                    row.macro_id
                ),
            ));
        }

        let mut parameter_names = BTreeSet::new();
        for parameter in &row.parameters {
            if parameter.name.is_empty() || !parameter_names.insert(parameter.name.as_str()) {
                diags.push(fatal(
                    MF_VAL_DSL_MACRO_DEFINITION_PARAMETER,
                    "dsl_macro_definitions.parameters",
                    format!(
                        "DSL macro '{}' has an empty or duplicate parameter name '{}'",
                        row.macro_id, parameter.name
                    ),
                ));
            }
            if !is_macro_value_type(parameter.value_type) {
                diags.push(fatal(
                    MF_VAL_DSL_MACRO_DEFINITION_PARAMETER,
                    "dsl_macro_definitions.parameters.value_type",
                    format!(
                        "DSL macro '{}' parameter '{}' type must be Bool, Int, or Text",
                        row.macro_id, parameter.name
                    ),
                ));
            }
        }
    }
}

fn id_in_bundle_namespace(id: &str, bundle_id: &str) -> bool {
    !id.is_empty()
        && !bundle_id.is_empty()
        && (id == bundle_id || id.starts_with(&format!("{bundle_id}/")))
}

fn is_macro_value_type(value_type: DslValueType) -> bool {
    matches!(
        value_type,
        DslValueType::Bool | DslValueType::Int | DslValueType::Text
    )
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
    use spellkave_schema::{BundleRow, DslMacroParameterRow, SchemaVersion};

    fn macro_row(id: &str) -> DslMacroDefinitionRow {
        DslMacroDefinitionRow {
            macro_id: id.to_owned(),
            bundle_id: "srd".to_owned(),
            return_type: DslValueType::Int,
            parameters: vec![DslMacroParameterRow {
                name: "score".to_owned(),
                value_type: DslValueType::Int,
            }],
            body_source: "actor.stat('srd/str')".to_owned(),
            description: String::new(),
        }
    }

    fn manifest(rows: Vec<DslMacroDefinitionRow>) -> Manifest {
        Manifest {
            schema_version: SchemaVersion::new(1, 0, 0),
            bundles: vec![BundleRow {
                bundle_id: "srd".to_owned(),
                display_name: "SRD".to_owned(),
                author: "test".to_owned(),
                version: "1.0.0".to_owned(),
                depends_on: Vec::new(),
                description: String::new(),
                tag_prefixes: Vec::new(),
                allow_flat_tags: false,
                default_extent_shape_id: String::new(),
                default_extent_params_json: String::new(),
            }],
            dsl_macro_definitions: rows,
            ..Manifest::default()
        }
    }

    #[test]
    fn unique_definition_indexes_in_lexical_id_order() {
        let manifest = manifest(vec![macro_row("srd/z"), macro_row("srd/a")]);
        let index = dsl_macro_definition_index(&manifest);
        assert_eq!(
            index.keys().copied().collect::<Vec<_>>(),
            ["srd/a", "srd/z"]
        );
        assert!(validate(&manifest).is_empty());
    }

    #[test]
    fn duplicate_definition_is_fatal_and_not_indexed() {
        let manifest = manifest(vec![
            macro_row("srd/ability_modifier"),
            macro_row("srd/ability_modifier"),
        ]);
        let diags = validate(&manifest);
        assert!(diags
            .iter()
            .any(|diag| diag.code == MF_VAL_DSL_MACRO_DEFINITION_DUP));
        assert!(dsl_macro_definition_index(&manifest).is_empty());
    }

    #[test]
    fn rejects_non_scalar_macro_signature_types() {
        let mut row = macro_row("srd/ref");
        row.return_type = DslValueType::RowRef;
        row.parameters[0].value_type = DslValueType::Outcome;
        let diags = validate(&manifest(vec![row]));
        assert!(diags
            .iter()
            .any(|diag| diag.code == MF_VAL_DSL_MACRO_DEFINITION_RETURN_TYPE));
        assert!(diags
            .iter()
            .any(|diag| diag.code == MF_VAL_DSL_MACRO_DEFINITION_PARAMETER));
    }

    fn validate(manifest: &Manifest) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        validate_dsl_macro_definitions(manifest, &mut diags);
        diags
    }
}
