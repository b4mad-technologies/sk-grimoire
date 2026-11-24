//! Stage-2 NPC authoring documents. These checks are client/platform admission;
//! the kernel never interprets metadata payloads (ADR-0165 D-1).
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// A versioned payload bound, independent of measured per-role host memory.
pub const MAX_DOCUMENT_BYTES: usize = 256 * 1024;
const SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, serde::Serialize)]
pub struct ReferenceDiagnostic {
    pub severity: &'static str,
    pub code: &'static str,
    pub reference_kind: String,
    pub reference_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bead: Option<String>,
}

fn object<'a>(
    value: &'a Value,
    path: &str,
    allowed: &[&str],
    required: &[&str],
    errors: &mut Vec<String>,
) -> Option<&'a Map<String, Value>> {
    let Some(fields) = value.as_object() else {
        errors.push(format!("{path}: expected object"));
        return None;
    };
    for key in fields.keys() {
        if !allowed.contains(&key.as_str()) {
            errors.push(format!("{path}.{key}: unknown field"));
        }
    }
    for key in required {
        if !fields.contains_key(*key) {
            errors.push(format!("{path}.{key}: required"));
        }
    }
    Some(fields)
}
fn text(value: &Value, path: &str, errors: &mut Vec<String>) {
    if !value.as_str().is_some_and(|v| !v.is_empty()) {
        errors.push(format!("{path}: expected nonempty string"));
    }
}
fn integer(value: &Value, path: &str, errors: &mut Vec<String>) {
    if !value.as_u64().is_some_and(|v| v <= SAFE_INTEGER) {
        errors.push(format!("{path}: expected safe unsigned integer"));
    }
}
fn strings(value: &Value, path: &str, errors: &mut Vec<String>) {
    match value.as_array().filter(|v| !v.is_empty()) {
        Some(values) => {
            for (index, value) in values.iter().enumerate() {
                text(value, &format!("{path}[{index}]"), errors);
            }
        }
        None => errors.push(format!("{path}: expected nonempty string array")),
    }
}
fn lines(value: &Value, path: &str, errors: &mut Vec<String>) {
    if let Some(fields) = object(
        value,
        path,
        &["greeting", "accept", "decline", "complete"],
        &["greeting", "accept", "decline"],
        errors,
    ) {
        for (key, value) in fields {
            text(value, &format!("{path}.{key}"), errors);
        }
    }
}
fn terminal(value: &Value, path: &str, errors: &mut Vec<String>) {
    let Some(fields) = object(
        value,
        path,
        &[
            "kind",
            "deadKeys",
            "anyDeadKeys",
            "flag",
            "escortKey",
            "atNodeId",
            "itemKey",
            "aliveKey",
        ],
        &["kind"],
        errors,
    ) else {
        return;
    };
    for (key, value) in fields {
        if ["deadKeys", "anyDeadKeys"].contains(&key.as_str()) {
            strings(value, &format!("{path}.{key}"), errors);
        } else {
            text(value, &format!("{path}.{key}"), errors);
        }
    }
    let required: &[&str] = match value["kind"].as_str() {
        Some("flag") => &["flag"],
        Some("all_dead") => &["deadKeys"],
        Some("any_dead_or_flag") => {
            if !fields.contains_key("anyDeadKeys") && !fields.contains_key("flag") {
                errors.push(format!("{path}: dead keys or flag required"));
            }
            &[]
        }
        Some("escort_alive_at") => &["escortKey", "atNodeId"],
        Some("has_item") => &["itemKey"],
        Some("all_dead_and_alive") => &["deadKeys", "aliveKey"],
        Some("all_dead_and_flag") => &["deadKeys", "flag"],
        _ => {
            errors.push(format!("{path}.kind: unsupported terminal form"));
            &[]
        }
    };
    // Auxiliary itemKey on a flag terminal is retained for existing consumers.
    for key in required {
        if !fields.contains_key(*key) {
            errors.push(format!("{path}.{key}: required for terminal form"));
        }
    }
}
fn bindings(value: &Value, errors: &mut Vec<String>) {
    let names = [
        "hero_blueprint_id",
        "level_state_id",
        "drafting_condition_id",
        "speech_trace_kind_id",
        "speak_action_id",
        "answer_count",
        "choice_actions",
        "open_actions",
        "prompt_actions",
        "flag_state_ids",
    ];
    let Some(fields) = object(value, "bindings", &names, &names, errors) else {
        return;
    };
    for name in &names[..5] {
        text(&value[name], &format!("bindings.{name}"), errors);
    }
    if value["answer_count"] != 2 {
        errors.push("bindings.answer_count: exact quest-board/v1 requires two answers".into());
    }
    for name in ["choice_actions", "open_actions", "prompt_actions"] {
        let Some(entries) = fields[name]
            .as_object()
            .filter(|entries| !entries.is_empty())
        else {
            errors.push(format!("bindings.{name}: expected nonempty map"));
            continue;
        };
        for (key, entry) in entries {
            if name == "choice_actions" {
                integer(entry, &format!("bindings.{name}.{key}"), errors);
            } else {
                if key.parse::<u64>().ok().is_none_or(|id| id > 16) {
                    errors.push(format!(
                        "bindings.{name}.{key}: expected bounded palette key"
                    ));
                }
                text(entry, &format!("bindings.{name}.{key}"), errors);
            }
        }
    }
    match fields["flag_state_ids"].as_object() {
        Some(flags) => {
            for (name, reference) in flags {
                if name.is_empty() {
                    errors.push("bindings.flag_state_ids: empty symbol".into());
                }
                text(
                    reference,
                    &format!("bindings.flag_state_ids.{name}"),
                    errors,
                );
            }
        }
        None => errors.push("bindings.flag_state_ids: expected map".into()),
    }
}

