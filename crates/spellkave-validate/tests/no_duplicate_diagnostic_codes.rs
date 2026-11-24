// Authors can identify a diagnostic unambiguously by its unique error code.
// Scans the executing checkout's validator sources through Cargo.
// keep-file(K5): diagnostic-code inventory; sk-jay0.7.174.27 and sk-jay0.7.174.28.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// `(constant name, code, file path)` for every `pub const MF_VAL_..: &str =
/// "MF-VAL-.."` line under `dir`, recursing into subdirectories.
fn collect_codes(dir: &Path, out: &mut Vec<(String, String, String)>) {
    for entry in fs::read_dir(dir).expect("read src dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_codes(&path, out);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read source file");
        for line in source.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("pub const MF_VAL_") else {
                continue;
            };
            let Some((name, rest)) = rest.split_once(':') else {
                continue;
            };
            let Some(quote_start) = rest.find('"') else {
                continue;
            };
            let rest = &rest[quote_start + 1..];
            let Some(quote_end) = rest.find('"') else {
                continue;
            };
            let code = &rest[..quote_end];
            if !code.starts_with("MF-VAL-") {
                continue;
            }
            out.push((
                format!("MF_VAL_{name}"),
                code.to_owned(),
                path.display().to_string(),
            ));
        }
    }
}

#[test]
fn no_two_diagnostic_code_constants_share_a_code() {
    let src_dir = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .expect("cargo test must supply the current package directory"),
    )
    .join("src");
    let mut declarations = Vec::new();
    collect_codes(&src_dir, &mut declarations);
    assert!(
        declarations.len() > 30,
        "expected 30+ MF_VAL_* declarations under src/, found {}: the scan may be broken",
        declarations.len()
    );

    // Grouping by code (not by declaration order) is independent of
    // `fs::read_dir`'s unspecified directory order.
    let mut by_code: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
    for (name, code, file) in &declarations {
        by_code
            .entry(code.as_str())
            .or_default()
            .insert(name.as_str(), file.as_str());
    }

    let collisions: Vec<String> = by_code
        .iter()
        .filter(|(_, names)| names.len() > 1)
        .map(|(code, names)| {
            let owners: Vec<String> = names
                .iter()
                .map(|(name, file)| format!("{name} ({file})"))
                .collect();
            format!("{code}: {}", owners.join(", "))
        })
        .collect();
    assert!(
        collisions.is_empty(),
        "an author-facing diagnostic code must name exactly one failure — collision(s):\n{}",
        collisions.join("\n")
    );
}
