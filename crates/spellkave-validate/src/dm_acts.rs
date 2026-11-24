//! ADR-0155: authored DM choices admit scoped vocabulary, never authority tags.
use crate::reference_scope::BundleScope;
pub use crate::refs::party_extents::party_extent_tags;
use crate::refs::MF_VAL_CROSS_BUNDLE_SCOPE;
use crate::{Diagnostic, Severity};
use spellkave_schema::Manifest;
use std::collections::{BTreeMap, BTreeSet};

pub const MF_VAL_DM_SELECTION: &str = "MF-VAL-388";
pub const MF_VAL_DM_AUTHORITY_TAG: &str = "MF-VAL-383";
pub const MF_VAL_DM_TEXT_BOUNDS: &str = "MF-VAL-384";
pub const MF_VAL_DM_OPTIONS: &str = "MF-VAL-385";
pub const MF_VAL_DM_SPAWN_BLUEPRINT: &str = "MF-VAL-386";

pub fn protected_tag(tag: &str, party_tags: &BTreeSet<String>) -> bool {
    tag == spellkave_schema::PLAYER_SEAT_TAG || party_tags.contains(tag)
}

pub fn validate(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    validate_with_world(manifest, None, diags);
}

pub fn validate_with_world(
    manifest: &Manifest,
    live: Option<&crate::refs::LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    validate_for_apply(manifest, manifest, live, diags);
}