/// Shape admission shared by compile/publish/activation callers. Source parsing
/// is owned by world-author; this function accepts JSON, never adventure TOML.
/// Reference closure against the compiled world is a separate required gate.
pub fn validate_quest_source_json(input: &str) -> Vec<String> {
    if input.len() > MAX_DOCUMENT_BYTES {
        return vec!["NPC document exceeds version-1 byte bound".into()];
    }
    let value: Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(_) => return vec!["NPC document is invalid JSON".into()],
    };
    let mut errors = Vec::new();
    let root = [
        "schema_version",
        "bundle_id",
        "catalog_namespace",
        "catalog_read_capability_id",
        "publisher_reader_role_id",
        "publisher_reader_binding_id",
        "known_missing_flags",
        "bindings",
        "quests",
        "givers",
    ];
    if object(&value, "source", &root, &root, &mut errors).is_none() {
        return errors;
    }
    if value["schema_version"] != 1 {
        errors.push("source.schema_version: unsupported version".into());
    }
    for name in &root[1..6] {
        text(&value[name], &format!("source.{name}"), &mut errors);
    }
    bindings(&value["bindings"], &mut errors);
    match value["known_missing_flags"].as_object() {
        Some(flags) => {
            for (reference, bead) in flags {
                if reference.is_empty() {
                    errors.push("known_missing_flags: empty reference".into());
                }
                text(bead, "known_missing_flags bead", &mut errors);
            }
        }
        None => errors.push("known_missing_flags: expected map".into()),
    }
    let quests = validate_quests(&value, true, &mut errors);
    let giver_fields = [
        "actor_key",
        "npc_kind",
        "schema_version",
        "namespace",
        "read_capability_id",
        "quest_refs",
        "hooks",
    ];
    let mut audiences =
        BTreeSet::from([value["catalog_read_capability_id"].as_str().unwrap_or("")]);
    let mut namespaces = BTreeSet::from([value["catalog_namespace"].as_str().unwrap_or("")]);
    let mut actors = BTreeSet::new();
    match value["givers"].as_array().filter(|rows| !rows.is_empty()) {
        Some(givers) => {
            for (index, giver) in givers.iter().enumerate() {
                let path = format!("givers[{index}]");
                if object(giver, &path, &giver_fields, &giver_fields, &mut errors).is_none() {
                    continue;
                }
                if giver["npc_kind"] != "quest-giver" {
                    errors.push(format!("{path}.npc_kind: unsupported kind"));
                }
                if giver["schema_version"] != 1 {
                    errors.push(format!("{path}.schema_version: unsupported version"));
                }
                if !giver["hooks"].as_object().is_some_and(Map::is_empty) {
                    errors.push(format!(
                        "{path}.hooks: quest-giver stage 2 admits no hook slots"
                    ));
                }
                for name in ["actor_key", "namespace", "read_capability_id"] {
                    text(&giver[name], &format!("{path}.{name}"), &mut errors);
                }
                if !audiences.insert(giver["read_capability_id"].as_str().unwrap_or(""))
                    || !namespaces.insert(giver["namespace"].as_str().unwrap_or(""))
                    || !actors.insert(giver["actor_key"].as_str().unwrap_or(""))
                {
                    errors.push(format!("{path}: duplicate actor, audience or namespace"));
                }
                strings(
                    &giver["quest_refs"],
                    &format!("{path}.quest_refs"),
                    &mut errors,
                );
                let mut refs = BTreeSet::new();
                if let Some(ids) = giver["quest_refs"].as_array() {
                    for id in ids {
                        match id.as_str().and_then(|id| quests.get(id)) {
                            Some(quest)
                                if quest["giverKey"] == giver["actor_key"]
                                    && refs.insert(id.as_str().unwrap()) => {}
                            _ => errors.push(format!(
                                "{path}.quest_refs: unknown, duplicate or foreign quest"
                            )),
                        }
                    }
                }
            }
        }
        None => errors.push("source.givers: expected nonempty array".into()),
    }
    errors
}

