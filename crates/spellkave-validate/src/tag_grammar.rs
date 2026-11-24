//! Checks authored tag namespaces, prefix declarations and reference suffixes.
//! Most grammar findings are warnings. Reserved namespaces, invalid natural
//! key fields and unresolved exact-value references are fatal (ADR-0098).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use spellkave_schema::row_kinds::ROW_KIND_NATURAL_KEY;
use spellkave_schema::Manifest;

use crate::reference_scope::BundleScope;
use crate::{Diagnostic, Severity};

/// A `tag` value containing `=` or `:` produces a warning.
pub const MF_VAL_TAG_ALLOWED_CHARACTER: &str = "MF-VAL-296";
/// A `tag` value with no `.` at all, on a bundle that has not opted out via
/// `allow_flat_tags = true`. `Warning`.
pub const MF_VAL_TAG_FLAT: &str = "MF-VAL-297";
/// A `tag` value whose leading namespace is not covered by any prefix
/// declared by the owning bundle, a bundle in its `depends_on` closure, or
/// the always-implicit `core`. `Warning`.
pub const MF_VAL_TAG_UNDECLARED_PREFIX: &str = "MF-VAL-298";
/// A `tag_prefixes` entry claiming the reserved `core` namespace (`prefix ==
/// "core"` or `prefix.starts_with("core.")`). `core.*` is kernel-reserved
/// and cannot be claimed by a bundle.
/// `Fatal`, unlike the three grammar checks above: this is a namespace
/// SQUAT on kernel vocabulary, not an advisory grammar nudge, and admitting
/// it would let a bundle's own declaration silently defeat the
/// undeclared-prefix rule's `core` carve-out for every OTHER bundle too.
pub const MF_VAL_TAG_PREFIX_CORE_RESERVED: &str = "MF-VAL-299";
/// A tag matched to a ref-typed `tag_prefixes` entry (`target_kind` +
/// `natural_key_field` both set) whose suffix (the part after the matched
/// prefix) does not resolve to any `target_kind` row's natural-key value —
/// exactly or as that value's final `/`-segment — within the declaring
/// bundle's own `depends_on` closure (`BundleScope`).
///
/// **Severity is split by ADR-0098 D-1** (bead sk-8b4, operator ruling
/// 2026-08-13):
///
/// - `Fatal` when the matched prefix names an EXACT-VALUE vocabulary
///   ([`is_exact_value_vocabulary`]) — the bundle enumerated its members, so
///   naming a non-member is a contradiction. *You may not name a tag that does
///   not exist.*
/// - `Warning` for every other ref-typed prefix, as it shipped under
///   ADR-0018's 2026-07-28 Postscript — those are cross-references into row
///   families authored for another purpose, never declared complete.
///
/// The flip's trigger was `tag_capability_grant` (sk-bwd) making identity tags
/// an authority surface: a typo'd membership silently grants or denies verbs.
/// Open namespaces stay open either way (ADR-0098 D-2).
pub const MF_VAL_TAG_REF_SUFFIX_UNRESOLVED: &str = "MF-VAL-300";
/// A `tag_prefixes` entry's authored `natural_key_field` does not match
/// `ROW_KIND_NATURAL_KEY`'s recorded natural-key field for the entry's
/// `target_kind` (or names a `target_kind` absent from that registry
/// entirely). `Fatal`, unlike every other check in this module: per
/// design's "natural-key cross-check, not presence-only" note, an
/// unchecked `natural_key_field` is shape-correct nonsense — a bundle could
/// declare a field that never resolves anything and no tooling would ever
/// notice, so this is a hard rejection rather than an advisory nudge.
pub const MF_VAL_TAG_PREFIX_NATURAL_KEY_MISMATCH: &str = "MF-VAL-301";
/// The first non-empty ref-resolution tier contains multiple distinct natural
/// keys. Advisory: no deterministic outbound target can be selected.
pub const MF_VAL_TAG_REF_AMBIGUOUS: &str = "MF-VAL-303";
/// A tag-value definition has an empty identity or ownership field. Fatal:
/// the row cannot be applied or scoped without all three identifiers.
pub const MF_VAL_TAG_VALUE_DEFINITION_EMPTY: &str = "MF-VAL-352";
/// A tag-value definition is not a dotted child of its declared prefix.
/// Fatal: a definition must not silently define a value outside its namespace.
pub const MF_VAL_TAG_VALUE_DEFINITION_PREFIX: &str = "MF-VAL-305";
/// A tag-value definition's bundle is missing or does not declare the exact
/// ref-typed prefix that owns the value. Fatal.
pub const MF_VAL_TAG_VALUE_DEFINITION_OWNERSHIP: &str = "MF-VAL-306";
/// More than one tag-value definition names the same complete tag. Fatal: the
/// complete tag is the row's globally unique natural key.
pub const MF_VAL_TAG_VALUE_DEFINITION_DUP: &str = "MF-VAL-357";

/// The namespace every bundle may reference without declaring it — kernel
/// vocabulary, reserved from bundle declaration (see
/// `MF_VAL_TAG_PREFIX_CORE_RESERVED`).
const CORE_NAMESPACE: &str = "core";

/// One occurrence of a `tag` value across any of the twelve uniform `*_tag`
/// tables (`entity_tag` plus eleven `<def>_tag` siblings — verified
/// field-by-field against `crates/spellkave-schema/src/lib.rs`, every one of
/// them exactly `{<owner_id_field>, tag, bundle_id, source}`). Borrowed, not
/// owned — this is a read-only scan over an already-parsed `Manifest`.
struct TagOccurrence<'a> {
    /// The manifest table this occurrence came from, e.g. `"entity_tag"` —
    /// used only to build a readable diagnostic `path`.
    table: &'static str,
    /// The owning row's own id (e.g. an `entity_key` or `capability_id`) —
    /// named in diagnostic messages so an author can find the offending row.
    owner_id: Cow<'a, str>,
    /// `Cow`, not `&str`, since sk-8b4: the twelve `*_tag` tables carry each
    /// tag as its own already-parsed `String` field (borrowed), but the
    /// `CreateWorldObject` frame-write payload carries them as ONE
    /// comma-separated free-text string that has to be split here (owned).
    tag: Cow<'a, str>,
    bundle_id: &'a str,
}

