//! Shared dependency closure and reference resolution for validation rules.

use std::collections::{BTreeMap, BTreeSet};

use spellkave_schema::Manifest;

/// Map of bundle_id → transitively-reachable bundle_ids (including self and
/// the implicit "core" baseline that admission always seeds first).
pub(crate) struct BundleScope {
    closure: BTreeMap<String, BTreeSet<String>>,
}

impl BundleScope {
    pub(crate) fn build(manifest: &Manifest) -> Self {
        Self::build_with_live(manifest, &BTreeMap::new())
    }

    pub(crate) fn build_with_live(
        manifest: &Manifest,
        live: &BTreeMap<String, BTreeSet<String>>,
    ) -> Self {
        // Incoming headers replace their own edges; other live headers are reference facts.
        let mut deps = live.clone();
        let mut all_bundles: BTreeSet<String> = deps.keys().cloned().collect();
        for row in &manifest.bundles {
            all_bundles.insert(row.bundle_id.clone());
            let entry = deps.entry(row.bundle_id.clone()).or_default();
            entry.clear();
            for dep in &row.depends_on {
                entry.insert(dep.clone());
            }
        }
        // Implicit "core" baseline — admission always seeds it before any
        // bundle's manifest applies. Treat every authored bundle as if it
        // depended on "core" so refs into core's vocabulary do not trip the
        // cross-bundle scope check.
        all_bundles.insert("core".to_owned());

        // Transitive closure (small graph; quadratic fixed-point is fine).
        let mut closure: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for bundle in &all_bundles {
            let mut reach: BTreeSet<String> = BTreeSet::new();
            reach.insert(bundle.clone());
            reach.insert("core".to_owned());
            let mut frontier: Vec<String> = vec![bundle.clone()];
            while let Some(b) = frontier.pop() {
                if let Some(direct) = deps.get(&b) {
                    for d in direct {
                        if reach.insert(d.clone()) {
                            frontier.push(d.clone());
                        }
                    }
                }
            }
            closure.insert(bundle.clone(), reach);
        }
        Self { closure }
    }

    /// True when `referenced_bundle` is in scope of `owning_bundle`.
    pub(crate) fn in_scope(&self, owning_bundle: &str, referenced_bundle: &str) -> bool {
        match self.closure.get(owning_bundle) {
            Some(reach) => reach.contains(referenced_bundle),
            // Unknown owning bundle — be lenient; FK closure check
            // (MF-VAL-101) already flagged the row.
            None => true,
        }
    }

    /// The transitive `depends_on` closure of `owning_bundle` (including self
    /// and the implicit `core` baseline), or `None` when the bundle is unknown
    /// to this manifest (no `[[bundles]]` row). Used by the reset-boundary
    /// existence tier to decide whether the full vocabulary is visible: a
    /// membership check may only reject an unknown token when every bundle in
    /// the closure is present in the manifest under validation (CR-12).
    pub(crate) fn closure_of(&self, owning_bundle: &str) -> Option<&BTreeSet<String>> {
        self.closure.get(owning_bundle)
    }
}

/// Result of checking a reference's bundle scope and the visible vocabulary.
pub(crate) enum RefResolution {
    /// Declared in this manifest and in the using row's `depends_on` closure.
    InScope,
    /// Declared in this manifest but by a bundle outside the closure.
    OutOfScope { decl_bundle: String },
    /// Not declared by any row here, and the full closure IS present → a typo.
    UndeclaredButVisible,
    /// Not declared here and the closure is incomplete (per-file run) → trust.
    Trusted,
}

/// Resolve an authored token inside the using bundle's dependency closure.
/// A missing token is conclusive only when the full closure is visible.
pub(crate) fn resolve_scoped_ref(
    using_bundle: &str,
    token: &str,
    decl_bundle: &BTreeMap<&str, &str>,
    scope: &BundleScope,
    present: &BTreeSet<&str>,
) -> RefResolution {
    match decl_bundle.get(token) {
        Some(decl) => {
            if scope.in_scope(using_bundle, decl) {
                RefResolution::InScope
            } else {
                RefResolution::OutOfScope {
                    decl_bundle: (*decl).to_owned(),
                }
            }
        }
        None => {
            let closure_visible = scope
                .closure_of(using_bundle)
                .is_some_and(|reach| reach.iter().all(|b| present.contains(b.as_str())));
            if closure_visible {
                RefResolution::UndeclaredButVisible
            } else {
                RefResolution::Trusted
            }
        }
    }
}