fn runtime_document(input: &str) -> Result<Value, Vec<String>> {
    if input.len() > MAX_DOCUMENT_BYTES {
        return Err(vec!["NPC document exceeds version-1 byte bound".into()]);
    }
    serde_json::from_str(input).map_err(|_| vec!["NPC document is invalid JSON".into()])
}

/// Runtime catalog admission shares the source's quest/binding rules. The
/// catalog intentionally has no prompt or spoken prose. Hash/target binding is
/// checked by the assigned host after this language-neutral shape admission.
pub fn validate_quest_catalog_json(input: &str) -> Vec<String> {
    let value = match runtime_document(input) {
        Ok(value) => value,
        Err(errors) => return errors,
    };
    let mut errors = vec![];
    let root = [
        "schema_version",
        "bindings",
        "quests",
        "content_hash",
        "publication_id",
    ];
    if object(&value, "catalog", &root, &root, &mut errors).is_none() {
        return errors;
    }
    if value["schema_version"] != 1 {
        errors.push("catalog.schema_version: unsupported version".into());
    }
    for field in ["content_hash", "publication_id"] {
        text(&value[field], field, &mut errors);
    }
    let runtime = &value["bindings"];
    let runtime_fields = [
        "hero_blueprint_id",
        "level_state_id",
        "drafting_condition_id",
        "answer_count",
        "vocabulary",
        "flag_state_ids",
    ];
    object(
        runtime,
        "catalog.bindings",
        &runtime_fields,
        &runtime_fields,
        &mut errors,
    );
    let vocabulary = &runtime["vocabulary"];
    let vocabulary_fields = [
        "speechTraceKindId",
        "speakActionId",
        "choiceActions",
        "openActions",
        "promptActions",
    ];
    object(
        vocabulary,
        "catalog.bindings.vocabulary",
        &vocabulary_fields,
        &vocabulary_fields,
        &mut errors,
    );
    let flattened = serde_json::json!({
        "hero_blueprint_id":runtime["hero_blueprint_id"],
        "level_state_id":runtime["level_state_id"],
        "drafting_condition_id":runtime["drafting_condition_id"],
        "answer_count":runtime["answer_count"],
        "speech_trace_kind_id":vocabulary["speechTraceKindId"],
        "speak_action_id":vocabulary["speakActionId"],
        "choice_actions":vocabulary["choiceActions"],
        "open_actions":vocabulary["openActions"],
        "prompt_actions":vocabulary["promptActions"],
        "flag_state_ids":runtime["flag_state_ids"],
    });
    bindings(&flattened, &mut errors);
    validate_quests(&value, false, &mut errors);
    errors
}

