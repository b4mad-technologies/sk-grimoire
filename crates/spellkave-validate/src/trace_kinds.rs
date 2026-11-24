//! Validates authored `[[trace_kind_definitions]]` rows (ADR-0043).
//! `location_kinds` names the spatial scopes where traces may attach.

use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;
use std::collections::BTreeSet;

/// `trace_kind_definition.kind_id` is empty.
pub const MF_VAL_TRACE_KIND_ID: &str = "MF-VAL-343";
/// Two `trace_kind_definitions` rows share a `kind_id` (the table PK is global;
/// the second insert would fail at apply — caught loudly here at authoring time).
pub const MF_VAL_TRACE_KIND_DUP: &str = "MF-VAL-250";
/// A `location_kinds` CSV entry is not a registered scope id.
pub const MF_VAL_TRACE_KIND_LOCATION_KIND: &str = "MF-VAL-251";
/// A place-node trace names an unknown trace kind in the available vocabulary.
pub const MF_VAL_PLACE_NODE_TRACE_KIND: &str = "MF-VAL-320";

/// Registered scope ids a trace kind may name in `location_kinds`.
pub const VALID_LOCATION_KINDS: &[&str] = &[
    "place_node",
    "place_edge",
    "place_graph",
    "trace",
    // ADR-0095 D-1: an `entity` row as a trace anchor (co-location with the
    // anchor's CURRENT position admits, per the two new seam arms).
    "entity",
    "any",
];

/// Trace kinds available from the core bootstrap without an authored definition.
pub const BOOTSTRAP_TRACE_KINDS: &[&str] = &["scrape_marks", "scent", "smoke", "handled_item"];

