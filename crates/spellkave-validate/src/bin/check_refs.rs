//! `check-refs` — loud CLI gate for the manifest cross-row reference lint
//! (MF-VAL-210..214) introduced in `srd-conformance-probe` phase 11.
//!
//! Loads one or more world-bundle directories (each pointing at a
//! `manifest.toml` with `[[include]]` files), runs `validate_manifest`,
//! and prints every diagnostic on stdout. Exits non-zero if any
//! diagnostic with severity `Fatal` is produced — surfacing authoring
//! errors that would otherwise be buried among hundreds of `cargo test`
//! results.
//!
//! Wired into `just check-manifest-refs` → `just precommit` → CI per
//! issue #259 acceptance.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use spellkave_schema::Manifest;
use spellkave_validate::{validate_manifest, Diagnostic, Severity};

/// Resolve direct `[[include]]` files structurally. Each file is parsed on its
/// own so fragment-local metadata can coexist with root metadata; row-only
/// TOML is assembled in declaration order and parsed once in root context.
fn load_included_manifest(manifest_path: &Path) -> Result<Manifest, String> {
    let root = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let base_dir = manifest_path
        .parent()
        .ok_or_else(|| format!("manifest path has no parent: {}", manifest_path.display()))?;
    let parsed_root = spellkave_format::parse_manifest(&root)
        .map_err(|e| format!("parse root manifest {}: {e}", manifest_path.display()))?
        .manifest;
    let mut flattened = root;
    flattened.push('\n');
    for inc in parsed_root.includes {
        let inc_path = base_dir.join(&inc.path);
        let inc_src = std::fs::read_to_string(&inc_path)
            .map_err(|e| format!("read include {}: {e}", inc_path.display()))?;
        let fragment = spellkave_format::split_manifest_fragment_source(&inc_src)
            .map_err(|e| format!("parse include {}: {e}", inc_path.display()))?;
        if fragment.has_includes {
            return Err(format!("nested include rejected in {}", inc_path.display()));
        }
        let metadata_diags = spellkave_validate::validate_manifest_metadata(&fragment.metadata);
        if !metadata_diags.is_empty() {
            return Err(format!(
                "invalid include metadata in {}: {}",
                inc_path.display(),
                metadata_diags
                    .into_iter()
                    .map(|diag| format!("{}: {} ({})", diag.code, diag.message, diag.path))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        flattened.push_str(&fragment.row_source);
        flattened.push('\n');
    }
    spellkave_format::parse_manifest(&flattened)
        .map(|parsed| {
            let mut manifest = parsed.manifest;
            manifest.includes.clear();
            manifest
        })
        .map_err(|e| format!("parse flattened manifest {}: {e}", manifest_path.display()))
}

fn format_diag(bundle: &str, d: &Diagnostic) -> String {
    let sev = match d.severity {
        Severity::Info => "Info",
        Severity::Warning => "Warning",
        Severity::Fatal => "Fatal",
    };
    let span = d
        .span
        .as_ref()
        .map(|s| format!(" [{}:{}]", s.start_line, s.start_col))
        .unwrap_or_default();
    let hint = d
        .hint
        .as_ref()
        .map(|h| format!("\n        hint: {h}"))
        .unwrap_or_default();
    format!(
        "{code} {sev}: {bundle}: {path}{span}: {msg}{hint}",
        code = d.code,
        sev = sev,
        bundle = bundle,
        path = d.path,
        span = span,
        msg = d.message,
        hint = hint,
    )
}

/// Merge `src` into `dst` by extending every Vec-shaped top-level field. Non-Vec
/// fields (e.g. `schema_version`) follow last-wins semantics — each authored
/// bundle redeclares them identically.
///
/// Uses serde_json as the field-agnostic walker so the merge stays correct as
/// new Vec fields land on `Manifest` without needing per-field updates.
fn merge_manifest_into(dst: &mut Manifest, src: Manifest) {
    let mut dst_json = serde_json::to_value(&*dst).expect("Manifest → JSON");
    let src_json = serde_json::to_value(src).expect("Manifest → JSON");
    if let (Some(d), Some(s)) = (dst_json.as_object_mut(), src_json.as_object()) {
        for (k, v) in s {
            match (d.get_mut(k), v.as_array()) {
                (Some(existing), Some(src_arr)) if existing.is_array() => {
                    existing
                        .as_array_mut()
                        .expect("checked is_array above")
                        .extend(src_arr.iter().cloned());
                }
                _ => {
                    d.insert(k.clone(), v.clone());
                }
            }
        }
    }
    *dst = serde_json::from_value(dst_json).expect("merged JSON → Manifest");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "usage: check-refs <bundle-dir> [<bundle-dir> ...]\n\
             each bundle-dir must contain a manifest.toml with [[include]] entries.\n\
             All bundle dirs are merged into one combined Manifest before validation,\n\
             matching the runtime's admission order (core seeded first, then layered)."
        );
        return ExitCode::from(2);
    }

    let mut any_load_error = false;
    let mut combined = Manifest::default();
    let combined_label = args.join(" + ");

    for arg in &args {
        // Accept either a bundle DIR (load <dir>/manifest.toml) or a single
        // A standalone manifest file.
        let arg_path = PathBuf::from(arg);
        let manifest_path = if arg_path.extension().is_some_and(|e| e == "toml") {
            arg_path
        } else {
            arg_path.join("manifest.toml")
        };

        println!("[check-refs] loading {}", manifest_path.display());

        let manifest = match load_included_manifest(&manifest_path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[check-refs] LOAD ERROR: {e}");
                any_load_error = true;
                continue;
            }
        };
        merge_manifest_into(&mut combined, manifest);
    }

    if any_load_error {
        eprintln!("[check-refs] FAIL: one or more bundles failed to load");
        return ExitCode::from(1);
    }

    let diags = validate_manifest(&combined);
    let fatal_count = diags
        .iter()
        .filter(|d| matches!(d.severity, Severity::Fatal))
        .count();

    if diags.is_empty() {
        println!("[check-refs] {combined_label}: OK (0 diagnostics)");
        return ExitCode::SUCCESS;
    }

    for d in &diags {
        println!("  {}", format_diag(&combined_label, d));
    }
    println!(
        "[check-refs] {combined_label}: {} diagnostic(s), {} fatal",
        diags.len(),
        fatal_count
    );

    if fatal_count > 0 {
        eprintln!("[check-refs] FAIL: fatal manifest reference diagnostics present");
        return ExitCode::from(1);
    }
    println!("[check-refs] OK: combined manifest clean");
    ExitCode::SUCCESS
}