/// Private giver documents expose only this giver's lines and ordered refs.
/// The host must additionally join these refs to the admitted catalog and body.
pub fn validate_quest_giver_document_json(input: &str) -> Vec<String> {
    let value = match runtime_document(input) {
        Ok(value) => value,
        Err(errors) => return errors,
    };
    let mut errors = vec![];
    let root = [
        "schema_version",
        "npc_kind",
        "adapter_id",
        "catalog_content_hash",
        "quest_refs",
        "lines",
        "hooks",
        "content_hash",
        "publication_id",
    ];
    if object(&value, "giver", &root, &root, &mut errors).is_none() {
        return errors;
    }
    if value["schema_version"] != 1 {
        errors.push("giver.schema_version: unsupported version".into());
    }
    if value["npc_kind"] != "quest-giver" {
        errors.push("giver.npc_kind: unsupported kind".into());
    }
    if value["adapter_id"] != "quest-board/v1" {
        errors.push("giver.adapter_id: unsupported adapter".into());
    }
    for field in ["catalog_content_hash", "content_hash", "publication_id"] {
        text(&value[field], field, &mut errors);
    }
    if !value["hooks"].as_object().is_some_and(Map::is_empty) {
        errors.push("giver.hooks: stage 2 admits no hook slots".into());
    }
    strings(&value["quest_refs"], "giver.quest_refs", &mut errors);
    let Some(prose) = value["lines"].as_object() else {
        errors.push("giver.lines: expected object".into());
        return errors;
    };
    let mut refs = BTreeSet::new();
    if let Some(quests) = value["quest_refs"].as_array() {
        for quest in quests {
            if let Some(id) = quest.as_str() {
                if !refs.insert(id) {
                    errors.push("giver.quest_refs: duplicate".into());
                }
                lines(
                    &value["lines"][id],
                    &format!("giver.lines.{id}"),
                    &mut errors,
                );
            }
        }
    }
    if prose.keys().any(|key| !refs.contains(key.as_str())) {
        errors.push("giver.lines: contains an unreferenced quest".into());
    }
    errors
}

fn validate_quests<'a>(
    value: &'a Value,
    include_lines: bool,
    errors: &mut Vec<String>,
) -> std::collections::BTreeMap<&'a str, &'a Value> {
    let mut quests = std::collections::BTreeMap::new();
    let mut quest_fields = vec![
        "id",
        "name",
        "rung",
        "giverKey",
        "giverName",
        "acceptSlot",
        "declineSlot",
        "slotAcceptText",
        "slotDeclineText",
        "promptId",
        "objective",
        "terminal",
        "rewardXp",
        "bonusXp",
        "bonusFlag",
        "killXpCreatures",
        "minimum_level",
        "progress_home",
        "progress_state_id",
        "rewarded_state_id",
        "awarded_state_id",
        "acceptance_action_id",
        "award_action_id",
        "settle_action_id",
        "prompt_lines",
        "spoken_lines",
    ];
    if !include_lines {
        quest_fields.retain(|name| !["prompt_lines", "spoken_lines"].contains(name));
    }
    let required: Vec<_> = quest_fields
        .iter()
        .copied()
        .filter(|name| !["bonusXp", "bonusFlag"].contains(name))
        .collect();
    let Some(rows) = value["quests"].as_array().filter(|rows| !rows.is_empty()) else {
        errors.push("source.quests: expected nonempty array".into());
        return quests;
    };
    let mut prompts = BTreeSet::new();
    for (index, quest) in rows.iter().enumerate() {
        let path = format!("quests[{index}]");
        if object(quest, &path, &quest_fields, &required, errors).is_none() {
            continue;
        }
        for name in [
            "id",
            "name",
            "rung",
            "giverKey",
            "giverName",
            "slotAcceptText",
            "slotDeclineText",
            "objective",
            "progress_state_id",
            "rewarded_state_id",
            "awarded_state_id",
            "acceptance_action_id",
            "award_action_id",
            "settle_action_id",
        ] {
            text(&quest[name], &format!("{path}.{name}"), errors);
        }
        for name in [
            "acceptSlot",
            "declineSlot",
            "promptId",
            "rewardXp",
            "minimum_level",
        ] {
            integer(&quest[name], &format!("{path}.{name}"), errors);
        }
        if quest["acceptSlot"] == quest["declineSlot"]
            || ![1, 2].contains(&quest["acceptSlot"].as_u64().unwrap_or(0))
            || ![1, 2].contains(&quest["declineSlot"].as_u64().unwrap_or(0))
        {
            errors.push(format!("{path}: invalid accept/decline slots"));
        }
        let prompt = quest["promptId"].as_u64().unwrap_or(0);
        if !(1..=16).contains(&prompt) || !prompts.insert(prompt) {
            errors.push(format!(
                "{path}.promptId: invalid or duplicate legacy prompt"
            ));
        }
        if ![Some("self"), Some("visitor")].contains(&quest["progress_home"].as_str()) {
            errors.push(format!("{path}.progress_home: expected self or visitor"));
        }
        if let Some(v) = quest.get("bonusXp") {
            integer(v, &format!("{path}.bonusXp"), errors);
        }
        if let Some(v) = quest.get("bonusFlag") {
            text(v, &format!("{path}.bonusFlag"), errors);
        }
        terminal(&quest["terminal"], &format!("{path}.terminal"), errors);
        if include_lines {
            lines(
                &quest["prompt_lines"],
                &format!("{path}.prompt_lines"),
                errors,
            );
            lines(
                &quest["spoken_lines"],
                &format!("{path}.spoken_lines"),
                errors,
            );
        }
        match quest["killXpCreatures"].as_array() {
            Some(creatures) => {
                for creature in creatures {
                    if object(
                        creature,
                        &format!("{path}.killXpCreatures"),
                        &["slug", "count"],
                        &["slug", "count"],
                        errors,
                    )
                    .is_some()
                    {
                        text(&creature["slug"], &path, errors);
                        integer(&creature["count"], &path, errors);
                    }
                }
            }
            None => errors.push(format!("{path}.killXpCreatures: expected array")),
        }
        if let Some(id) = quest["id"].as_str() {
            if quests.insert(id, quest).is_some() {
                errors.push(format!("{path}.id: duplicate"));
            }
        }
    }
    quests
}