/// Validate the `[[trace_kind_definitions]]` section.
pub fn validate_trace_kind_definitions(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let mut seen_ids: BTreeSet<&str> = BTreeSet::new();

    for (i, row) in manifest.trace_kind_definitions.iter().enumerate() {
        let kind_id = row.kind_id.trim();
        if kind_id.is_empty() {
            diags.push(fatal(
                MF_VAL_TRACE_KIND_ID,
                &format!("trace_kind_definitions[{i}].kind_id"),
                "trace_kind_definition.kind_id must not be empty".to_owned(),
            ));
        } else if !seen_ids.insert(kind_id) {
            diags.push(fatal(
                MF_VAL_TRACE_KIND_DUP,
                &format!("trace_kind_definitions[{i}].kind_id"),
                format!(
                    "trace_kind_definition '{kind_id}': duplicate kind_id in this manifest \
                     (kind_id is a global primary key)"
                ),
            ));
        }

        // `location_kinds` is a comma-joined list of registered scope ids.
        for part in row.location_kinds.split(',') {
            let scope = part.trim();
            if scope.is_empty() {
                continue;
            }
            if !VALID_LOCATION_KINDS.contains(&scope) {
                diags.push(Diagnostic {
                    code: MF_VAL_TRACE_KIND_LOCATION_KIND.to_owned(),
                    severity: Severity::Fatal,
                    message: format!(
                        "trace_kind_definition '{}': location_kinds entry '{scope}' is not a \
                         registered scope. Valid scopes: [{}].",
                        row.kind_id,
                        VALID_LOCATION_KINDS.join(", "),
                    ),
                    path: format!("trace_kind_definitions[{i}].location_kinds"),
                    span: None,
                    hint: Some(
                        "a trace kind attaches at registered scopes (place_node, place_edge, …); \
                         scopes are not modder-authorable today"
                            .to_owned(),
                    ),
                    details: Some(serde_json::json!({
                        "invalid_scope": scope,
                        "valid_scopes": VALID_LOCATION_KINDS,
                    })),
                });
            }
        }
    }
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

/// Validate `place_node_traces.trace_kind_id` closes against
/// `trace_kind_definitions` authored IN THE SAME MANIFEST (sk-9ll.8).
///
/// Closed-world only, by design (mirrors every other offline FK check in
/// this crate, e.g. `MF_VAL_FK_CLOSURE`'s `place_node_trace references
/// unknown node` arm in `lib.rs`): a `trace_kind_id` that resolves against
/// an ALREADY-LIVE kind from a prior apply (not redeclared in this
/// manifest) is deferred to live validation. The two checks together give the
/// ADR-0023 guarantee: the SAME invalid manifest (an entirely undeclared,
/// never-committed `trace_kind_id`) is rejected by both `spellkave
/// validate <file>` and the reducer.
pub fn validate_place_node_trace_kind_closure(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let declared_kinds: BTreeSet<&str> = manifest
        .trace_kind_definitions
        .iter()
        .map(|row| row.kind_id.as_str())
        .chain(BOOTSTRAP_TRACE_KINDS.iter().copied())
        .collect();
    for (i, row) in manifest.place_node_traces.iter().enumerate() {
        if !declared_kinds.contains(row.trace_kind_id.as_str()) {
            diags.push(fatal(
                MF_VAL_PLACE_NODE_TRACE_KIND,
                &format!("place_node_traces[{i}].trace_kind_id"),
                format!(
                    "place_node_trace '{}' references unknown trace_kind_id '{}' (no matching \
                     trace_kind_definitions row in this manifest, and not one of the bootstrap \
                     TRACE_KINDS — a kind from an already-live prior apply is admitted \
                     separately, at commit)",
                    row.node_id, row.trace_kind_id
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{PlaceNodeTraceRow, TraceKindDefinitionRow};

    fn manifest_with(rows: Vec<TraceKindDefinitionRow>) -> Manifest {
        Manifest {
            trace_kind_definitions: rows,
            ..Manifest::default()
        }
    }

    fn row(kind_id: &str, location_kinds: &str) -> TraceKindDefinitionRow {
        TraceKindDefinitionRow {
            kind_id: kind_id.into(),
            bundle_id: "phandelver".into(),
            location_kinds: location_kinds.into(),
            schema_json: String::new(),
            perceptible_for_ticks: None,
            description: "a test trace kind".into(),
        }
    }

    fn codes(diags: &[Diagnostic]) -> Vec<String> {
        diags.iter().map(|d| d.code.clone()).collect()
    }

    #[test]
    fn authored_utterance_kind_is_valid() {
        let manifest = manifest_with(vec![row("phandelver/utterance", "place_node,place_edge")]);
        let mut diags = Vec::new();
        validate_trace_kind_definitions(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "valid authored trace kind must pass: {diags:?}"
        );
    }

    #[test]
    fn empty_kind_id_is_rejected() {
        let manifest = manifest_with(vec![row("", "place_node")]);
        let mut diags = Vec::new();
        validate_trace_kind_definitions(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_TRACE_KIND_ID.to_owned()));
    }

    #[test]
    fn duplicate_kind_id_is_rejected() {
        let manifest = manifest_with(vec![
            row("phandelver/utterance", "place_node"),
            row("phandelver/utterance", "place_edge"),
        ]);
        let mut diags = Vec::new();
        validate_trace_kind_definitions(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_TRACE_KIND_DUP.to_owned()));
    }

    #[test]
    fn unknown_location_kind_is_rejected() {
        // `widget` is not, and never has been, a registered scope.
        let manifest = manifest_with(vec![row("phandelver/utterance", "place_node,widget")]);
        let mut diags = Vec::new();
        validate_trace_kind_definitions(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_TRACE_KIND_LOCATION_KIND.to_owned()));
    }

    #[test]
    fn all_registered_scopes_are_accepted() {
        // place_graph / trace / entity / any are all valid scopes too (ADR-0095
        // D-1 registered `entity` as the sixth).
        let manifest = manifest_with(vec![row(
            "phandelver/wide",
            "place_node,place_edge,place_graph,trace,entity,any",
        )]);
        let mut diags = Vec::new();
        validate_trace_kind_definitions(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "all registered scopes must pass: {diags:?}"
        );
    }

    #[test]
    fn empty_location_kinds_entries_are_skipped() {
        // Trailing/empty CSV parts (e.g. "place_node,") are ignored, not flagged.
        let manifest = manifest_with(vec![row("phandelver/utterance", "place_node,")]);
        let mut diags = Vec::new();
        validate_trace_kind_definitions(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "empty CSV parts must be skipped: {diags:?}"
        );
    }

    fn place_node_trace(node_id: &str, trace_kind_id: &str) -> PlaceNodeTraceRow {
        PlaceNodeTraceRow {
            node_id: node_id.into(),
            trace_kind_id: trace_kind_id.into(),
        }
    }

    #[test]
    fn place_node_trace_referencing_undeclared_kind_is_rejected() {
        // sk-9ll.8: no `[[trace_kind_definitions]]` row for this kind ANYWHERE
        // in the manifest — the offline half of the two-tier check.
        let manifest = Manifest {
            place_node_traces: vec![place_node_trace("phandelver/entrance", "phandelver/scent")],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_place_node_trace_kind_closure(&manifest, &mut diags);
        assert!(codes(&diags).contains(&MF_VAL_PLACE_NODE_TRACE_KIND.to_owned()));
        assert!(diags[0].message.contains("phandelver/scent"));
    }

    #[test]
    fn place_node_trace_referencing_kind_declared_in_same_manifest_is_accepted() {
        // sk-9ll.8: the forward-reference case — the kind is authored in the
        // SAME manifest as the place_node_traces row naming it.
        let manifest = Manifest {
            trace_kind_definitions: vec![row("phandelver/scent", "place_node")],
            place_node_traces: vec![place_node_trace("phandelver/entrance", "phandelver/scent")],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_place_node_trace_kind_closure(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "a kind declared in the same manifest must resolve: {diags:?}"
        );
    }

    #[test]
    fn place_node_trace_with_no_declarations_at_all_is_fine() {
        // No place_node_traces rows at all — nothing to flag.
        let manifest = Manifest::default();
        let mut diags = Vec::new();
        validate_place_node_trace_kind_closure(&manifest, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn place_node_trace_referencing_a_bootstrap_kind_needs_no_declaration() {
        // Bootstrap kinds such as `scrape_marks` need no authored declaration.
        let manifest = Manifest {
            place_node_traces: vec![place_node_trace("phandelver/cellar", "scrape_marks")],
            ..Manifest::default()
        };
        let mut diags = Vec::new();
        validate_place_node_trace_kind_closure(&manifest, &mut diags);
        assert!(
            diags.is_empty(),
            "a bootstrap trace kind needs no manifest declaration: {diags:?}"
        );
    }
}