/// Reconstructed consequence writes resolve references but are not incoming edits.
pub fn validate_for_apply(
    manifest: &Manifest,
    applying: &Manifest,
    live: Option<&crate::refs::LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    let graphs: BTreeSet<&str> = manifest
        .place_graphs
        .iter()
        .map(|row| row.place_graph_id.as_str())
        .chain(
            live.into_iter()
                .flat_map(|world| world.place_graphs.keys().map(String::as_str)),
        )
        .collect();
    for row in &applying.effect_definitions {
        if !row
            .primitives_csv
            .split(',')
            .any(|primitive| primitive.trim() == "Spawn")
        {
            continue;
        }
        let Ok(params) = serde_json::from_str::<serde_json::Value>(&row.parameters_schema_json)
        else {
            continue;
        };
        if params
            .get("unbound_actor")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && params
                .get("blueprint_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| graphs.contains(id))
        {
            diags.push(fatal(
                MF_VAL_DM_SPAWN_BLUEPRINT,
                &format!(
                    "effect_definitions.{}.parameters_schema_json",
                    row.effect_def_id
                ),
                "Spawn.unbound_actor requires an entity definition, not a place-graph blueprint",
            ));
        }
    }
    let mut party_tags = party_extent_tags(manifest, live);
    if let Some(live) = live {
        party_tags.extend(live.dm.party_tags.iter().cloned());
    }
    let mut tags: BTreeMap<&str, &str> = live
        .into_iter()
        .flat_map(|w| {
            w.dm.tags
                .iter()
                .map(|(id, owner)| (id.as_str(), owner.as_str()))
        })
        .collect();
    tags.extend(
        manifest
            .tag_value_definitions
            .iter()
            .map(|r| (r.tag.as_str(), r.bundle_id.as_str())),
    );
    let mut conditions: BTreeMap<&str, &str> = live
        .into_iter()
        .flat_map(|w| {
            w.dm.conditions
                .iter()
                .map(|(id, owner)| (id.as_str(), owner.as_str()))
        })
        .collect();
    conditions.extend(
        manifest
            .condition_definitions
            .iter()
            .map(|r| (r.condition_definition_id.as_str(), r.bundle_id.as_str())),
    );
    let mut items: BTreeMap<&str, &str> = live
        .into_iter()
        .flat_map(|w| {
            w.dm.items
                .iter()
                .map(|(id, owner)| (id.as_str(), owner.as_str()))
        })
        .collect();
    items.extend(
        manifest
            .entity_definitions
            .iter()
            .map(|r| (r.entity_def_id.as_str(), r.bundle_id.as_str())),
    );
    let owners: BTreeMap<_, _> = manifest
        .consequence_templates
        .iter()
        .map(|r| (r.template_id.as_str(), r.bundle_id.as_str()))
        .collect();
    let scope = BundleScope::build_with_live(
        manifest,
        &live
            .map(|w| w.dm.bundle_dependencies.clone())
            .unwrap_or_default(),
    );
    for row in &applying.consequence_frame_writes {
        let context = SelectionContext {
            owner: owners.get(row.template_id.as_str()).copied().unwrap_or(""),
            scope: &scope,
            tags: &tags,
            conditions: &conditions,
            items: &items,
            party_tags: &party_tags,
        };
        walk(
            &row.frame_write,
            &format!(
                "consequence_frame_writes.{}:{}",
                row.template_id, row.ordinal
            ),
            &context,
            diags,
        );
    }
}

struct SelectionContext<'a> {
    owner: &'a str,
    scope: &'a BundleScope,
    tags: &'a BTreeMap<&'a str, &'a str>,
    conditions: &'a BTreeMap<&'a str, &'a str>,
    items: &'a BTreeMap<&'a str, &'a str>,
    party_tags: &'a BTreeSet<String>,
}
fn fatal(code: &str, path: &str, message: &str) -> Diagnostic {
    Diagnostic {
        code: code.into(),
        severity: Severity::Fatal,
        path: path.into(),
        message: message.into(),
        span: None,
        hint: None,
        details: None,
    }
}
fn selection(
    value: &serde_json::Value,
    vocabulary: &BTreeMap<&str, &str>,
    path: &str,
    authority: bool,
    context: &SelectionContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(ids) = value
        .get("allowed_ids")
        .and_then(serde_json::Value::as_array)
    else {
        diags.push(fatal(
            MF_VAL_DM_SELECTION,
            path,
            "selection requires a nonempty allowed_ids list",
        ));
        return;
    };
    if ids.is_empty() {
        diags.push(fatal(
            MF_VAL_DM_SELECTION,
            path,
            "selection requires a nonempty allowed_ids list",
        ));
    }
    for id in ids {
        let Some(id) = id.as_str().filter(|id| !id.trim().is_empty()) else {
            diags.push(fatal(
                MF_VAL_DM_SELECTION,
                path,
                "selected ids must be nonempty strings",
            ));
            continue;
        };
        match vocabulary.get(id) {
            None => diags.push(fatal(MF_VAL_DM_SELECTION, path, &format!("selected id '{id}' must resolve to a declared definition"))),
            Some(owner) if !context.scope.in_scope(context.owner, owner) => diags.push(fatal(MF_VAL_CROSS_BUNDLE_SCOPE, path, &format!("selected id '{id}' belongs to bundle '{owner}', outside the dependency closure of '{}'", context.owner))),
            Some(_) if authority && protected_tag(id, context.party_tags) => diags.push(fatal(
                MF_VAL_DM_AUTHORITY_TAG, path, &format!("SetEntityTag cannot write authority tag '{id}'"),
            )),
            Some(_) => {}
        }
    }
}
fn walk(
    value: &serde_json::Value,
    path: &str,
    context: &SelectionContext<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(array) = value.as_array() {
        for value in array {
            walk(value, path, context, diags);
        }
        return;
    }
    let Some(map) = value.as_object() else {
        return;
    };
    for (variant, fields) in map {
        match variant.as_str() {
            "SetEntityTag" => selection(&fields["tag"], context.tags, path, true, context, diags),
            "SetEntityText" => {
                if !fields
                    .get("max_chars")
                    .and_then(serde_json::Value::as_u64)
                    .is_some_and(|n| (1..=4096).contains(&n))
                {
                    diags.push(fatal(
                        MF_VAL_DM_TEXT_BOUNDS,
                        path,
                        "SetEntityText.max_chars must be in 1..=4096",
                    ));
                }
            }
            "ApplyStatus" | "RemoveCondition" => {
                if let Some(s) = fields.get("selection").filter(|v| !v.is_null()) {
                    selection(s, context.conditions, path, false, context, diags);
                }
            }
            "InsertItem" | "ActorInventoryItem" => {
                if fields
                    .get("selection")
                    .is_some_and(|value| !value.is_null())
                    && fields
                        .get("item_def_id")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|id| !id.trim().is_empty())
                {
                    diags.push(fatal(
                        MF_VAL_DM_SELECTION,
                        path,
                        "an item selection cannot also provide a literal item_def_id",
                    ));
                }
                if let Some(s) = fields.get("selection").filter(|v| !v.is_null()) {
                    selection(s, context.items, path, false, context, diags);
                }
            }
            "Inscribe" => {
                if let Some(payload) = fields
                    .get("payload_json")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                {
                    if let Some(options) = payload.get("options") {
                        let valid = options.as_array().is_some_and(|labels| {
                            labels.len() <= 16
                                && labels.iter().all(|label| {
                                    label.as_str().is_some_and(|s| s.chars().count() <= 120)
                                })
                        });
                        if !valid {
                            diags.push(fatal(MF_VAL_DM_OPTIONS, path, "literal speech options require at most 16 labels of at most 120 characters"));
                        }
                    }
                }
            }
            _ => {}
        }
        for key in ["body", "source"] {
            if let Some(nested) = fields.get(key) {
                walk(nested, path, context, diags);
            }
        }
    }
}