/// Compile/publish reference closure against canonical world and dependency
/// manifests. Runtime activation cannot re-prove this through a scoped seat
/// cache, and must not interpret an undisclosed definition as a bad reference.
pub fn validate_quest_references_json(source: &str, worlds: &str) -> Vec<ReferenceDiagnostic> {
    let shape = validate_quest_source_json(source);
    if !shape.is_empty() {
        return shape
            .into_iter()
            .map(|error| ReferenceDiagnostic {
                severity: "error",
                code: "npc-source-invalid",
                reference_kind: "source".into(),
                reference_id: error,
                bead: None,
            })
            .collect();
    }
    let source: Value = serde_json::from_str(source).expect("source admitted");
    let worlds: Vec<Value> = match serde_json::from_str::<Vec<Value>>(worlds) {
        Ok(worlds) if !worlds.is_empty() && worlds.iter().all(Value::is_object) => worlds,
        _ => {
            return vec![ReferenceDiagnostic {
                severity: "error",
                code: "npc-reference-worlds-invalid",
                reference_kind: "world".into(),
                reference_id: String::new(),
                bead: None,
            }]
        }
    };
    let collect = |table: &str, key: &str| -> BTreeSet<String> {
        worlds
            .iter()
            .flat_map(|world| world[table].as_array().into_iter().flatten())
            .filter_map(|row| row[key].as_str().map(str::to_owned))
            .collect()
    };
    let mut states = collect("resource_definitions", "resource_definition_id");
    states.extend(collect("object_state_definitions", "state_id"));
    let entities = collect("entities", "entity_key");
    let places = collect("place_nodes", "node_id");
    let actions = collect("action_definitions", "action_id");
    let definitions = collect("entity_definitions", "entity_def_id");
    let conditions = collect("condition_definitions", "condition_definition_id");
    let traces = collect("trace_kind_definitions", "kind_id");
    let bundles = collect("bundles", "bundle_id");
    let flags = source["bindings"]["flag_state_ids"]
        .as_object()
        .expect("admitted flags");
    let exceptions = source["known_missing_flags"]
        .as_object()
        .expect("admitted exceptions");
    let mut seen = BTreeSet::new();
    let mut diagnostics = vec![];
    let mut check = |kind: &str, id: &str, available: &BTreeSet<String>, flag: bool| {
        if !available.contains(id) && seen.insert((kind.to_owned(), id.to_owned())) {
            let bead = flag
                .then(|| exceptions.get(id).and_then(Value::as_str))
                .flatten();
            diagnostics.push(ReferenceDiagnostic {
                severity: if bead.is_some() {
                    "known-missing"
                } else {
                    "error"
                },
                code: if bead.is_some() {
                    "npc-reference-known-missing"
                } else {
                    "npc-reference-missing"
                },
                reference_kind: kind.into(),
                reference_id: id.into(),
                bead: bead.map(str::to_owned),
            });
        }
    };
    check(
        "bundle",
        source["bundle_id"].as_str().unwrap(),
        &bundles,
        false,
    );
    let bindings = &source["bindings"];
    check(
        "entity_definition",
        bindings["hero_blueprint_id"].as_str().unwrap(),
        &definitions,
        false,
    );
    check(
        "object_state_definition",
        bindings["level_state_id"].as_str().unwrap(),
        &states,
        false,
    );
    check(
        "condition_definition",
        bindings["drafting_condition_id"].as_str().unwrap(),
        &conditions,
        false,
    );
    check(
        "trace_kind_definition",
        bindings["speech_trace_kind_id"].as_str().unwrap(),
        &traces,
        false,
    );
    check(
        "action",
        bindings["speak_action_id"].as_str().unwrap(),
        &actions,
        false,
    );
    for id in bindings["choice_actions"].as_object().unwrap().keys() {
        check("action", id, &actions, false);
    }
    for palette in ["open_actions", "prompt_actions"] {
        for id in bindings[palette].as_object().unwrap().values() {
            check("action", id.as_str().unwrap(), &actions, false);
        }
    }
    let mut used_flags = BTreeSet::new();
    for quest in source["quests"].as_array().unwrap() {
        check(
            "entity",
            quest["giverKey"].as_str().unwrap(),
            &entities,
            false,
        );
        for field in ["progress_state_id", "rewarded_state_id", "awarded_state_id"] {
            check(
                "object_state_definition",
                quest[field].as_str().unwrap(),
                &states,
                false,
            );
        }
        for field in [
            "acceptance_action_id",
            "award_action_id",
            "settle_action_id",
        ] {
            check("action", quest[field].as_str().unwrap(), &actions, false);
        }
        let terminal = &quest["terminal"];
        for field in ["itemKey", "escortKey", "aliveKey"] {
            if let Some(id) = terminal[field].as_str() {
                check("entity", id, &entities, false);
            }
        }
        for field in ["deadKeys", "anyDeadKeys"] {
            if let Some(ids) = terminal[field].as_array() {
                for id in ids {
                    check("entity", id.as_str().unwrap(), &entities, false);
                }
            }
        }
        if let Some(id) = terminal["atNodeId"].as_str() {
            check("place_node", id, &places, false);
        }
        for symbol in [terminal["flag"].as_str(), quest["bonusFlag"].as_str()]
            .into_iter()
            .flatten()
        {
            if let Some(reference) = flags.get(symbol).and_then(Value::as_str) {
                used_flags.insert(reference.to_owned());
                check("object_state_definition", reference, &states, true);
            } else {
                check("flag_binding", symbol, &BTreeSet::new(), false);
            }
        }
    }
    for giver in source["givers"].as_array().unwrap() {
        check(
            "entity",
            giver["actor_key"].as_str().unwrap(),
            &entities,
            false,
        );
    }
    drop(check);
    for reference in exceptions.keys() {
        if !used_flags.contains(reference) || states.contains(reference) {
            diagnostics.push(ReferenceDiagnostic {
                severity: "error",
                code: "npc-reference-exception-unused",
                reference_kind: "object_state_definition".into(),
                reference_id: reference.clone(),
                bead: exceptions[reference].as_str().map(str::to_owned),
            });
        }
    }
    for quest in source["quests"].as_array().unwrap() {
        let prompt = quest["promptId"].as_u64().unwrap().to_string();
        if !bindings["prompt_actions"]
            .as_object()
            .unwrap()
            .contains_key(&prompt)
        {
            diagnostics.push(ReferenceDiagnostic {
                severity: "error",
                code: "npc-prompt-binding-missing",
                reference_kind: "prompt_binding".into(),
                reference_id: prompt,
                bead: None,
            });
        }
    }
    for key in ["0", "2"] {
        if !bindings["open_actions"]
            .as_object()
            .unwrap()
            .contains_key(key)
        {
            diagnostics.push(ReferenceDiagnostic {
                severity: "error",
                code: "npc-open-binding-missing",
                reference_kind: "open_binding".into(),
                reference_id: key.into(),
                bead: None,
            });
        }
    }
    if !bindings["prompt_actions"]
        .as_object()
        .unwrap()
        .contains_key("0")
    {
        diagnostics.push(ReferenceDiagnostic {
            severity: "error",
            code: "npc-prompt-binding-missing",
            reference_kind: "prompt_binding".into(),
            reference_id: "0".into(),
            bead: None,
        });
    }
    let choices: BTreeSet<_> = bindings["choice_actions"]
        .as_object()
        .unwrap()
        .values()
        .filter_map(Value::as_u64)
        .collect();
    if choices != BTreeSet::from([1, 2])
        || bindings["choice_actions"].as_object().unwrap().len() != 2
    {
        diagnostics.push(ReferenceDiagnostic {
            severity: "error",
            code: "npc-choice-binding-invalid",
            reference_kind: "choice_binding".into(),
            reference_id: String::new(),
            bead: None,
        });
    }
    diagnostics
}