/// Collects tags supplied in `CreateWorldObject.tags` so they receive the
/// same checks as tags authored in row fields.
fn create_world_object_tag_occurrences<'a>(
    manifest: &'a Manifest,
    out: &mut Vec<TagOccurrence<'a>>,
) {
    // `consequence_frame_writes` rows carry no `bundle_id` of their own; the
    // owning bundle is the one on their `consequence_templates` parent.
    let template_bundles: BTreeMap<&str, &str> = manifest
        .consequence_templates
        .iter()
        .map(|row| (row.template_id.as_str(), row.bundle_id.as_str()))
        .collect();

    for row in &manifest.consequence_frame_writes {
        let Some(tags) = row
            .frame_write
            .get("CreateWorldObject")
            .and_then(|op| op.get("tags"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(&bundle_id) = template_bundles.get(row.template_id.as_str()) else {
            // An orphaned frame-write row is `refs.rs`'s finding to report,
            // not a tag-grammar one; scoping it to a bundle is impossible.
            continue;
        };
        // Split on commas and semicolons, skipping empty segments.
        for tag in tags
            .split([',', ';'])
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
        {
            out.push(TagOccurrence {
                table: "consequence_frame_writes.CreateWorldObject.tags",
                owner_id: Cow::Owned(format!("{}#{}", row.template_id, row.ordinal)),
                tag: Cow::Owned(tag.to_string()),
                bundle_id,
            });
        }
    }
}

/// Every `tag` occurrence across all twelve tag tables, in manifest
/// declaration order (table-by-table, row-by-row within each table).
fn all_tag_occurrences(manifest: &Manifest) -> Vec<TagOccurrence<'_>> {
    let mut out = Vec::new();
    for row in &manifest.entity_definition_tag {
        out.push(TagOccurrence {
            table: "entity_definition_tag",
            owner_id: Cow::Borrowed(row.entity_def_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.object_state_definition_tag {
        out.push(TagOccurrence {
            table: "object_state_definition_tag",
            owner_id: Cow::Borrowed(row.state_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.capability_definition_tag {
        out.push(TagOccurrence {
            table: "capability_definition_tag",
            owner_id: Cow::Borrowed(row.capability_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.niche_definition_tag {
        out.push(TagOccurrence {
            table: "niche_definition_tag",
            owner_id: Cow::Borrowed(row.niche_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.outcome_template_tag {
        out.push(TagOccurrence {
            table: "outcome_template_tag",
            owner_id: Cow::Borrowed(row.outcome_template_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.action_definition_tag {
        out.push(TagOccurrence {
            table: "action_definition_tag",
            owner_id: Cow::Borrowed(row.action_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.resource_definition_tag {
        out.push(TagOccurrence {
            table: "resource_definition_tag",
            owner_id: Cow::Borrowed(row.resource_definition_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.roll_table_tag {
        out.push(TagOccurrence {
            table: "roll_table_tag",
            owner_id: Cow::Borrowed(row.roll_table_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.roll_table_entry_tag {
        out.push(TagOccurrence {
            table: "roll_table_entry_tag",
            owner_id: Cow::Borrowed(row.roll_table_entry_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.modifier_template_tag {
        out.push(TagOccurrence {
            table: "modifier_template_tag",
            owner_id: Cow::Borrowed(row.modifier_template_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.condition_definition_tag {
        out.push(TagOccurrence {
            table: "condition_definition_tag",
            owner_id: Cow::Borrowed(row.condition_definition_id.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    for row in &manifest.entity_tag {
        out.push(TagOccurrence {
            table: "entity_tag",
            owner_id: Cow::Borrowed(row.entity_key.as_str()),
            tag: Cow::Borrowed(row.tag.as_str()),
            bundle_id: row.bundle_id.as_str(),
        });
    }
    // sk-8b4: the thirteenth source — not a table, but the one other place a
    // manifest mints tags. See `create_world_object_tag_occurrences`.
    create_world_object_tag_occurrences(manifest, &mut out);
    out
}

/// The single dotted-prefix-match rule (design Decision 2 / specification's
/// Requirement text, restated verbatim as code): `tag == prefix` or `tag`
/// starts with `prefix` followed by `.`. Deliberately NOT a
/// leading-dot-segment comparison — see this module's and the spec's own
/// worked counter-example for why that alternative is wrong.
pub(crate) fn dotted_prefix_matches(tag: &str, prefix: &str) -> bool {
    tag == prefix
        || tag
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

/// Run all six tag-vocabulary grammar and ref-resolution checks against
/// `manifest`.
pub fn validate_tag_vocabulary(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    validate_core_prefix_reservation(manifest, diags);
    validate_natural_key_field_registry(manifest, diags);
    validate_tag_value_definitions(manifest, diags);

    let occurrences = all_tag_occurrences(manifest);
    let allow_flat_tags: BTreeMap<&str, bool> = manifest
        .bundles
        .iter()
        .map(|row| (row.bundle_id.as_str(), row.allow_flat_tags))
        .collect();
    let bundle_scope = BundleScope::build(manifest);
    let declared_prefixes: BTreeMap<&str, &[spellkave_schema::TagPrefixDeclaration]> = manifest
        .bundles
        .iter()
        .map(|row| (row.bundle_id.as_str(), row.tag_prefixes.as_slice()))
        .collect();
    // Serialized once per pass — `resolve_ref_typed_suffix` indexes into it
    // generically by `target_kind` (a manifest field name), mirroring
    // `ref-tags.ts`'s `resolveRefTagValue(manifest: Record<string, unknown>, ...)`.
    let manifest_json = serde_json::to_value(manifest).unwrap_or(serde_json::Value::Null);

    for occurrence in &occurrences {
        validate_allowed_characters(occurrence, diags);
        validate_flat_tag(occurrence, &allow_flat_tags, diags);
        validate_undeclared_prefix(occurrence, &bundle_scope, &declared_prefixes, diags);
        validate_ref_typed_suffix(
            occurrence,
            &bundle_scope,
            &declared_prefixes,
            &manifest_json,
            diags,
        );
    }
}

/// Validate the opt-in exact-value vocabulary rows themselves. This is
/// deliberately separate from membership resolution: malformed or ambiguously
/// owned definitions would fail live primary-key/apply invariants, while an
/// ordinary tag whose value is absent is MF-VAL-300 (Fatal since ADR-0098 D-1).
fn validate_tag_value_definitions(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let bundles: BTreeMap<&str, &spellkave_schema::BundleRow> = manifest
        .bundles
        .iter()
        .map(|bundle| (bundle.bundle_id.as_str(), bundle))
        .collect();
    let mut seen_tags: BTreeMap<&str, usize> = BTreeMap::new();

    for (index, row) in manifest.tag_value_definitions.iter().enumerate() {
        let path = |field: &str| format!("tag_value_definitions[{index}].{field}");
        let tag = row.tag.as_str();
        let prefix = row.prefix.as_str();
        let bundle_id = row.bundle_id.as_str();

        if tag.trim().is_empty() || prefix.trim().is_empty() || bundle_id.trim().is_empty() {
            let empty_fields = [
                ("tag", tag.trim().is_empty()),
                ("prefix", prefix.trim().is_empty()),
                ("bundle_id", bundle_id.trim().is_empty()),
            ]
            .into_iter()
            .filter_map(|(field, empty)| empty.then_some(field))
            .collect::<Vec<_>>();
            diags.push(fatal(
                MF_VAL_TAG_VALUE_DEFINITION_EMPTY,
                &path(empty_fields.first().copied().unwrap_or("tag")),
                format!(
                    "tag_value_definition at index {index} has empty {} — tag, prefix, and \
                     bundle_id are required identity/ownership fields",
                    empty_fields.join(", ")
                ),
            ));
        }

        if !tag.trim().is_empty()
            && !prefix.trim().is_empty()
            && !tag
                .strip_prefix(prefix)
                .is_some_and(|suffix| suffix.starts_with('.') && suffix.len() > 1)
        {
            diags.push(fatal(
                MF_VAL_TAG_VALUE_DEFINITION_PREFIX,
                &path("tag"),
                format!(
                    "tag_value_definition tag '{}' must be a dotted child of prefix '{}' with \
                     a non-empty suffix",
                    row.tag, row.prefix
                ),
            ));
        }

        if !tag.trim().is_empty() {
            if let Some(first_index) = seen_tags.insert(tag, index) {
                diags.push(fatal(
                    MF_VAL_TAG_VALUE_DEFINITION_DUP,
                    &path("tag"),
                    format!(
                        "tag_value_definition tag '{}' duplicates the complete natural key \
                         authored at index {first_index}",
                        row.tag
                    ),
                ));
            }
        }

        let Some(bundle) = bundles.get(bundle_id).copied() else {
            if !bundle_id.trim().is_empty() {
                diags.push(fatal(
                    MF_VAL_TAG_VALUE_DEFINITION_OWNERSHIP,
                    &path("bundle_id"),
                    format!(
                        "tag_value_definition tag '{}' names missing owning bundle '{}'",
                        row.tag, row.bundle_id
                    ),
                ));
            }
            continue;
        };

        let owns_ref_prefix = bundle.tag_prefixes.iter().any(|entry| {
            entry.prefix == row.prefix
                && entry.target_kind.as_deref() == Some("tag_value_definitions")
                && entry.natural_key_field.as_deref() == Some("tag")
        });
        if !owns_ref_prefix {
            diags.push(fatal(
                MF_VAL_TAG_VALUE_DEFINITION_OWNERSHIP,
                &path("prefix"),
                format!(
                    "tag_value_definition tag '{}' in bundle '{}' requires that bundle's own \
                     tag_prefixes entry '{}' to target tag_value_definitions.tag",
                    row.tag, row.bundle_id, row.prefix
                ),
            ));
        }
    }
}

fn validate_allowed_characters(occurrence: &TagOccurrence<'_>, diags: &mut Vec<Diagnostic>) {
    let Some(offending) = occurrence.tag.chars().find(|c| *c == '=' || *c == ':') else {
        return;
    };
    diags.push(warning(
        MF_VAL_TAG_ALLOWED_CHARACTER,
        &format!("{}.tag", occurrence.table),
        format!(
            "{} row '{}' has tag '{}' containing '{}' — kv/colon-shaped tags smuggle a typed \
             parameter into the membership table; use the dot-namespace convention instead \
             (e.g. 'srd.ability.dex', not '{}')",
            occurrence.table, occurrence.owner_id, occurrence.tag, offending, occurrence.tag
        ),
    ));
}

fn validate_flat_tag(
    occurrence: &TagOccurrence<'_>,
    allow_flat_tags: &BTreeMap<&str, bool>,
    diags: &mut Vec<Diagnostic>,
) {
    if occurrence.tag.contains('.') {
        return;
    }
    if allow_flat_tags
        .get(occurrence.bundle_id)
        .copied()
        .unwrap_or(false)
    {
        return;
    }
    diags.push(warning(
        MF_VAL_TAG_FLAT,
        &format!("{}.tag", occurrence.table),
        format!(
            "{} row '{}' has flat tag '{}' with no namespace segment ('.'); either adopt the \
             dot-namespace convention or set 'allow_flat_tags = true' on bundle '{}'",
            occurrence.table, occurrence.owner_id, occurrence.tag, occurrence.bundle_id
        ),
    ));
}

fn validate_undeclared_prefix(
    occurrence: &TagOccurrence<'_>,
    bundle_scope: &BundleScope,
    declared_prefixes: &BTreeMap<&str, &[spellkave_schema::TagPrefixDeclaration]>,
    diags: &mut Vec<Diagnostic>,
) {
    if dotted_prefix_matches(occurrence.tag.as_ref(), CORE_NAMESPACE) {
        return;
    }
    let in_scope_bundles = bundle_scope.closure_of(occurrence.bundle_id);
    let declared = match in_scope_bundles {
        // Unknown owning bundle — FK closure (MF-VAL-101) already flags the
        // row; be lenient here rather than compounding it with a second
        // diagnostic for an already-reported problem.
        None => return,
        Some(bundles) => bundles.iter().any(|bundle_id| {
            declared_prefixes
                .get(bundle_id.as_str())
                .into_iter()
                .flat_map(|entries| entries.iter())
                .any(|entry| dotted_prefix_matches(occurrence.tag.as_ref(), &entry.prefix))
        }),
    };
    if declared {
        return;
    }
    diags.push(warning(
        MF_VAL_TAG_UNDECLARED_PREFIX,
        &format!("{}.tag", occurrence.table),
        format!(
            "{} row '{}' has tag '{}' whose namespace is not declared by bundle '{}', any \
             bundle in its depends_on closure, or 'core' — add a matching '[[bundle.tag_prefixes]]' \
             entry",
            occurrence.table, occurrence.owner_id, occurrence.tag, occurrence.bundle_id
        ),
    ));
}

// ── The single-origin tag-membership rule (ADR-0098 D-3) ─────────────────
//
// Everything between here and `validate_ref_typed_suffix` is THE rule, in one
// place, for every runtime that writes or inspects a tag. ADR-0098 D-3 binds
// two readers to it — the authoring validator below, resolving against a
// parsed `Manifest`, and the reducer choke point `tags::add_entity_tag`,
// resolving against live tables. Only the VOCABULARY handed to the rule varies
// by runtime; the prefix matching, the longest-match tie-break, the
// bare-prefix carve-out and the verdict never do. That is ADR-0044's
// "admission only varies the manifest it is given … no runtime-local rule
// reimplementation", applied to this rule family, and it is what lets a
// reducer reject without becoming a second validator (ADR-0023 D-1/D-3).

/// One `tag_prefixes` declaration visible to a tag, flattened out of whatever
/// carrier the caller holds: `bundles[].tag_prefixes` for the authoring
/// validator, the `mod_bundle.tag_prefixes` live mirror for the reducer.
///
/// `declaring_bundle` is carried separately from the prefix because
/// ref-resolution scopes against the bundle that DECLARED the prefix, not
/// necessarily the bundle that owns the tagged row.
#[derive(Clone, Copy, Debug)]
pub struct VisibleTagPrefix<'a> {
    pub declaring_bundle: &'a str,
    pub prefix: &'a str,
    pub target_kind: Option<&'a str>,
    pub natural_key_field: Option<&'a str>,
}

/// What the caller was asked to resolve: a tag that matched a ref-typed
/// declaration, decomposed into everything a lookup and a diagnostic need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MembershipQuery<'a> {
    /// The complete observed tag.
    pub tag: &'a str,
    pub declaring_bundle: &'a str,
    pub prefix: &'a str,
    pub target_kind: &'a str,
    pub natural_key_field: &'a str,
    /// The part after the matched prefix and its `.` separator.
    pub suffix: &'a str,
}

/// The caller's answer for one [`MembershipQuery`] — i.e. what its own
/// vocabulary says, never what the rule concludes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MembershipResolution {
    /// Exactly one visible natural key matches.
    Resolved,
    /// No visible natural key matches.
    Unresolved,
    /// The winning match tier holds several distinct keys.
    Ambiguous(Vec<String>),
    /// This caller cannot losslessly reconstruct `target_kind`'s natural-key
    /// space, so it declines to judge (ADR-0098 D-5, mirroring ADR-0044 D-3).
    /// The rule treats a declined query as admitted: the authoring validator,
    /// which sees the whole manifest, is the authoritative surface for that
    /// class, and a runtime that guessed would false-reject content the
    /// validator admits. The authoring validator never returns this.
    Deferred,
}

/// The rule's verdict for one tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagMembership<'a> {
    /// No closed vocabulary claims this tag, or the one that does declares it.
    /// Covers ADR-0098 D-2's open frontier: a tag matching no declared prefix,
    /// and a tag under a bare namespace-only declaration, are both admitted
    /// here — MF-VAL-298 remains the advisory check for the former.
    Admitted,
    /// A declared closed vocabulary claims this tag and does not define it.
    /// ADR-0098 D-1: an admission failure.
    Undeclared(MembershipQuery<'a>),
    /// The vocabulary claims it but cannot name one target deterministically.
    Ambiguous(MembershipQuery<'a>, Vec<String>),
}

/// Resolve one tag against the vocabulary `visible` to it.
///
/// The **longest matching declared prefix wins** (design Decision 2,
/// mirroring `tag-prefix-families.ts`'s `resolveTagPrefixFamily`: sort
/// longest-first, first match wins); ties break on
/// `(declaring_bundle, prefix, target_kind, natural_key_field)` ascending, so
/// the verdict is independent of the order `visible` yields.
///
/// `visible` is an iterator rather than a slice so neither caller has to
/// allocate a per-tag vector in its hot loop.
pub fn resolve_tag_membership<'a, I>(
    tag: &'a str,
    visible: I,
    resolve: &dyn Fn(&MembershipQuery<'_>) -> MembershipResolution,
) -> TagMembership<'a>
where
    I: IntoIterator<Item = VisibleTagPrefix<'a>>,
{
    let mut best: Option<VisibleTagPrefix<'a>> = None;
    for entry in visible {
        if !dotted_prefix_matches(tag, entry.prefix) {
            continue;
        }
        let wins = best.is_none_or(|current| {
            entry.prefix.len() > current.prefix.len()
                || (entry.prefix.len() == current.prefix.len()
                    && (
                        entry.declaring_bundle,
                        entry.prefix,
                        entry.target_kind,
                        entry.natural_key_field,
                    ) < (
                        current.declaring_bundle,
                        current.prefix,
                        current.target_kind,
                        current.natural_key_field,
                    ))
        });
        if wins {
            best = Some(entry);
        }
    }
    // No declared prefix claims the tag — the open frontier ADR-0018 D-1
    // protects and ADR-0098 D-2 leaves open. This is the leg that makes the
    // leftover colon-shaped tags (`faction:party`, `member_of:<party>` —
    // retired as live emitters by sk-ntz.3/sk-ntz.7) need no declaration and
    // no carve-out: they match no dotted prefix, so no closed vocabulary
    // ever claims them.
    let Some(entry) = best else {
        return TagMembership::Admitted;
    };
    // A bare namespace declaration carries no ref semantics — nothing was
    // enumerated, so nothing can be missing from it (ADR-0098 D-2).
    let (Some(target_kind), Some(natural_key_field)) = (entry.target_kind, entry.natural_key_field)
    else {
        return TagMembership::Admitted;
    };
    // Ref semantics only ever apply to the dotted-child form (mirrors
    // `RefTagFamily.prefix`, which always carries the trailing "."): a tag
    // equal to the bare prefix itself has no suffix to resolve.
    let Some(suffix) = tag
        .strip_prefix(entry.prefix)
        .and_then(|rest| rest.strip_prefix('.'))
    else {
        return TagMembership::Admitted;
    };
    let query = MembershipQuery {
        tag,
        declaring_bundle: entry.declaring_bundle,
        prefix: entry.prefix,
        target_kind,
        natural_key_field,
        suffix,
    };
    match resolve(&query) {
        MembershipResolution::Resolved | MembershipResolution::Deferred => TagMembership::Admitted,
        MembershipResolution::Unresolved => TagMembership::Undeclared(query),
        MembershipResolution::Ambiguous(candidates) => TagMembership::Ambiguous(query, candidates),
    }
}

/// Does this declaration name an **exact-value vocabulary** — a catalogue
/// authored for the sole purpose of enumerating tag values?
///
/// ADR-0098 D-1 turns an unresolved membership Fatal for exactly these, and
/// leaves every other ref-typed prefix advisory. The line is not arbitrary:
///
/// - `tag_value_definitions` is the row kind ADR-0018's 2026-07-28 Postscript
///   created *to be a tag vocabulary*. Declaring one is a statement that the
///   list is complete, so naming a non-member is a contradiction. MF-VAL-305
///   forces each row's `tag` to be the complete dotted tag, so resolution is a
///   single exact lookup — which is also why the reducer can reach the same
///   verdict from live tables (D-5).
/// - Every other `target_kind` points at a row family authored for some other
///   purpose (`resource_definitions`, …) and resolves through the *suffix* and
///   final-`/`-segment tiers. Such a tag is a cross-reference, not a
///   membership claim, and the target set was never declared complete — a
///   partially-populated authoring workspace can legitimately hold the tag
///   before the row it points at exists. Those stay MF-VAL-300 `Warning`.
pub fn is_exact_value_vocabulary(target_kind: &str, natural_key_field: &str) -> bool {
    target_kind == "tag_value_definitions" && natural_key_field == "tag"
}

/// The `Undeclared` verdict rendered as a reason clause, shared by the
/// authoring diagnostic and the reducer's rejection so an author reads the
/// same sentence whichever surface catches the tag (ADR-0024 D-2's spirit:
/// one condition, one wording). Callers frame it with their own row context —
/// the validator prefixes `"<table> row '<owner>' has "`, the reducer names
/// the entity. `owning_bundle` is the bundle the TAGGED ROW belongs to, which
/// is not necessarily `query.declaring_bundle`.
pub fn undeclared_membership_reason(query: &MembershipQuery<'_>, owning_bundle: &str) -> String {
    format!(
        "tag '{}' whose suffix '{}' does not resolve to any '{}' row's '{}' within bundle \
         '{}'s declared scope (tag_prefixes entry '{}', declared by bundle '{}')",
        query.tag,
        query.suffix,
        query.target_kind,
        query.natural_key_field,
        owning_bundle,
        query.prefix,
        query.declaring_bundle,
    )
}

/// Does `target_kind`'s natural-key id space (rows visible within `closure`)
/// contain a value matching `suffix`, either exactly or as its final
/// `/`-segment? Mirrors `ref-tags.ts`'s `resolveRefTagValue` exactly (same
/// suffix-match rule), scoped to `closure` (`BundleScope`) instead of
/// `ref-tags.ts`'s manifest-wide scan (design Decision 2 explicitly
/// supersedes the TS mechanism's own scoping precedent for this NEW Rust
/// check — see that decision's "Ref-typed suffix resolution scope" note).
enum RefTagResolution {
    Resolved,
    Unresolved,
    Ambiguous(Vec<String>),
}

fn resolve_ref_tag_value(
    manifest_json: &serde_json::Value,
    target_kind: &str,
    natural_key_field: &str,
    full_tag: &str,
    suffix: &str,
    closure: &BTreeSet<String>,
) -> RefTagResolution {
    let Some(rows) = manifest_json.get(target_kind).and_then(|v| v.as_array()) else {
        return RefTagResolution::Unresolved;
    };
    let keys = rows
        .iter()
        .filter_map(|row| {
            let in_scope = row
                .get("bundle_id")
                .and_then(|v| v.as_str())
                .is_some_and(|bundle_id| closure.contains(bundle_id));
            if !in_scope {
                return None;
            }
            row.get(natural_key_field).and_then(|v| v.as_str())
        })
        .collect::<Vec<_>>();
    for candidates in [
        keys.iter()
            .filter(|key| **key == full_tag)
            .copied()
            .collect::<BTreeSet<_>>(),
        keys.iter()
            .filter(|key| **key == suffix)
            .copied()
            .collect::<BTreeSet<_>>(),
        keys.iter()
            .filter(|key| key.rsplit('/').next() == Some(suffix))
            .copied()
            .collect::<BTreeSet<_>>(),
    ] {
        if candidates.is_empty() {
            continue;
        }
        if candidates.len() == 1 {
            return RefTagResolution::Resolved;
        }
        return RefTagResolution::Ambiguous(candidates.into_iter().map(str::to_owned).collect());
    }
    RefTagResolution::Unresolved
}

fn validate_ref_typed_suffix(
    occurrence: &TagOccurrence<'_>,
    bundle_scope: &BundleScope,
    declared_prefixes: &BTreeMap<&str, &[spellkave_schema::TagPrefixDeclaration]>,
    manifest_json: &serde_json::Value,
    diags: &mut Vec<Diagnostic>,
) {
    // Every entry visible in the OWNING bundle's `depends_on` closure, handed
    // to the shared rule as a lazy iterator (no per-tag allocation). The rule
    // owns prefix matching and the longest-match tie-break; this closure owns
    // only the lookup, against the serialized manifest.
    let Some(owning_closure) = bundle_scope.closure_of(occurrence.bundle_id) else {
        // Unknown owning bundle — MF-VAL-101 already flags the row.
        return;
    };
    let visible = owning_closure.iter().flat_map(|declaring_bundle| {
        declared_prefixes
            .get(declaring_bundle.as_str())
            .into_iter()
            .flat_map(|entries| entries.iter())
            .map(move |entry| VisibleTagPrefix {
                declaring_bundle: declaring_bundle.as_str(),
                prefix: entry.prefix.as_str(),
                target_kind: entry.target_kind.as_deref(),
                natural_key_field: entry.natural_key_field.as_deref(),
            })
    });
    let membership = resolve_tag_membership(occurrence.tag.as_ref(), visible, &|query| {
        // Ref-resolution scopes against the bundle that DECLARED the prefix,
        // not the bundle that owns the tagged row.
        let Some(closure) = bundle_scope.closure_of(query.declaring_bundle) else {
            return MembershipResolution::Deferred;
        };
        match resolve_ref_tag_value(
            manifest_json,
            query.target_kind,
            query.natural_key_field,
            query.tag,
            query.suffix,
            closure,
        ) {
            RefTagResolution::Resolved => MembershipResolution::Resolved,
            RefTagResolution::Unresolved => MembershipResolution::Unresolved,
            RefTagResolution::Ambiguous(candidates) => MembershipResolution::Ambiguous(candidates),
        }
    });
    match membership {
        TagMembership::Admitted => {}
        TagMembership::Ambiguous(query, candidates) => {
            diags.push(warning(
                MF_VAL_TAG_REF_AMBIGUOUS,
                &format!("{}.tag", occurrence.table),
                format!(
                    "{} row '{}' has tag '{}' whose first non-empty ref-resolution tier is ambiguous for '{}' — candidates: {}",
                    occurrence.table, occurrence.owner_id, occurrence.tag, query.target_kind, candidates.join(", ")
                ),
            ));
        }
        // ADR-0098 D-1: Fatal where the bundle declared an EXACT-VALUE
        // vocabulary — it enumerated the members, so naming a non-member is a
        // contradiction, not an advisory nudge. Every other ref-typed prefix
        // is a cross-reference into a row family authored for another purpose
        // and stays Warning; see `is_exact_value_vocabulary`.
        TagMembership::Undeclared(query) => {
            let message = format!(
                "{} row '{}' has {}",
                occurrence.table,
                occurrence.owner_id,
                undeclared_membership_reason(&query, occurrence.bundle_id),
            );
            let path = format!("{}.tag", occurrence.table);
            diags.push(
                if is_exact_value_vocabulary(query.target_kind, query.natural_key_field) {
                    fatal(MF_VAL_TAG_REF_SUFFIX_UNRESOLVED, &path, message)
                } else {
                    warning(MF_VAL_TAG_REF_SUFFIX_UNRESOLVED, &path, message)
                },
            );
        }
    }
}

fn validate_natural_key_field_registry(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for bundle in &manifest.bundles {
        for entry in &bundle.tag_prefixes {
            let (Some(target_kind), Some(natural_key_field)) =
                (&entry.target_kind, &entry.natural_key_field)
            else {
                continue;
            };
            match ROW_KIND_NATURAL_KEY
                .iter()
                .find(|(kind, _)| kind == target_kind)
            {
                Some((_, expected_field)) if expected_field == natural_key_field => {}
                Some((_, expected_field)) => {
                    diags.push(fatal(
                        MF_VAL_TAG_PREFIX_NATURAL_KEY_MISMATCH,
                        "bundle.tag_prefixes",
                        format!(
                            "bundle '{}' tag_prefixes entry '{}' declares natural_key_field \
                             '{}' for target_kind '{}', but the row-kind registry's natural-key \
                             field for '{}' is '{}'",
                            bundle.bundle_id,
                            entry.prefix,
                            natural_key_field,
                            target_kind,
                            target_kind,
                            expected_field
                        ),
                    ));
                }
                None => {
                    diags.push(fatal(
                        MF_VAL_TAG_PREFIX_NATURAL_KEY_MISMATCH,
                        "bundle.tag_prefixes",
                        format!(
                            "bundle '{}' tag_prefixes entry '{}' declares target_kind '{}', which \
                             is not a recognized row kind in the natural-key registry — \
                             natural_key_field '{}' cannot be cross-checked",
                            bundle.bundle_id, entry.prefix, target_kind, natural_key_field
                        ),
                    ));
                }
            }
        }
    }
}

fn validate_core_prefix_reservation(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    for bundle in &manifest.bundles {
        for entry in &bundle.tag_prefixes {
            if dotted_prefix_matches(&entry.prefix, CORE_NAMESPACE) {
                diags.push(fatal(
                    MF_VAL_TAG_PREFIX_CORE_RESERVED,
                    "bundle.tag_prefixes",
                    format!(
                        "bundle '{}' declares tag_prefixes entry '{}' — the 'core' namespace is \
                         reserved for the Spellkave kernel; no bundle may declare ownership of it",
                        bundle.bundle_id, entry.prefix
                    ),
                ));
            }
        }
    }
}

fn warning(code: &str, path: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Warning,
        message,
        path: path.to_owned(),
        span: None,
        hint: None,
        details: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::{
        BundleRow, CapabilityDefinitionTagRow, ConsequenceFrameWriteRow, ConsequenceTemplateRow,
        EntityTagRow, ResourceDefinitionRow, TagPrefixDeclaration, TagValueDefinitionRow,
    };

    fn diags_for(manifest: &Manifest) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        validate_tag_vocabulary(manifest, &mut diags);
        diags
    }

    fn codes(manifest: &Manifest) -> Vec<String> {
        diags_for(manifest).into_iter().map(|d| d.code).collect()
    }

    fn bundle(
        id: &str,
        depends_on: Vec<&str>,
        tag_prefixes: Vec<TagPrefixDeclaration>,
    ) -> BundleRow {
        BundleRow {
            bundle_id: id.into(),
            display_name: id.into(),
            author: "test".into(),
            version: "1.0.0".into(),
            depends_on: depends_on.into_iter().map(String::from).collect(),
            description: String::new(),
            tag_prefixes,
            allow_flat_tags: false,
            default_extent_shape_id: String::new(),
            default_extent_params_json: String::new(),
        }
    }

    fn prefix(p: &str) -> TagPrefixDeclaration {
        TagPrefixDeclaration {
            prefix: p.into(),
            target_kind: None,
            natural_key_field: None,
        }
    }

    fn cap_tag(capability_id: &str, tag: &str, bundle_id: &str) -> CapabilityDefinitionTagRow {
        CapabilityDefinitionTagRow {
            capability_id: capability_id.into(),
            tag: tag.into(),
            bundle_id: bundle_id.into(),
            source: "authored".into(),
        }
    }

    fn entity_tag(entity_key: &str, tag: &str, bundle_id: &str) -> EntityTagRow {
        EntityTagRow {
            entity_key: entity_key.into(),
            tag: tag.into(),
            bundle_id: bundle_id.into(),
            source: "authored".into(),
        }
    }

    fn ref_prefix(p: &str, target_kind: &str, natural_key_field: &str) -> TagPrefixDeclaration {
        TagPrefixDeclaration {
            prefix: p.into(),
            target_kind: Some(target_kind.into()),
            natural_key_field: Some(natural_key_field.into()),
        }
    }

    fn resource_def(id: &str, bundle_id: &str) -> ResourceDefinitionRow {
        ResourceDefinitionRow {
            resource_definition_id: id.into(),
            bundle_id: bundle_id.into(),
            display_name: id.into(),
            value_kind: "int".into(),
            min_value: 0,
            max_value: 100,
            recovery_rule: String::new(),
            initial_value: None,
            description: String::new(),
        }
    }

    fn tag_value_def(tag: &str, prefix: &str, bundle_id: &str) -> TagValueDefinitionRow {
        TagValueDefinitionRow {
            tag: tag.into(),
            prefix: prefix.into(),
            bundle_id: bundle_id.into(),
            description: "test value".into(),
        }
    }

    fn exact_value_prefix(prefix: &str) -> TagPrefixDeclaration {
        ref_prefix(prefix, "tag_value_definitions", "tag")
    }

    #[test]
    fn well_formed_exact_tag_value_definition_is_clean() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![exact_value_prefix("demo.creature_type")],
        ));
        m.tag_value_definitions.push(tag_value_def(
            "demo.creature_type.humanoid",
            "demo.creature_type",
            "demo",
        ));
        assert!(
            diags_for(&m).is_empty(),
            "well-formed definition should be clean"
        );
    }

    #[test]
    fn tag_value_definition_empty_identity_is_fatal() {
        let mut m = Manifest::default();
        m.tag_value_definitions.push(tag_value_def("", "", ""));
        let hit = diags_for(&m)
            .into_iter()
            .find(|diagnostic| diagnostic.code == MF_VAL_TAG_VALUE_DEFINITION_EMPTY)
            .expect("empty identity must be rejected");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn tag_value_definition_rejects_whitespace_padded_identity() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![exact_value_prefix("demo.creature_type")],
        ));
        m.tag_value_definitions.push(tag_value_def(
            " demo.creature_type.humanoid",
            "demo.creature_type",
            "demo",
        ));
        assert!(
            codes(&m).contains(&MF_VAL_TAG_VALUE_DEFINITION_PREFIX.to_owned()),
            "stored tag, not a trimmed surrogate, must satisfy dotted-child identity"
        );
    }

    #[test]
    fn tag_value_definition_missing_owner_bundle_is_fatal() {
        let mut m = Manifest::default();
        m.tag_value_definitions.push(tag_value_def(
            "demo.creature_type.humanoid",
            "demo.creature_type",
            "missing",
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|diagnostic| diagnostic.code == MF_VAL_TAG_VALUE_DEFINITION_OWNERSHIP)
            .expect("missing owner bundle must be rejected");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(hit.message.contains("missing"));
    }

    #[test]
    fn tag_value_definition_outside_prefix_is_fatal() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![exact_value_prefix("demo.creature_type")],
        ));
        m.tag_value_definitions.push(tag_value_def(
            "demo.damage_type.fire",
            "demo.creature_type",
            "demo",
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|diagnostic| diagnostic.code == MF_VAL_TAG_VALUE_DEFINITION_PREFIX)
            .expect("outside-prefix definition must be rejected");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(hit.message.contains("demo.damage_type.fire"));
        assert!(hit.message.contains("demo.creature_type"));
    }

    #[test]
    fn tag_value_definition_requires_owning_ref_typed_prefix() {
        let mut m = Manifest::default();
        m.bundles
            .push(bundle("demo", vec![], vec![prefix("demo.creature_type")]));
        m.tag_value_definitions.push(tag_value_def(
            "demo.creature_type.humanoid",
            "demo.creature_type",
            "demo",
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|diagnostic| diagnostic.code == MF_VAL_TAG_VALUE_DEFINITION_OWNERSHIP)
            .expect("bare namespace must not admit exact values");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(hit.message.contains("tag_value_definitions.tag"));
    }

    #[test]
    fn dependent_bundle_cannot_extend_another_bundles_exact_values() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![exact_value_prefix("demo.creature_type")],
        ));
        m.bundles.push(bundle("homebrew", vec!["demo"], vec![]));
        m.tag_value_definitions.push(tag_value_def(
            "demo.creature_type.new_kind",
            "demo.creature_type",
            "homebrew",
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|diagnostic| diagnostic.code == MF_VAL_TAG_VALUE_DEFINITION_OWNERSHIP)
            .expect("dependent bundle must not extend another bundle's prefix");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(hit.message.contains("homebrew"));
    }

    #[test]
    fn duplicate_complete_tag_value_definition_is_fatal_and_deterministic() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![exact_value_prefix("demo.creature_type")],
        ));
        m.tag_value_definitions.extend([
            tag_value_def("demo.creature_type.humanoid", "demo.creature_type", "demo"),
            tag_value_def("demo.creature_type.humanoid", "demo.creature_type", "demo"),
        ]);
        let hit = diags_for(&m)
            .into_iter()
            .find(|diagnostic| diagnostic.code == MF_VAL_TAG_VALUE_DEFINITION_DUP)
            .expect("duplicate complete tag must be rejected");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(hit.message.contains("demo.creature_type.humanoid"));
    }

    // --- Scenario: A `key=value`-shaped tag is flagged, not rejected -------

    #[test]
    fn kv_shaped_tag_warns_296() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("srd", vec![], vec![prefix("srd")]));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "ability=dex", "srd"));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_TAG_ALLOWED_CHARACTER)
            .expect("MF-VAL-296 must fire for a kv-shaped tag");
        assert_eq!(hit.severity, Severity::Warning);
        assert!(!codes(&m).contains(&"MF-VAL-001".to_string())); // sanity: no unrelated Fatal
    }

    // --- Scenario: A colon-shaped tag is flagged ----------------------------

    #[test]
    fn colon_shaped_tag_warns_296() {
        let mut m = Manifest::default();
        m.entity_tag
            .push(entity_tag("core/goblin-1", "ability:dex", "core"));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_ALLOWED_CHARACTER)
            .expect("MF-VAL-296 must fire for a colon-shaped tag");
        assert_eq!(hit.severity, Severity::Warning);
    }

    #[test]
    fn dotted_tag_with_no_special_characters_is_clean_296() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("core", vec![], vec![]));
        m.entity_tag
            .push(entity_tag("core/goblin-1", "core.actor", "core"));
        assert!(!codes(&m).contains(&MF_VAL_TAG_ALLOWED_CHARACTER.to_string()));
    }

    // --- Scenario: A flat tag is flagged unless the bundle opts out --------

    #[test]
    fn flat_tag_warns_unless_bundle_opts_out_297() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("core", vec![], vec![]));
        m.entity_tag
            .push(entity_tag("core/goblin-1", "goblin", "core"));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_FLAT)
            .expect("MF-VAL-297 must fire for a flat tag with no opt-out");
        assert_eq!(hit.severity, Severity::Warning);
    }

    // --- Scenario: A flat tag is accepted when the bundle opts out ---------

    #[test]
    fn flat_tag_accepted_when_bundle_opts_out_297() {
        let mut m = Manifest::default();
        let mut proto = bundle("prototype", vec![], vec![]);
        proto.allow_flat_tags = true;
        m.bundles.push(proto);
        m.entity_tag
            .push(entity_tag("prototype/goblin-1", "goblin", "prototype"));
        assert!(!codes(&m).contains(&MF_VAL_TAG_FLAT.to_string()));
    }

    // --- Scenario: An undeclared-prefix tag is flagged ----------------------

    #[test]
    fn undeclared_prefix_tag_warns_298() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("owner", vec![], vec![])); // no tag_prefixes declared
        m.entity_tag
            .push(entity_tag("owner/goblin-1", "srd5e.goblin", "owner"));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_UNDECLARED_PREFIX)
            .expect("MF-VAL-298 must fire for an undeclared prefix");
        assert_eq!(hit.severity, Severity::Warning);
    }

    // --- Scenario: A declared single-segment prefix passes cleanly ---------

    // --- sk-8b4: the `CreateWorldObject` frame-write `tags` payload ---------
    // The one manifest tag-minting surface that is not a `*_tag` table. Before
    // this it was invisible to every check in this module, so a consequence
    // template could mint an arbitrary, ungrammatical, PERMANENT instance tag
    // that no validator ever saw.

    fn create_world_object_frame_write(
        template_id: &str,
        bundle_id: &str,
        tags: &str,
    ) -> (ConsequenceTemplateRow, ConsequenceFrameWriteRow) {
        let template = ConsequenceTemplateRow {
            template_id: template_id.into(),
            bundle_id: bundle_id.into(),
            effect_refs_csv: String::new(),
            traces_json: "[]".into(),
            description: String::new(),
        };
        let write = ConsequenceFrameWriteRow {
            template_id: template_id.into(),
            ordinal: 0,
            // Deliberately a JSON *literal*, never `serde_json::to_value` of a
            // `FrameWriteOp`: this pins the externally-tagged wire shape the
            // reader in `create_world_object_tag_occurrences` depends on. If
            // anyone ever adds `#[serde(tag = …)]` to `FrameWriteOp`, the
            // reader silently matches nothing and reports zero findings — a
            // literal here turns that into a red test instead.
            frame_write: serde_json::json!({
                "CreateWorldObject": {
                    "object_def_id": "demo/wall",
                    "initial_state_ids": [],
                    "tags": tags,
                }
            }),
        };
        (template, write)
    }

    fn with_frame_write_tags(bundle: BundleRow, tags: &str) -> Manifest {
        let bundle_id = bundle.bundle_id.clone();
        let mut m = Manifest::default();
        m.bundles.push(bundle);
        let (template, write) =
            create_world_object_frame_write("demo/build_wall", &bundle_id, tags);
        m.consequence_templates.push(template);
        m.consequence_frame_writes.push(write);
        m
    }

    #[test]
    fn create_world_object_tags_are_scanned_for_undeclared_prefix_298() {
        // The bead's own reproduction: a consequence template minting
        // `srd.advancement_trainer` — a namespace the bundle never declared.
        let m = with_frame_write_tags(bundle("owner", vec![], vec![]), "srd.advancement_trainer");
        assert!(codes(&m).contains(&MF_VAL_TAG_UNDECLARED_PREFIX.to_string()));
    }

    #[test]
    fn create_world_object_tags_split_on_the_runtime_separators() {
        // `split_tags` semantics: `,` and `;`, trimmed, empties dropped. The
        // shipped example is `tags = "wall,cover,built_artifact"` — three flat
        // tags, each of which must be reported in its own right.
        let m = with_frame_write_tags(
            bundle("owner", vec![], vec![]),
            " wall,cover ; built_artifact ; ",
        );
        let flat = codes(&m).iter().filter(|c| *c == MF_VAL_TAG_FLAT).count();
        assert_eq!(flat, 3, "each split tag is its own occurrence");
    }

    #[test]
    fn create_world_object_tags_under_a_declared_prefix_are_clean() {
        let m = with_frame_write_tags(
            bundle("owner", vec![], vec![prefix("owner")]),
            "owner.built_artifact",
        );
        assert!(!codes(&m).contains(&MF_VAL_TAG_UNDECLARED_PREFIX.to_string()));
    }

    #[test]
    fn create_world_object_tags_resolve_ref_typed_prefixes_300() {
        // A closed vocabulary (`target_kind = "tag_value_definitions"`): a
        // frame write naming a value that does not exist is now caught at
        // authoring time, in the single source of truth, rather than being
        // minted permanently at runtime.
        let mut m = with_frame_write_tags(
            bundle("owner", vec![], vec![exact_value_prefix("owner.kind")]),
            "owner.kind.nonexistent",
        );
        m.tag_value_definitions
            .push(tag_value_def("owner.kind.real", "owner.kind", "owner"));
        assert!(codes(&m).contains(&MF_VAL_TAG_REF_SUFFIX_UNRESOLVED.to_string()));
    }

    #[test]
    fn non_create_world_object_frame_writes_are_not_tag_occurrences() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("owner", vec![], vec![]));
        m.consequence_templates.push(ConsequenceTemplateRow {
            template_id: "demo/remove".into(),
            bundle_id: "owner".into(),
            effect_refs_csv: String::new(),
            traces_json: "[]".into(),
            description: String::new(),
        });
        m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
            template_id: "demo/remove".into(),
            ordinal: 0,
            frame_write: serde_json::json!({ "RemoveItem": { "count": 4 } }),
        });
        assert!(!codes(&m).contains(&MF_VAL_TAG_FLAT.to_string()));
    }

    #[test]
    fn declared_single_segment_prefix_is_clean_298() {
        let mut m = Manifest::default();
        m.bundles
            .push(bundle("owner", vec![], vec![prefix("srd5e")]));
        m.entity_tag
            .push(entity_tag("owner/goblin-1", "srd5e.goblin", "owner"));
        assert!(!codes(&m).contains(&MF_VAL_TAG_UNDECLARED_PREFIX.to_string()));
    }

    // --- Scenario: A declared multi-segment prefix does not false-flag ------
    // its own tags (the leading-segment-only false-positive regression).

    #[test]
    fn declared_multi_segment_prefix_does_not_false_flag_298() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![TagPrefixDeclaration {
                prefix: "srd.ability".into(),
                target_kind: Some("resource_definitions".into()),
                natural_key_field: Some("resource_definition_id".into()),
            }],
        ));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wis", "srd"));
        assert!(
            !codes(&m).contains(&MF_VAL_TAG_UNDECLARED_PREFIX.to_string()),
            "a bundle declaring only 'srd.ability' must not false-flag its own 'srd.ability.wis' \
             tag by comparing only the leading dot-segment"
        );
    }

    /// A first-segment-only comparison would compare `srd` (the tag's
    /// leading segment) against the declared string `srd.ability` and never
    /// match; a bare, undeclared `srd.other.thing` (different second
    /// segment) SHOULD still warn — this pins that the dotted-prefix-match
    /// rule is not accidentally over-permissive either.
    #[test]
    fn declared_multi_segment_prefix_does_not_cover_a_different_second_segment_298() {
        let mut m = Manifest::default();
        m.bundles
            .push(bundle("srd", vec![], vec![prefix("srd.ability")]));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.other.thing", "srd"));
        assert!(codes(&m).contains(&MF_VAL_TAG_UNDECLARED_PREFIX.to_string()));
    }

    // --- Scenario: A `core.*` tag never triggers the undeclared-prefix rule

    #[test]
    fn core_tag_never_triggers_undeclared_prefix_298() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("owner", vec![], vec![])); // declares nothing, incl. not "core"
        m.entity_tag
            .push(entity_tag("owner/goblin-1", "core.actor", "owner"));
        assert!(!codes(&m).contains(&MF_VAL_TAG_UNDECLARED_PREFIX.to_string()));
    }

    #[test]
    fn undeclared_prefix_resolves_via_depends_on_closure_298() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("vocab", vec![], vec![prefix("srd")]));
        m.bundles.push(bundle("owner", vec!["vocab"], vec![]));
        m.entity_tag
            .push(entity_tag("owner/goblin-1", "srd.goblin", "owner"));
        assert!(
            !codes(&m).contains(&MF_VAL_TAG_UNDECLARED_PREFIX.to_string()),
            "'srd' is declared by 'vocab', which 'owner' depends_on"
        );
    }

    // --- The `core` namespace is reserved and cannot be declared ------------

    #[test]
    fn bundle_declaring_core_prefix_is_rejected_299() {
        let mut m = Manifest::default();
        m.bundles
            .push(bundle("owner", vec![], vec![prefix("core")]));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_PREFIX_CORE_RESERVED)
            .expect("MF-VAL-299 must fire for a bundle declaring 'core'");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn bundle_declaring_core_dotted_prefix_is_rejected_299() {
        let mut m = Manifest::default();
        m.bundles
            .push(bundle("owner", vec![], vec![prefix("core.system")]));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_PREFIX_CORE_RESERVED)
            .expect("MF-VAL-299 must fire for a bundle declaring 'core.system'");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn bundle_declaring_a_prefix_merely_starting_with_core_text_is_not_reserved_299() {
        // "corex" is NOT "core" or "core."-prefixed — must not false-positive
        // on a substring match.
        let mut m = Manifest::default();
        m.bundles
            .push(bundle("owner", vec![], vec![prefix("corex")]));
        assert!(!codes(&m).contains(&MF_VAL_TAG_PREFIX_CORE_RESERVED.to_string()));
    }

    #[test]
    fn manifest_failing_all_three_warning_rules_still_validates_clean_of_fatal() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("owner", vec![], vec![])); // no allow_flat_tags, no tag_prefixes
        m.entity_tag
            .push(entity_tag("owner/goblin-1", "ability=dex", "owner")); // kv AND flat AND undeclared
        let diags = diags_for(&m);
        assert!(diags.iter().all(|d| d.severity != Severity::Fatal));
        assert!(diags.iter().any(|d| d.code == MF_VAL_TAG_ALLOWED_CHARACTER));
        assert!(diags.iter().any(|d| d.code == MF_VAL_TAG_FLAT));
        assert!(diags.iter().any(|d| d.code == MF_VAL_TAG_UNDECLARED_PREFIX));
    }

    // -------------------------------------------------------------------
    // Task 3.2 — ref-typed suffix resolution
    // -------------------------------------------------------------------

    // --- Scenario: A resolving ref-typed tag passes -------------------------

    #[test]
    fn resolving_ref_typed_tag_is_clean_300() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![ref_prefix(
                "srd.ability",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        m.resource_definitions.push(resource_def("srd/wis", "srd"));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wis", "srd"));
        assert!(!codes(&m).contains(&MF_VAL_TAG_REF_SUFFIX_UNRESOLVED.to_string()));
    }

    // --- Scenario: A typo'd ref-typed tag is flagged ------------------------

    #[test]
    fn typod_ref_typed_tag_warns_300() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![ref_prefix(
                "srd.ability",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        m.resource_definitions.push(resource_def("srd/wis", "srd"));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wsi", "srd"));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_REF_SUFFIX_UNRESOLVED)
            .expect("MF-VAL-300 must fire for a typo'd ref-typed tag suffix");
        // ADR-0098 D-1 deliberately does NOT promote this one: 'srd.ability'
        // points at `resource_definitions`, a row family authored for another
        // purpose and never declared complete, so an unresolved reference
        // stays advisory. See `undeclared_member_of_an_exact_value_vocabulary_is_fatal_300`
        // for the case that IS an admission failure.
        assert_eq!(hit.severity, Severity::Warning);
        assert!(hit.message.contains("wsi"));
        assert!(hit.message.contains("resource_definitions"));
    }

    #[test]
    fn full_tag_tier_wins_over_suffix_and_slash_segment() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![ref_prefix(
                "demo.creature_type",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        m.resource_definitions.extend([
            resource_def("demo.creature_type.humanoid", "demo"),
            resource_def("humanoid", "demo"),
            resource_def("other/humanoid", "demo"),
        ]);
        m.capability_definition_tag.push(cap_tag(
            "demo/capability",
            "demo.creature_type.humanoid",
            "demo",
        ));
        let codes = codes(&m);
        assert!(!codes.contains(&MF_VAL_TAG_REF_SUFFIX_UNRESOLVED.to_owned()));
        assert!(!codes.contains(&MF_VAL_TAG_REF_AMBIGUOUS.to_owned()));
    }

    #[test]
    fn multiple_distinct_winning_tier_keys_warn_303() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![ref_prefix(
                "demo.creature_type",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        m.resource_definitions.extend([
            resource_def("one/humanoid", "demo"),
            resource_def("two/humanoid", "demo"),
        ]);
        m.capability_definition_tag.push(cap_tag(
            "demo/capability",
            "demo.creature_type.humanoid",
            "demo",
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|diagnostic| diagnostic.code == MF_VAL_TAG_REF_AMBIGUOUS)
            .expect("multiple final-segment candidates must warn");
        assert_eq!(hit.severity, Severity::Warning);
        assert!(hit.message.contains("one/humanoid, two/humanoid"));
    }

    // --- Scenario: The longest matching declared prefix wins ---------------

    #[test]
    fn longest_matching_declared_prefix_wins_for_ref_resolution_300() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![
                prefix("srd"), // no ref semantics
                ref_prefix(
                    "srd.ability",
                    "resource_definitions",
                    "resource_definition_id",
                ),
            ],
        ));
        m.resource_definitions.push(resource_def("srd/wis", "srd"));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wis", "srd"));
        // Resolves cleanly via the more specific 'srd.ability' declaration —
        // if the bare 'srd' (no ref semantics) declaration won instead, there
        // would be nothing to resolve against and this would stay clean too,
        // so the real assertion is in the sibling test below: an unresolved
        // suffix under the specific declaration still warns, proving the
        // specific declaration (not the bare one) governs resolution.
        assert!(!codes(&m).contains(&MF_VAL_TAG_REF_SUFFIX_UNRESOLVED.to_string()));
    }

    #[test]
    fn longest_matching_declared_prefix_governs_even_when_it_is_the_one_that_warns_300() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![
                prefix("srd"), // no ref semantics — would never warn on its own
                ref_prefix(
                    "srd.ability",
                    "resource_definitions",
                    "resource_definition_id",
                ),
            ],
        ));
        // No 'srd/wsi' resource_definitions row authored at all.
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wsi", "srd"));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_REF_SUFFIX_UNRESOLVED)
            .expect(
                "the specific 'srd.ability' ref-typed declaration must govern resolution, not \
                 the bare 'srd' declaration (which carries no ref semantics and would never warn)",
            );
        assert_eq!(hit.severity, Severity::Warning);
    }

    // --- Scenario: Ref resolution does not reach outside the bundle's ------
    // dependency closure.

    #[test]
    fn ref_resolution_does_not_reach_outside_depends_on_closure_300() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![], // does NOT depend_on "other"
            vec![ref_prefix(
                "srd.ability",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        m.bundles.push(bundle("other", vec![], vec![]));
        // The 'srd/wis' row is authored only by 'other', which 'srd' does not depend on.
        m.resource_definitions
            .push(resource_def("srd/wis", "other"));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wis", "srd"));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_REF_SUFFIX_UNRESOLVED)
            .expect("resolution must not reach outside the declaring bundle's depends_on closure");
        assert_eq!(hit.severity, Severity::Warning);
    }

    #[test]
    fn ref_resolution_reaches_a_dependency_in_scope_300() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec!["vocab"],
            vec![ref_prefix(
                "srd.ability",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        m.bundles.push(bundle("vocab", vec![], vec![]));
        m.resource_definitions
            .push(resource_def("srd/wis", "vocab"));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wis", "srd"));
        assert!(
            !codes(&m).contains(&MF_VAL_TAG_REF_SUFFIX_UNRESOLVED.to_string()),
            "'vocab' is in scope via 'srd's depends_on"
        );
    }

    #[test]
    fn ref_suffix_resolves_via_final_slash_segment() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![ref_prefix(
                "srd.ability",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        m.resource_definitions.push(resource_def("srd/wis", "srd"));
        m.capability_definition_tag
            .push(cap_tag("srd/skill/acrobatics", "srd.ability.wis", "srd"));
        assert!(!codes(&m).contains(&MF_VAL_TAG_REF_SUFFIX_UNRESOLVED.to_string()));
    }

    // -------------------------------------------------------------------
    // Task 3.2 — `natural_key_field` vs. row-kind registry cross-check
    // -------------------------------------------------------------------

    // --- Scenario: `natural_key_field` matching the registry passes --------

    #[test]
    fn matching_natural_key_field_is_clean_301() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![ref_prefix(
                "srd.ability",
                "resource_definitions",
                "resource_definition_id",
            )],
        ));
        assert!(!codes(&m).contains(&MF_VAL_TAG_PREFIX_NATURAL_KEY_MISMATCH.to_string()));
    }

    // --- Scenario: A mismatched `natural_key_field` is rejected -------------

    #[test]
    fn mismatched_natural_key_field_is_rejected_301() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![ref_prefix(
                "srd.ability",
                "resource_definitions",
                "display_name",
            )],
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_PREFIX_NATURAL_KEY_MISMATCH)
            .expect("MF-VAL-301 must fire for a mismatched natural_key_field");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(hit.message.contains("display_name"));
        assert!(hit.message.contains("resource_definition_id"));
    }

    #[test]
    fn unknown_target_kind_is_rejected_301() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "srd",
            vec![],
            vec![ref_prefix("srd.ability", "not_a_real_row_kind", "anything")],
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_PREFIX_NATURAL_KEY_MISMATCH)
            .expect("MF-VAL-301 must fire for a target_kind absent from the registry");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    // ── ADR-0098 D-1: the severity split ─────────────────────────────────

    /// The promoted case. `demo.creature_type` is an EXACT-VALUE vocabulary —
    /// the bundle enumerated its members — so naming a non-member is an
    /// admission failure, not an advisory nudge.
    #[test]
    fn undeclared_member_of_an_exact_value_vocabulary_is_fatal_300() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![ref_prefix(
                "demo.creature_type",
                "tag_value_definitions",
                "tag",
            )],
        ));
        m.tag_value_definitions.push(TagValueDefinitionRow {
            tag: "demo.creature_type.humanoid".into(),
            prefix: "demo.creature_type".into(),
            bundle_id: "demo".into(),
            description: "the only declared member".into(),
        });
        m.entity_tag.push(entity_tag(
            "demo/villager",
            "demo.creature_type.grue",
            "demo",
        ));
        let hit = diags_for(&m)
            .into_iter()
            .find(|d| d.code == MF_VAL_TAG_REF_SUFFIX_UNRESOLVED)
            .expect("MF-VAL-300 must fire for a non-member of a closed vocabulary");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(hit.message.contains("grue"), "{}", hit.message);
    }

    /// The declared member of the same vocabulary stays silent — the gate is
    /// selective, not a blanket rejection of the namespace.
    #[test]
    fn declared_member_of_an_exact_value_vocabulary_is_silent() {
        let mut m = Manifest::default();
        m.bundles.push(bundle(
            "demo",
            vec![],
            vec![ref_prefix(
                "demo.creature_type",
                "tag_value_definitions",
                "tag",
            )],
        ));
        m.tag_value_definitions.push(TagValueDefinitionRow {
            tag: "demo.creature_type.humanoid".into(),
            prefix: "demo.creature_type".into(),
            bundle_id: "demo".into(),
            description: "declared".into(),
        });
        m.entity_tag.push(entity_tag(
            "demo/villager",
            "demo.creature_type.humanoid",
            "demo",
        ));
        assert!(!codes(&m).contains(&MF_VAL_TAG_REF_SUFFIX_UNRESOLVED.to_string()));
    }

    /// Pins the diagnostic classification used for live revalidation.
    #[test]
    fn only_tag_value_definitions_keyed_on_tag_is_an_exact_value_vocabulary() {
        assert!(is_exact_value_vocabulary("tag_value_definitions", "tag"));
        assert!(!is_exact_value_vocabulary(
            "resource_definitions",
            "resource_definition_id"
        ));
        assert!(!is_exact_value_vocabulary(
            "tag_value_definitions",
            "prefix"
        ));
        assert!(!is_exact_value_vocabulary("entity_definitions", "tag"));
    }

    // ── The shared membership rule (ADR-0098 D-2/D-3/D-5) ────────────────
    //
    // These exercise `resolve_tag_membership` directly, the way the reducer
    // choke point calls it — no `Manifest` involved. They are the proof that
    // one rule serves both runtimes.

    fn visible<'a>(
        entries: &'a [(&'a str, &'a str, Option<&'a str>, Option<&'a str>)],
    ) -> Vec<VisibleTagPrefix<'a>> {
        entries
            .iter()
            .map(
                |(declaring_bundle, prefix, target_kind, natural_key_field)| VisibleTagPrefix {
                    declaring_bundle,
                    prefix,
                    target_kind: *target_kind,
                    natural_key_field: *natural_key_field,
                },
            )
            .collect()
    }

    fn never_resolves(_: &MembershipQuery<'_>) -> MembershipResolution {
        MembershipResolution::Unresolved
    }

    /// The obstacle sk-8b4 was expected to trip over, proven absent. Colon-
    /// shaped tags (`faction:<side>`, `member_of:<party>`) cannot be declared
    /// as `tag_value_definition` rows — MF-VAL-305 requires a definition to be
    /// a DOTTED child of its prefix. Under ADR-0098 D-2 they need no
    /// declaration and no "genesis vocabulary" carve-out: they match no
    /// dotted prefix, so no closed vocabulary claims them. Live emitters of
    /// these shapes were retired by sk-ntz.3 / sk-ntz.7; the grammar still
    /// admits leftovers.
    #[test]
    fn colon_shaped_kernel_tags_are_claimed_by_no_closed_vocabulary() {
        let entries = visible(&[
            ("srd", "srd", Some("tag_value_definitions"), Some("tag")),
            (
                "srd",
                "srd.ability",
                Some("resource_definitions"),
                Some("resource_definition_id"),
            ),
        ]);
        for tag in ["faction:party", "faction:monsters", "member_of:party_1"] {
            assert_eq!(
                resolve_tag_membership(tag, entries.iter().copied(), &never_resolves),
                TagMembership::Admitted,
                "kernel-formatted '{tag}' must not be claimed by any declared vocabulary",
            );
        }
    }

    /// ADR-0098 D-2, the clause ADR-0018 keeps: a bundle that declared only a
    /// NAMESPACE enumerated nothing, so nothing can be missing from it.
    #[test]
    fn bare_namespace_declaration_stays_open() {
        let entries = visible(&[("demo", "demo", None, None)]);
        assert_eq!(
            resolve_tag_membership(
                "demo.anything_at_all",
                entries.iter().copied(),
                &never_resolves
            ),
            TagMembership::Admitted,
        );
    }

    /// A tag under no declared prefix at all is the open frontier ADR-0018 D-1
    /// exists to protect — MF-VAL-298 stays the advisory check for it.
    #[test]
    fn tag_outside_every_declared_prefix_is_admitted() {
        let entries = visible(&[("demo", "demo", Some("tag_value_definitions"), Some("tag"))]);
        assert_eq!(
            resolve_tag_membership("homebrew.thing", entries.iter().copied(), &never_resolves),
            TagMembership::Admitted,
        );
    }

    /// ADR-0098 D-5: a caller that cannot losslessly reconstruct a
    /// `target_kind`'s natural-key space declines rather than guesses, and a
    /// declined query is ADMITTED — the reducer must never false-reject what
    /// the authoring validator would admit.
    #[test]
    fn deferred_resolution_is_admitted_not_rejected() {
        let entries = visible(&[(
            "demo",
            "demo.kind",
            Some("place_node_definitions"),
            Some("place_node_id"),
        )]);
        assert_eq!(
            resolve_tag_membership("demo.kind.whatever", entries.iter().copied(), &|_| {
                MembershipResolution::Deferred
            }),
            TagMembership::Admitted,
        );
    }

    /// A tag equal to the bare prefix has no suffix to resolve.
    #[test]
    fn tag_equal_to_its_ref_typed_prefix_is_admitted() {
        let entries = visible(&[(
            "demo",
            "demo.kind",
            Some("tag_value_definitions"),
            Some("tag"),
        )]);
        assert_eq!(
            resolve_tag_membership("demo.kind", entries.iter().copied(), &never_resolves),
            TagMembership::Admitted,
        );
    }

    /// Longest match wins, and the verdict does not depend on the order the
    /// caller's vocabulary happens to iterate — the reducer reads `mod_bundle`
    /// in table order, the validator walks a sorted `BundleScope` closure, and
    /// ADR-0024 D-2 needs them to agree.
    #[test]
    fn longest_match_wins_and_is_order_independent() {
        let entries = visible(&[
            ("demo", "demo", None, None),
            (
                "demo",
                "demo.kind",
                Some("tag_value_definitions"),
                Some("tag"),
            ),
        ]);
        let forward = entries.to_vec();
        let reversed: Vec<_> = entries.iter().copied().rev().collect();
        for order in [forward, reversed] {
            match resolve_tag_membership("demo.kind.ghost", order, &never_resolves) {
                TagMembership::Undeclared(query) => {
                    assert_eq!(query.prefix, "demo.kind");
                    assert_eq!(query.suffix, "ghost");
                }
                other => panic!("the specific 'demo.kind' entry must govern, got {other:?}"),
            }
        }
    }

    /// The reason clause the reducer prints is the validator's own — one
    /// condition, one wording, whichever surface catches the tag.
    #[test]
    fn undeclared_reason_names_the_vocabulary_that_rejected_the_tag() {
        let entries = visible(&[(
            "srd",
            "srd.creature_type",
            Some("tag_value_definitions"),
            Some("tag"),
        )]);
        let TagMembership::Undeclared(query) = resolve_tag_membership(
            "srd.creature_type.grue",
            entries.iter().copied(),
            &never_resolves,
        ) else {
            panic!("an undeclared member of a closed vocabulary must be rejected");
        };
        let reason = undeclared_membership_reason(&query, "homebrew");
        assert!(reason.contains("srd.creature_type.grue"), "{reason}");
        assert!(reason.contains("grue"), "{reason}");
        assert!(reason.contains("tag_value_definitions"), "{reason}");
        // The OWNING bundle is named, not the declaring one, for the scope.
        assert!(
            reason.contains("bundle 'homebrew's declared scope"),
            "{reason}"
        );
        assert!(reason.contains("declared by bundle 'srd'"), "{reason}");
    }
}
