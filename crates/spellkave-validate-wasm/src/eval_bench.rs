use std::collections::BTreeMap;

use serde::Serialize;
use spellkave_dsl::{
    compile_int_expression_with_macros, compile_predicate_with_macros,
    compile_text_expression_with_macros, evaluate_expression_traced, BindingFact, BindingRef,
    ChannelPoolFact, ComposedRollBias, DeterministicRng, DslMacroDefinition, EvaluationContext,
    EvaluationError, EvaluationLimits, EvaluationValue, EventRef, Expression, ImpliedByCountFact,
    ItemDslSourceFact, OccupantsAtFact, ParticipantTagGroupCountFact, PeersNearTargetFact,
    RollModifier, RollTableEntryFact, StochasticDraw, SubjectBoolFact, SubjectIdFact,
    SubjectIntFact, SubjectRef, SubjectResourceBoundFact, SubjectTextFact, TraceEntry,
};

#[derive(Debug, PartialEq, Eq)]
pub enum AuthorFactsError {
    InvalidJson(String),
    UnknownSubject {
        subject: String,
    },
    UnknownPool {
        pool: String,
        subject: Option<String>,
    },
    InvalidFactValue {
        message: String,
    },
    ItemSourceCompileError {
        subject: String,
        slot: String,
        message: String,
    },
}

impl std::fmt::Display for AuthorFactsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(msg) => write!(f, "invalid facts JSON: {msg}"),
            Self::UnknownSubject { subject } => write!(f, "unknown subject '{subject}'"),
            Self::UnknownPool { pool, subject } => {
                if let Some(sub) = subject {
                    write!(f, "unknown pool '{pool}' for subject '{sub}'")
                } else {
                    write!(f, "unknown pool '{pool}'")
                }
            }
            Self::InvalidFactValue { message } => write!(f, "invalid fact value: {message}"),
            Self::ItemSourceCompileError {
                subject,
                slot,
                message,
            } => {
                write!(
                    f,
                    "failed to compile item source slot '{slot}' for subject '{subject}': {message}"
                )
            }
        }
    }
}

impl std::error::Error for AuthorFactsError {}

#[derive(Clone, Debug, Serialize)]
pub struct DslDiagnosticJson {
    pub code: String,
    pub message: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvaluationRejection {
    pub kind: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw_number: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macro_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<DslDiagnosticJson>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvaluationResultJson {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<EvaluationValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draws: Option<Vec<StochasticDraw>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<TraceEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejection: Option<EvaluationRejection>,
}

/// Parses an authored subject key into a canonical `SubjectRef`.
pub fn parse_subject_ref(key: &str) -> Result<SubjectRef, AuthorFactsError> {
    let lower = key.to_ascii_lowercase();
    match lower.as_str() {
        "actor" => Ok(SubjectRef::Actor),
        "target" => Ok(SubjectRef::Target),
        "location" => Ok(SubjectRef::Location),
        "session" => Ok(SubjectRef::Session),
        "world" => Ok(SubjectRef::World),
        "actor.wielded_item" | "wielded_item" | "actorwieldeditem" => {
            Ok(SubjectRef::ActorWieldedItem)
        }
        "event" | "$event" => Ok(SubjectRef::Event(EventRef::Kind)),
        _ => {
            // Check for equipped_item patterns:
            // "actor.equipped_item:<slot>", "equipped_item:<slot>",
            // "actor.equipped_item('<slot>')", "equipped_item('<slot>')",
            // "actor.equipped_item.<slot>", "equipped_item.<slot>"
            if let Some(slot) = extract_slot_id(key, "actor.equipped_item")
                .or_else(|| extract_slot_id(key, "equipped_item"))
            {
                return Ok(SubjectRef::ActorEquippedItem {
                    slot_id: slot.to_string(),
                });
            }

            // Check for binding patterns:
            // "binding:<name>", "bindings:<name>", "binding.<name>", "bindings.<name>"
            if let Some(name) = extract_binding_name(key) {
                return Ok(SubjectRef::Binding(BindingRef {
                    name: name.to_string(),
                }));
            }

            Err(AuthorFactsError::UnknownSubject {
                subject: key.to_string(),
            })
        }
    }
}

fn extract_slot_id<'a>(key: &'a str, prefix: &str) -> Option<&'a str> {
    if !key.starts_with(prefix) {
        return None;
    }
    let rest = &key[prefix.len()..];
    if rest.starts_with(':') || rest.starts_with('.') {
        let slot = &rest[1..];
        if !slot.is_empty() {
            return Some(slot);
        }
    } else if (rest.starts_with("('") && rest.ends_with("')"))
        || (rest.starts_with("(\"") && rest.ends_with("\")"))
    {
        return Some(&rest[2..rest.len() - 2]);
    }
    None
}

fn extract_binding_name(key: &str) -> Option<&str> {
    for prefix in &["binding:", "bindings:", "binding.", "bindings."] {
        if let Some(rest) = key.strip_prefix(prefix) {
            if !rest.is_empty() {
                return Some(rest);
            }
        }
    }
    None
}

/// Converts a canonical `SubjectRef` into its authored DSL string representation.
pub fn subject_to_string(subject: &SubjectRef) -> String {
    match subject {
        SubjectRef::Actor => "actor".to_string(),
        SubjectRef::Target => "target".to_string(),
        SubjectRef::Location => "location".to_string(),
        SubjectRef::Session => "session".to_string(),
        SubjectRef::World => "world".to_string(),
        SubjectRef::ActorWieldedItem => "actor.wielded_item".to_string(),
        SubjectRef::ActorEquippedItem { slot_id } => {
            format!("actor.equipped_item:{slot_id}")
        }
        SubjectRef::Binding(b) => format!("binding:{}", b.name),
        SubjectRef::Event(e) => match e {
            EventRef::Kind => "event.kind".to_string(),
            EventRef::Detail { field_id } => format!("event.detail:{field_id}"),
        },
    }
}

/// Maps grouped author facts to an `EvaluationContext`.
///
/// Builds the context by naming EVERY field explicitly with NO `..`.
/// Rejects any unknown subject or unknown pool by name.
pub fn build_evaluation_context(
    facts_val: &serde_json::Value,
    macros: &[DslMacroDefinition],
    manifest_roll_tables: &[RollTableEntryFact],
) -> Result<EvaluationContext, AuthorFactsError> {
    let mut bool_fields = Vec::<SubjectBoolFact>::new();
    let mut int_fields = Vec::<SubjectIntFact>::new();
    let mut effective_int_fields = Vec::<SubjectIntFact>::new();
    let mut resource_bounds = Vec::<SubjectResourceBoundFact>::new();
    let mut text_fields = Vec::<SubjectTextFact>::new();
    let mut capabilities = Vec::<SubjectIdFact>::new();
    let mut items = Vec::<SubjectIdFact>::new();
    let mut niches = Vec::<SubjectIdFact>::new();
    let mut tags = Vec::<SubjectIdFact>::new();
    let mut statuses = Vec::<SubjectIdFact>::new();
    let mut traces = Vec::<SubjectIdFact>::new();
    let mut known_entities = Vec::<SubjectIdFact>::new();
    let mut bindings = Vec::<BindingFact>::new();
    let mut roll_table_entries_map = BTreeMap::<String, Vec<RollTableEntryFact>>::new();
    let mut item_dsl_sources = Vec::<ItemDslSourceFact>::new();
    let mut implied_by_counts = Vec::<ImpliedByCountFact>::new();
    let mut participant_tag_group_counts = Vec::<ParticipantTagGroupCountFact>::new();
    let mut peers_near_target = Vec::<PeersNearTargetFact>::new();
    let mut occupants_at = Vec::<OccupantsAtFact>::new();
    let mut channel_pools = Vec::<ChannelPoolFact>::new();
    let mut event_kind: Option<String> = None;
    let mut roll_modifier_bias: Option<ComposedRollBias> = None;

    // Seed manifest roll tables
    for entry in manifest_roll_tables {
        roll_table_entries_map
            .entry(entry.table_id.clone())
            .or_default()
            .push(entry.clone());
    }

    let obj = facts_val
        .as_object()
        .ok_or_else(|| AuthorFactsError::InvalidFactValue {
            message: "facts must be an object".to_string(),
        })?;
    for (key, val) in obj {
        match key.as_str() {
            "roll_tables" => {
                if let Some(tables_obj) = val.as_object() {
                    for (table_id, entries_val) in tables_obj {
                        let mut table_facts = Vec::new();
                        let entries_arr = entries_val.as_array().ok_or_else(|| {
                            AuthorFactsError::InvalidFactValue {
                                message: format!(
                                    "roll_tables.{table_id} must be an array of entries"
                                ),
                            }
                        })?;
                        for (i, entry_val) in entries_arr.iter().enumerate() {
                            let entry_obj = entry_val.as_object().ok_or_else(|| {
                                AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "roll_tables.{table_id}[{i}] must be an object"
                                    ),
                                }
                            })?;
                            let entry_id = entry_obj
                                            .get("entry_id")
                                            .or_else(|| entry_obj.get("id"))
                                            .map(|v| v.as_str().map(str::to_string).ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                                message: format!("roll_tables.{table_id}[{i}].entry_id must be a string"),
                                            }))
                                            .transpose()?
                                            .unwrap_or_else(|| format!("{table_id}:{i}"));
                            let weight = entry_obj
                                            .get("weight")
                                            .map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                                message: format!("roll_tables.{table_id}[{i}].weight must be an unsigned 32-bit integer"),
                                            }))
                                            .transpose()?
                                            .unwrap_or(1);
                            let result_ref = entry_obj
                                            .get("result_ref")
                                            .or_else(|| entry_obj.get("value"))
                                            .map(|v| match v {
                                                serde_json::Value::String(s) => Ok(s.clone()),
                                                serde_json::Value::Number(n) => Ok(n.to_string()),
                                                _ => Err(AuthorFactsError::InvalidFactValue {
                                                    message: format!("roll_tables.{table_id}[{i}].result_ref must be a string or number"),
                                                }),
                                            })
                                            .transpose()?
                                            .unwrap_or_default();
                            table_facts.push(RollTableEntryFact {
                                table_id: table_id.clone(),
                                entry_id,
                                weight,
                                result_ref,
                            });
                        }
                        // Supplied roll tables take precedence over manifest
                        roll_table_entries_map.insert(table_id.clone(), table_facts);
                    }
                } else {
                    return Err(AuthorFactsError::InvalidFactValue {
                        message: "roll_tables must be an object of table_id -> entries".to_string(),
                    });
                }
            }
            "item_sources" | "item_dsl_sources" => {
                let sources_obj =
                    val.as_object()
                        .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                            message: "item_sources must be an object of subject -> slots"
                                .to_string(),
                        })?;
                for (sub_key, slots_val) in sources_obj {
                    let subject = parse_subject_ref(sub_key)?;
                    let slots_obj = slots_val.as_object().ok_or_else(|| {
                        AuthorFactsError::InvalidFactValue {
                            message: format!(
                                "item_sources.{sub_key} must be an object of slot -> DSL text"
                            ),
                        }
                    })?;
                    for (slot, source_val) in slots_obj {
                        let dsl_text = source_val.as_str().ok_or_else(|| {
                            AuthorFactsError::InvalidFactValue {
                                message: format!(
                                    "item source slot '{slot}' must be a DSL text string"
                                ),
                            }
                        })?;
                        let expr = compile_int_expression_with_macros(dsl_text, macros).map_err(
                            |failure| {
                                let msg = failure
                                    .diagnostics
                                    .iter()
                                    .map(|d| format!("{}: {}", d.code, d.message))
                                    .collect::<Vec<_>>()
                                    .join("; ");
                                AuthorFactsError::ItemSourceCompileError {
                                    subject: sub_key.clone(),
                                    slot: slot.clone(),
                                    message: msg,
                                }
                            },
                        )?;
                        item_dsl_sources.push(ItemDslSourceFact {
                            subject: subject.clone(),
                            slot: slot.clone(),
                            expr,
                        });
                    }
                }
            }
            "roll_modifier_bias" | "roll_bias" => {
                let bias_obj =
                    val.as_object()
                        .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                            message: format!("{key} must be an object"),
                        })?;
                let flat_bonus = bias_obj
                    .get("flat_bonus")
                    .map(|v| {
                        v.as_i64()
                            .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                message: format!("{key}.flat_bonus must be an integer"),
                            })
                    })
                    .transpose()?
                    .unwrap_or(0);
                let roll_modifier = bias_obj
                    .get("roll_modifier")
                    .map(|v| match v.as_str().map(|s| s.to_ascii_lowercase()) {
                        Some(s) if s == "advantage" => Ok(RollModifier::Advantage),
                        Some(s) if s == "disadvantage" => Ok(RollModifier::Disadvantage),
                        _ => Err(AuthorFactsError::InvalidFactValue {
                            message: format!(
                                "{key}.roll_modifier must be 'advantage' or 'disadvantage'"
                            ),
                        }),
                    })
                    .transpose()?;
                roll_modifier_bias = Some(ComposedRollBias {
                    flat_bonus,
                    roll_modifier,
                });
            }
            "event_kind" => {
                event_kind = Some(
                    val.as_str()
                        .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                            message: "event_kind must be a string".to_string(),
                        })?
                        .to_string(),
                );
            }
            "bindings" => {
                let b_obj = val
                    .as_object()
                    .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                        message: "bindings must be an object".to_string(),
                    })?;
                for (name, b_val) in b_obj {
                    let value_str = match b_val {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => {
                            return Err(AuthorFactsError::InvalidFactValue {
                                message: format!(
                                    "bindings.{name} must be a string, number, or boolean"
                                ),
                            })
                        }
                    };
                    bindings.push(BindingFact {
                        name: name.clone(),
                        value: value_str,
                    });
                }
            }
            "implied_by_counts" => {
                let map = val
                    .as_object()
                    .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                        message: "implied_by_counts must be an object".to_string(),
                    })?;
                for (cond_id, c_val) in map {
                    let count =
                        c_val
                            .as_i64()
                            .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                message: format!("implied_by_counts.{cond_id} must be an integer"),
                            })?;
                    implied_by_counts.push(ImpliedByCountFact {
                        condition_definition_id: cond_id.clone(),
                        count,
                    });
                }
            }
            "participant_tag_group_counts" => {
                let arr = val
                    .as_array()
                    .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                        message: "participant_tag_group_counts must be an array".to_string(),
                    })?;
                for (i, item) in arr.iter().enumerate() {
                    let obj =
                        item.as_object()
                            .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                message: format!(
                                    "participant_tag_group_counts[{i}] must be an object"
                                ),
                            })?;
                    let prefix = obj
                        .get("prefix")
                        .map(|v| {
                            v.as_str()
                                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "participant_tag_group_counts[{i}].prefix must be a string"
                                    ),
                                })
                        })
                        .transpose()?
                        .unwrap_or_default()
                        .to_string();
                    let excluding_condition_definition_id = obj
                                    .get("excluding_condition_definition_id")
                                    .map(|v| v.as_str().map(str::to_string).ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("participant_tag_group_counts[{i}].excluding_condition_definition_id must be a string") }))
                                    .transpose()?;
                    let count = obj.get("count").map(|v| v.as_i64().ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("participant_tag_group_counts[{i}].count must be an integer") })).transpose()?.unwrap_or(0);
                    participant_tag_group_counts.push(ParticipantTagGroupCountFact {
                        prefix,
                        excluding_condition_definition_id,
                        count,
                    });
                }
            }
            "peers_near_target" => {
                let arr = val
                    .as_array()
                    .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                        message: "peers_near_target must be an array".to_string(),
                    })?;
                for (i, item) in arr.iter().enumerate() {
                    let obj =
                        item.as_object()
                            .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                message: format!("peers_near_target[{i}] must be an object"),
                            })?;
                    let prefix = obj
                        .get("prefix")
                        .map(|v| {
                            v.as_str()
                                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "peers_near_target[{i}].prefix must be a string"
                                    ),
                                })
                        })
                        .transpose()?
                        .unwrap_or_default()
                        .to_string();
                    let radius_mm = obj
                        .get("radius_mm")
                        .map(|v| {
                            v.as_i64()
                                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "peers_near_target[{i}].radius_mm must be an integer"
                                    ),
                                })
                        })
                        .transpose()?
                        .unwrap_or(0);
                    let excluding_condition_definition_id = obj
                                    .get("excluding_condition_definition_id")
                                    .map(|v| v.as_str().map(str::to_string).ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("peers_near_target[{i}].excluding_condition_definition_id must be a string") }))
                                    .transpose()?;
                    let count = obj
                        .get("count")
                        .map(|v| {
                            v.as_i64()
                                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "peers_near_target[{i}].count must be an integer"
                                    ),
                                })
                        })
                        .transpose()?
                        .unwrap_or(0);
                    peers_near_target.push(PeersNearTargetFact {
                        prefix,
                        radius_mm,
                        excluding_condition_definition_id,
                        count,
                    });
                }
            }
            "occupants_at" => {
                let arr = val
                    .as_array()
                    .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                        message: "occupants_at must be an array".to_string(),
                    })?;
                for (i, item) in arr.iter().enumerate() {
                    let obj =
                        item.as_object()
                            .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                message: format!("occupants_at[{i}] must be an object"),
                            })?;
                    let scope = obj
                        .get("scope")
                        .map(|v| {
                            v.as_str()
                                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                    message: format!("occupants_at[{i}].scope must be a string"),
                                })
                        })
                        .transpose()?
                        .unwrap_or_default()
                        .to_string();
                    let excluding_condition_definition_id = obj
                                    .get("excluding_condition_definition_id")
                                    .map(|v| v.as_str().map(str::to_string).ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("occupants_at[{i}].excluding_condition_definition_id must be a string") }))
                                    .transpose()?;
                    let count = obj
                        .get("count")
                        .map(|v| {
                            v.as_i64()
                                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                    message: format!("occupants_at[{i}].count must be an integer"),
                                })
                        })
                        .transpose()?
                        .unwrap_or(0);
                    occupants_at.push(OccupantsAtFact {
                        scope,
                        excluding_condition_definition_id,
                        count,
                    });
                }
            }
            "channel_pools" => {
                let map = val
                    .as_object()
                    .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                        message: "channel_pools must be an object".to_string(),
                    })?;
                for (ch_id, c_val) in map {
                    let pool_ms_per_round =
                        c_val
                            .as_i64()
                            .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                message: format!("channel_pools.{ch_id} must be an integer"),
                            })?;
                    channel_pools.push(ChannelPoolFact {
                        channel_id: ch_id.clone(),
                        pool_ms_per_round,
                    });
                }
            }
            _ => {
                // Must be a subject
                let subject = parse_subject_ref(key)?;
                let sub_obj =
                    val.as_object()
                        .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                            message: format!("subject '{key}' facts must be an object of pools"),
                        })?;

                for (pool_key, pool_val) in sub_obj {
                    let pool_lower = pool_key.to_ascii_lowercase();
                    match pool_lower.as_str() {
                        "stat" | "stats" | "int_fields" | "int" | "ints" => {
                            let map = pool_val.as_object().ok_or_else(|| {
                                AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "pool '{pool_key}' of subject '{key}' must be an object"
                                    ),
                                }
                            })?;
                            for (field_id, num_val) in map {
                                let value = num_val.as_i64().ok_or_else(|| {
                                        AuthorFactsError::InvalidFactValue {
                                            message: format!(
                                                "int field '{field_id}' of subject '{key}' must be an integer"
                                            ),
                                        }
                                    })?;
                                int_fields.push(SubjectIntFact {
                                    subject: subject.clone(),
                                    field_id: field_id.clone(),
                                    value,
                                });
                            }
                        }
                        "effective"
                        | "effective_stats"
                        | "effective_int_fields"
                        | "effective_ints" => {
                            let map = pool_val.as_object().ok_or_else(|| {
                                AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "pool '{pool_key}' of subject '{key}' must be an object"
                                    ),
                                }
                            })?;
                            for (field_id, num_val) in map {
                                let value = num_val.as_i64().ok_or_else(|| {
                                        AuthorFactsError::InvalidFactValue {
                                            message: format!(
                                                "effective int field '{field_id}' of subject '{key}' must be an integer"
                                            ),
                                        }
                                    })?;
                                effective_int_fields.push(SubjectIntFact {
                                    subject: subject.clone(),
                                    field_id: field_id.clone(),
                                    value,
                                });
                            }
                        }
                        "state" | "states" | "bool" | "bools" | "bool_fields" => {
                            let map = pool_val.as_object().ok_or_else(|| {
                                AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "pool '{pool_key}' of subject '{key}' must be an object"
                                    ),
                                }
                            })?;
                            for (field_id, b_val) in map {
                                let value = b_val.as_bool().ok_or_else(|| {
                                        AuthorFactsError::InvalidFactValue {
                                            message: format!(
                                                "bool field '{field_id}' of subject '{key}' must be a boolean"
                                            ),
                                        }
                                    })?;
                                bool_fields.push(SubjectBoolFact {
                                    subject: subject.clone(),
                                    field_id: field_id.clone(),
                                    value,
                                });
                            }
                        }
                        "text" | "texts" | "text_fields" | "string" | "strings" => {
                            let map = pool_val.as_object().ok_or_else(|| {
                                AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "pool '{pool_key}' of subject '{key}' must be an object"
                                    ),
                                }
                            })?;
                            for (field_id, t_val) in map {
                                let value = t_val.as_str().ok_or_else(|| {
                                        AuthorFactsError::InvalidFactValue {
                                            message: format!(
                                                "text field '{field_id}' of subject '{key}' must be a string"
                                            ),
                                        }
                                    })?;
                                text_fields.push(SubjectTextFact {
                                    subject: subject.clone(),
                                    field_id: field_id.clone(),
                                    value: value.to_string(),
                                });
                            }
                        }
                        "resource_bounds" | "resources" | "resource" => {
                            let map = pool_val.as_object().ok_or_else(|| {
                                AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "pool '{pool_key}' of subject '{key}' must be an object"
                                    ),
                                }
                            })?;
                            for (field_id, bound_val) in map {
                                let (min_val, max_val) = if let Some(b_obj) = bound_val.as_object()
                                {
                                    let min_val =
                                            b_obj.get("min").map(|v| v.as_i64().ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("resource_bounds.{key}.{field_id}.min must be an integer") })).transpose()?.unwrap_or(0);
                                    let max_val =
                                            b_obj.get("max").map(|v| v.as_i64().ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("resource_bounds.{key}.{field_id}.max must be an integer") })).transpose()?.unwrap_or(0);
                                    (min_val, max_val)
                                } else if let Some(arr) = bound_val.as_array() {
                                    if arr.len() != 2 {
                                        return Err(AuthorFactsError::InvalidFactValue { message: format!("resource_bounds.{key}.{field_id} must be [min, max]") });
                                    }
                                    let min_val =
                                            arr[0].as_i64().ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("resource_bounds.{key}.{field_id}[0] must be an integer") })?;
                                    let max_val =
                                            arr[1].as_i64().ok_or_else(|| AuthorFactsError::InvalidFactValue { message: format!("resource_bounds.{key}.{field_id}[1] must be an integer") })?;
                                    (min_val, max_val)
                                } else {
                                    return Err(AuthorFactsError::InvalidFactValue {
                                            message: format!(
                                                "resource bound for '{field_id}' must be an object with min/max or [min, max]"
                                            ),
                                        });
                                };
                                resource_bounds.push(SubjectResourceBoundFact {
                                    subject: subject.clone(),
                                    field_id: field_id.clone(),
                                    min_value: min_val,
                                    max_value: max_val,
                                });
                            }
                        }
                        "capabilities" | "capability" | "has_capability" => {
                            collect_id_facts(&mut capabilities, &subject, pool_val, key, pool_key)?;
                        }
                        "items" | "item" | "has_item" => {
                            collect_id_facts(&mut items, &subject, pool_val, key, pool_key)?;
                        }
                        "niches" | "niche" | "has_niche" => {
                            collect_id_facts(&mut niches, &subject, pool_val, key, pool_key)?;
                        }
                        "tags" | "tag" | "has_tag" => {
                            collect_id_facts(&mut tags, &subject, pool_val, key, pool_key)?;
                        }
                        "statuses" | "status" | "has_status" => {
                            collect_id_facts(&mut statuses, &subject, pool_val, key, pool_key)?;
                        }
                        "traces" | "trace" | "has_trace" => {
                            collect_id_facts(&mut traces, &subject, pool_val, key, pool_key)?;
                        }
                        "known_entities" | "knows_entity" => {
                            collect_id_facts(
                                &mut known_entities,
                                &subject,
                                pool_val,
                                key,
                                pool_key,
                            )?;
                        }
                        "source" | "sources" | "item_dsl_sources" | "item_sources" => {
                            let slots_obj = pool_val.as_object().ok_or_else(|| {
                                    AuthorFactsError::InvalidFactValue {
                                        message: format!(
                                            "sources pool of subject '{key}' must be an object of slot -> DSL text"
                                        ),
                                    }
                                })?;
                            for (slot, source_val) in slots_obj {
                                let dsl_text = source_val.as_str().ok_or_else(|| {
                                    AuthorFactsError::InvalidFactValue {
                                        message: format!(
                                            "item source slot '{slot}' must be a DSL text string"
                                        ),
                                    }
                                })?;
                                let expr = compile_int_expression_with_macros(dsl_text, macros)
                                    .map_err(|failure| {
                                        let msg = failure
                                            .diagnostics
                                            .iter()
                                            .map(|d| format!("{}: {}", d.code, d.message))
                                            .collect::<Vec<_>>()
                                            .join("; ");
                                        AuthorFactsError::ItemSourceCompileError {
                                            subject: key.clone(),
                                            slot: slot.clone(),
                                            message: msg,
                                        }
                                    })?;
                                item_dsl_sources.push(ItemDslSourceFact {
                                    subject: subject.clone(),
                                    slot: slot.clone(),
                                    expr,
                                });
                            }
                        }
                        "kind" if matches!(subject, SubjectRef::Event(_)) => {
                            event_kind = Some(
                                pool_val
                                    .as_str()
                                    .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                                        message: format!(
                                            "event.kind for subject '{key}' must be a string"
                                        ),
                                    })?
                                    .to_string(),
                            );
                        }
                        "detail" | "details" if matches!(subject, SubjectRef::Event(_)) => {
                            let detail_obj = pool_val.as_object().ok_or_else(|| {
                                AuthorFactsError::InvalidFactValue {
                                    message: format!(
                                        "event.details for subject '{key}' must be an object"
                                    ),
                                }
                            })?;
                            for (detail_field, d_val) in detail_obj {
                                let event_subject = SubjectRef::Event(EventRef::Detail {
                                    field_id: detail_field.clone(),
                                });
                                if let Some(n) = d_val.as_i64() {
                                    int_fields.push(SubjectIntFact {
                                        subject: event_subject,
                                        field_id: detail_field.clone(),
                                        value: n,
                                    });
                                } else if let Some(b) = d_val.as_bool() {
                                    bool_fields.push(SubjectBoolFact {
                                        subject: event_subject,
                                        field_id: detail_field.clone(),
                                        value: b,
                                    });
                                } else if let Some(s) = d_val.as_str() {
                                    text_fields.push(SubjectTextFact {
                                        subject: event_subject,
                                        field_id: detail_field.clone(),
                                        value: s.to_string(),
                                    });
                                } else {
                                    return Err(AuthorFactsError::InvalidFactValue { message: format!("event.details.{detail_field} for subject '{key}' must be an integer, boolean, or string") });
                                }
                            }
                        }
                        _ => {
                            return Err(AuthorFactsError::UnknownPool {
                                pool: pool_key.clone(),
                                subject: Some(key.clone()),
                            });
                        }
                    }
                }
            }
        }
    }

    // Flatten roll table entries map into vector
    let roll_table_entries: Vec<RollTableEntryFact> =
        roll_table_entries_map.into_values().flatten().collect();

    // Construct context by explicitly naming every field with NO `..`
    Ok(EvaluationContext {
        bool_fields,
        int_fields,
        effective_int_fields,
        resource_bounds,
        text_fields,
        capabilities,
        items,
        niches,
        tags,
        statuses,
        traces,
        known_entities,
        bindings,
        roll_table_entries,
        item_dsl_sources,
        implied_by_counts,
        participant_tag_group_counts,
        peers_near_target,
        occupants_at,
        channel_pools,
        event_kind,
        roll_modifier_bias,
    })
}

fn collect_id_facts(
    target: &mut Vec<SubjectIdFact>,
    subject: &SubjectRef,
    val: &serde_json::Value,
    subject_key: &str,
    pool_key: &str,
) -> Result<(), AuthorFactsError> {
    if let Some(arr) = val.as_array() {
        for (i, item) in arr.iter().enumerate() {
            let s = item
                .as_str()
                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                    message: format!("{subject_key}.{pool_key}[{i}] must be a string"),
                })?;
            target.push(SubjectIdFact {
                subject: subject.clone(),
                id: s.to_string(),
            });
        }
    } else if let Some(obj) = val.as_object() {
        for (id, v) in obj {
            let enabled = v
                .as_bool()
                .ok_or_else(|| AuthorFactsError::InvalidFactValue {
                    message: format!("{subject_key}.{pool_key}.{id} must be a boolean"),
                })?;
            if enabled {
                target.push(SubjectIdFact {
                    subject: subject.clone(),
                    id: id.clone(),
                });
            }
        }
    } else {
        return Err(AuthorFactsError::InvalidFactValue {
            message: format!("{subject_key}.{pool_key} must be an array of strings or an object of id -> boolean"),
        });
    }
    Ok(())
}

/// Maps an `EvaluationError` to a named rejection with an EXHAUSTIVE `match`.
///
/// For an exhausted dice list, reports the 1-based number of the draw that found
/// the list empty, computed as `supplied_dice_len + 1` (not from `attempted`,
/// which restarts at zero inside nested source evaluations).
///
/// For a draw-bound error, reports `expression_budget` from
/// `EvaluationLimits::for_expression`, not the error's `max_draws`.
pub fn map_evaluation_error(
    err: &EvaluationError,
    supplied_dice_len: usize,
    expression_budget: u32,
) -> EvaluationRejection {
    match err {
        EvaluationError::DrawStreamExhausted {
            node_id: _,
            attempted: _,
        } => {
            let draw_num = supplied_dice_len + 1;
            EvaluationRejection {
                kind: "dice_exhausted".to_string(),
                message: format!("draw {draw_num} found the list empty"),
                subject: None,
                field_id: None,
                pool: None,
                draw_number: Some(draw_num),
                bound: None,
                budget: None,
                macro_id: None,
                operator: None,
                diagnostics: None,
            }
        }
        EvaluationError::DrawLimitExceeded {
            attempted: _,
            max_draws: _,
        } => EvaluationRejection {
            kind: "bound_exceeded".to_string(),
            message: format!("draw bound exceeded: budget {expression_budget}"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: Some("draw".to_string()),
            budget: Some(expression_budget),
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::DepthLimitExceeded {
            node_id: _,
            max_depth,
        } => EvaluationRejection {
            kind: "bound_exceeded".to_string(),
            message: format!("depth bound exceeded: limit {max_depth}"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: Some("depth".to_string()),
            budget: Some(*max_depth),
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::NodeLimitExceeded {
            actual: _,
            max_nodes,
        } => EvaluationRejection {
            kind: "bound_exceeded".to_string(),
            message: format!("node bound exceeded: limit {max_nodes}"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: Some("node".to_string()),
            budget: Some(*max_nodes),
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::UnknownField { subject, field_id } => {
            let sub_name = subject_to_string(subject);
            EvaluationRejection {
                kind: "missing_fact".to_string(),
                message: format!("missing fact: {sub_name}.{field_id}"),
                subject: Some(sub_name),
                field_id: Some(field_id.clone()),
                pool: None,
                draw_number: None,
                bound: None,
                budget: None,
                macro_id: None,
                operator: None,
                diagnostics: None,
            }
        }
        EvaluationError::NoItemDslSourceFacts {
            node_id: _,
            subject,
            slot,
        } => {
            let sub_name = subject_to_string(subject);
            EvaluationRejection {
                kind: "missing_fact".to_string(),
                message: format!("missing item source slot: {sub_name}.{slot}"),
                subject: Some(sub_name),
                field_id: Some(slot.clone()),
                pool: None,
                draw_number: None,
                bound: None,
                budget: None,
                macro_id: None,
                operator: None,
                diagnostics: None,
            }
        }
        EvaluationError::MissingItemDslSource {
            node_id: _,
            subject,
            slot,
        } => {
            let sub_name = subject_to_string(subject);
            EvaluationRejection {
                kind: "missing_fact".to_string(),
                message: format!("missing item source slot: {sub_name}.{slot}"),
                subject: Some(sub_name),
                field_id: Some(slot.clone()),
                pool: None,
                draw_number: None,
                bound: None,
                budget: None,
                macro_id: None,
                operator: None,
                diagnostics: None,
            }
        }
        EvaluationError::UnsupportedInPredicate { node_id } => EvaluationRejection {
            kind: "unsupported_operator".to_string(),
            message: format!("operator at node {node_id} is only executable by the server"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: Some(format!("node_{node_id}")),
            diagnostics: None,
        },
        EvaluationError::EmptyRollTable {
            node_id: _,
            table_id,
        } => EvaluationRejection {
            kind: "empty_roll_table".to_string(),
            message: format!("roll table '{table_id}' is empty"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::NonUniformRollTableIntEntry {
            node_id: _,
            table_id,
            entry_id,
            result_ref,
        } => EvaluationRejection {
            kind: "non_uniform_roll_table_entry".to_string(),
            message: format!(
                "non-uniform roll table entry '{entry_id}' in table '{table_id}' with result '{result_ref}'"
            ),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::DivisionByZero { node_id } => EvaluationRejection {
            kind: "division_by_zero".to_string(),
            message: format!("division by zero at node {node_id}"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::CycleDetected { node_id } => EvaluationRejection {
            kind: "cycle_detected".to_string(),
            message: format!("cycle detected at node {node_id}"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::TypeMismatch {
            node_id,
            expected,
            actual,
        } => EvaluationRejection {
            kind: "type_mismatch".to_string(),
            message: format!(
                "type mismatch at node {node_id}: expected {expected:?}, got {actual:?}"
            ),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::MissingNode { node_id } => EvaluationRejection {
            kind: "missing_node".to_string(),
            message: format!("missing node {node_id}"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::UnknownBinding { binding } => EvaluationRejection {
            kind: "unknown_binding".to_string(),
            message: format!("unknown binding '{binding}'"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
        EvaluationError::DslSourceRecursionLimit {
            node_id: _,
            subject,
            slot,
        } => {
            let sub_name = subject_to_string(subject);
            EvaluationRejection {
                kind: "recursion_limit".to_string(),
                message: format!("item source recursion limit exceeded: {sub_name}.{slot}"),
                subject: Some(sub_name),
                field_id: Some(slot.clone()),
                pool: None,
                draw_number: None,
                bound: None,
                budget: None,
                macro_id: None,
                operator: None,
                diagnostics: None,
            }
        }
        EvaluationError::Validation(val_err) => EvaluationRejection {
            kind: "validation_error".to_string(),
            message: format!("validation error: {val_err:?}"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: None,
            diagnostics: None,
        },
    }
}

/// Evaluates a DSL expression with traced execution, returning JSON.
pub fn evaluate_dsl_expression_json_core(
    source: &str,
    kind: &str,
    facts_json: &str,
    dice_json: &str,
    manifest_toml: Option<&str>,
) -> Result<String, String> {
    // Validate kind
    let normalized_kind = kind.to_ascii_lowercase();
    if !matches!(normalized_kind.as_str(), "predicate" | "int" | "text") {
        return Err(format!(
            "unknown kind '{}': expected 'predicate', 'int', or 'text'",
            kind
        ));
    }

    // Parse dice
    let dice: Vec<u32> = serde_json::from_str(dice_json)
        .map_err(|err| format!("dice must be a JSON array of integers: {err}"))?;

    // Parse manifest if provided
    let mut macros = Vec::<DslMacroDefinition>::new();
    let mut manifest_roll_tables = Vec::<RollTableEntryFact>::new();
    if let Some(toml) = manifest_toml {
        let trimmed = toml.trim();
        if !trimmed.is_empty() {
            let parsed = spellkave_format::parse_manifest(trimmed)
                .map_err(|err| format!("manifest TOML parse failed: {err}"))?;
            macros =
                spellkave_validate::dsl_macros::dsl_macro_compiler_definitions(&parsed.manifest);
            for row in &parsed.manifest.roll_table_entries {
                manifest_roll_tables.push(RollTableEntryFact {
                    table_id: row.roll_table_id.clone(),
                    entry_id: row.roll_table_entry_id.clone(),
                    weight: row.weight,
                    result_ref: row.result_ref.clone(),
                });
            }
        }
    }

    // Parse facts
    let facts_val: serde_json::Value = if facts_json.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(facts_json)
            .map_err(|err| format!("facts must be a valid JSON string: {err}"))?
    };
    if !facts_val.is_object() {
        return Err("facts must be a JSON object".to_string());
    }

    let facts = match build_evaluation_context(&facts_val, &macros, &manifest_roll_tables) {
        Ok(ctx) => ctx,
        Err(err) => {
            let rejection = match err {
                AuthorFactsError::UnknownSubject { subject } => EvaluationRejection {
                    kind: "unknown_subject".to_string(),
                    message: format!("unknown subject '{subject}'"),
                    subject: Some(subject),
                    field_id: None,
                    pool: None,
                    draw_number: None,
                    bound: None,
                    budget: None,
                    macro_id: None,
                    operator: None,
                    diagnostics: None,
                },
                AuthorFactsError::UnknownPool { pool, subject } => EvaluationRejection {
                    kind: "unknown_pool".to_string(),
                    message: if let Some(ref sub) = subject {
                        format!("unknown pool '{pool}' for subject '{sub}'")
                    } else {
                        format!("unknown pool '{pool}'")
                    },
                    subject,
                    field_id: None,
                    pool: Some(pool),
                    draw_number: None,
                    bound: None,
                    budget: None,
                    macro_id: None,
                    operator: None,
                    diagnostics: None,
                },
                _ => EvaluationRejection {
                    kind: "invalid_facts".to_string(),
                    message: err.to_string(),
                    subject: None,
                    field_id: None,
                    pool: None,
                    draw_number: None,
                    bound: None,
                    budget: None,
                    macro_id: None,
                    operator: None,
                    diagnostics: None,
                },
            };
            let result = EvaluationResultJson {
                ok: false,
                value: None,
                draws: None,
                trace: None,
                error: Some(rejection.message.clone()),
                rejection: Some(rejection),
            };
            return serde_json::to_string(&result)
                .map_err(|e| format!("JSON serialization failed: {e}"));
        }
    };

    // Compile expression
    let compile_res = match normalized_kind.as_str() {
        "predicate" => compile_predicate_with_macros(source, &macros),
        "int" => compile_int_expression_with_macros(source, &macros),
        "text" => compile_text_expression_with_macros(source, &macros),
        _ => unreachable!(),
    };

    let expr = match compile_res {
        Ok(e) => e,
        Err(compile_failure) => {
            // Check if this is an outcome-only expression
            let outcome_op = if let Ok(outcome_expr) = spellkave_dsl::parse_outcome(source) {
                has_outcome_only_operator(&outcome_expr)
            } else {
                None
            };

            let unknown_macro = compile_failure
                .diagnostics
                .iter()
                .find(|d| d.code == "MF-DSL-MACRO-UNKNOWN");

            let rejection = if let Some(op_name) = outcome_op {
                EvaluationRejection {
                    kind: "unsupported_operator".to_string(),
                    message: format!("operator '{op_name}' is only executable by the server"),
                    subject: None,
                    field_id: None,
                    pool: None,
                    draw_number: None,
                    bound: None,
                    budget: None,
                    macro_id: None,
                    operator: Some(op_name),
                    diagnostics: None,
                }
            } else if let Some(d) = unknown_macro {
                let macro_id = extract_macro_id_from_message(&d.message);
                EvaluationRejection {
                    kind: "unknown_macro".to_string(),
                    message: d.message.clone(),
                    subject: None,
                    field_id: None,
                    pool: None,
                    draw_number: None,
                    bound: None,
                    budget: None,
                    macro_id: Some(macro_id),
                    operator: None,
                    diagnostics: None,
                }
            } else {
                let json_diags = compile_failure
                    .diagnostics
                    .iter()
                    .map(|d| DslDiagnosticJson {
                        code: d.code.to_string(),
                        message: d.message.clone(),
                        start: d.span.start,
                        end: d.span.end,
                    })
                    .collect();
                EvaluationRejection {
                    kind: "parse_error".to_string(),
                    message: compile_failure
                        .diagnostics
                        .iter()
                        .map(|d| format!("{}: {}", d.code, d.message))
                        .collect::<Vec<_>>()
                        .join("; "),
                    subject: None,
                    field_id: None,
                    pool: None,
                    draw_number: None,
                    bound: None,
                    budget: None,
                    macro_id: None,
                    operator: None,
                    diagnostics: Some(json_diags),
                }
            };

            let result = EvaluationResultJson {
                ok: false,
                value: None,
                draws: None,
                trace: None,
                error: Some(rejection.message.clone()),
                rejection: Some(rejection),
            };
            return serde_json::to_string(&result)
                .map_err(|e| format!("JSON serialization failed: {e}"));
        }
    };

    // Check for outcome-only operators
    if let Some(op_name) = has_outcome_only_operator(&expr) {
        let rejection = EvaluationRejection {
            kind: "unsupported_operator".to_string(),
            message: format!("operator '{op_name}' is only executable by the server"),
            subject: None,
            field_id: None,
            pool: None,
            draw_number: None,
            bound: None,
            budget: None,
            macro_id: None,
            operator: Some(op_name),
            diagnostics: None,
        };
        let result = EvaluationResultJson {
            ok: false,
            value: None,
            draws: None,
            trace: None,
            error: Some(rejection.message.clone()),
            rejection: Some(rejection),
        };
        return serde_json::to_string(&result)
            .map_err(|e| format!("JSON serialization failed: {e}"));
    }

    // Build limits and RNG
    let limits = EvaluationLimits::for_expression(&expr);
    let mut rng = DeterministicRng::from_recorded_values(dice.clone());

    // Traced evaluation
    let eval_res = evaluate_expression_traced(&expr, &facts, &mut rng, limits);
    let result = match eval_res {
        Ok(traced) => EvaluationResultJson {
            ok: true,
            value: Some(traced.value),
            draws: Some(traced.stochastic_draws),
            trace: Some(traced.trace),
            error: None,
            rejection: None,
        },
        Err(err) => {
            let rejection = map_evaluation_error(&err.error, dice.len(), limits.max_draws);
            EvaluationResultJson {
                ok: false,
                value: None,
                draws: None,
                trace: Some(err.trace),
                error: Some(rejection.message.clone()),
                rejection: Some(rejection),
            }
        }
    };

    serde_json::to_string(&result).map_err(|e| format!("JSON serialization failed: {e}"))
}

fn extract_macro_id_from_message(msg: &str) -> String {
    if let Some(start) = msg.find('`') {
        if let Some(end) = msg[start + 1..].find('`') {
            return msg[start + 1..start + 1 + end].to_string();
        }
    }
    msg.to_string()
}

fn has_outcome_only_operator(expr: &Expression) -> Option<String> {
    for node in &expr.nodes {
        match &node.op {
            spellkave_dsl::Op::OutcomeSequence(_) => return Some("OutcomeSequence".to_string()),
            spellkave_dsl::Op::InvokeOutcomeGraph(_) => {
                return Some("InvokeOutcomeGraph".to_string())
            }
            spellkave_dsl::Op::Arbitrate(_) => return Some("Arbitrate".to_string()),
            spellkave_dsl::Op::ModifyResource(_) => return Some("ModifyResource".to_string()),
            spellkave_dsl::Op::ApplyStatus(_) => return Some("ApplyStatus".to_string()),
            spellkave_dsl::Op::EmitTrace(_) => return Some("EmitTrace".to_string()),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_dsl::{ValidationError, ValueType};

    // --- Task 2.1 Tests ---

    #[test]
    fn maps_every_subject_kind() {
        let facts_json = serde_json::json!({
            "actor": { "stat": { "str": 10 } },
            "target": { "stat": { "ac": 15 } },
            "location": { "tag": ["outdoor"] },
            "session": { "tag": ["combat"] },
            "world": { "stat": { "tick": 100 } },
            "actor.wielded_item": { "stat": { "reach": 5 } },
            "actor.equipped_item:main_hand": { "stat": { "bonus": 2 } },
            "binding:party": { "text": { "leader": "gandalf" } },
            "event": { "kind": "attack", "detail": { "dmg": 8 } }
        });
        let ctx = build_evaluation_context(&facts_json, &[], &[]).expect("must parse all subjects");
        assert!(ctx
            .int_fields
            .iter()
            .any(|f| f.subject == SubjectRef::Actor && f.field_id == "str" && f.value == 10));
        assert!(ctx
            .int_fields
            .iter()
            .any(|f| f.subject == SubjectRef::Target && f.field_id == "ac" && f.value == 15));
        assert!(ctx
            .tags
            .iter()
            .any(|f| f.subject == SubjectRef::Location && f.id == "outdoor"));
        assert!(ctx
            .tags
            .iter()
            .any(|f| f.subject == SubjectRef::Session && f.id == "combat"));
        assert!(ctx
            .int_fields
            .iter()
            .any(|f| f.subject == SubjectRef::World && f.field_id == "tick" && f.value == 100));
        assert!(ctx
            .int_fields
            .iter()
            .any(|f| f.subject == SubjectRef::ActorWieldedItem
                && f.field_id == "reach"
                && f.value == 5));
        assert!(ctx.int_fields.iter().any(|f| f.subject
            == SubjectRef::ActorEquippedItem {
                slot_id: "main_hand".into()
            }
            && f.field_id == "bonus"
            && f.value == 2));
        assert!(ctx.text_fields.iter().any(|f| f.subject
            == SubjectRef::Binding(BindingRef {
                name: "party".into()
            })
            && f.field_id == "leader"
            && f.value == "gandalf"));
        assert_eq!(ctx.event_kind.as_deref(), Some("attack"));
        assert!(ctx.int_fields.iter().any(|f| matches!(&f.subject, SubjectRef::Event(EventRef::Detail { field_id }) if field_id == "dmg") && f.value == 8));
    }

    #[test]
    fn rejects_unknown_subject_by_name() {
        let facts_json = serde_json::json!({
            "unknown_entity_42": { "stat": { "hp": 50 } }
        });
        let err = build_evaluation_context(&facts_json, &[], &[])
            .expect_err("must reject unknown subject");
        assert_eq!(
            err,
            AuthorFactsError::UnknownSubject {
                subject: "unknown_entity_42".into()
            }
        );
    }

    #[test]
    fn rejects_unknown_pool_by_name() {
        let facts_json = serde_json::json!({
            "actor": { "invalid_pool_name": { "hp": 50 } }
        });
        let err =
            build_evaluation_context(&facts_json, &[], &[]).expect_err("must reject unknown pool");
        assert_eq!(
            err,
            AuthorFactsError::UnknownPool {
                pool: "invalid_pool_name".into(),
                subject: Some("actor".into())
            }
        );
    }

    #[test]
    fn non_object_facts_are_usage_errors_at_the_public_boundary() {
        for facts in ["null", "[]", "42", "true", "\"facts\""] {
            assert_eq!(
                evaluate_dsl_expression_json_core("1", "int", facts, "[]", None),
                Err("facts must be a JSON object".to_string()),
                "input: {facts}"
            );
        }
    }

    #[test]
    fn rejects_malformed_supplied_fact_values_with_paths() {
        let malformed_cases = [
            (
                serde_json::json!({ "actor": { "capabilities": { "sneak": "false" } } }),
                "actor.capabilities.sneak",
            ),
            (
                serde_json::json!({ "roll_modifier_bias": { "roll_modifier": "advantge" } }),
                "roll_modifier_bias.roll_modifier",
            ),
            (
                serde_json::json!({ "participant_tag_group_counts": [{ "count": "two" }] }),
                "participant_tag_group_counts[0].count",
            ),
            (
                serde_json::json!({ "actor": { "resource": { "hp": [0, "ten"] } } }),
                "resource_bounds.actor.hp[1]",
            ),
            (
                serde_json::json!({ "peers_near_target": ["not a row"] }),
                "peers_near_target[0]",
            ),
            (
                serde_json::json!({ "roll_tables": { "loot": [null] } }),
                "roll_tables.loot[0]",
            ),
        ];

        for (facts, path) in malformed_cases {
            let err = build_evaluation_context(&facts, &[], &[]).expect_err(path);
            assert!(
                matches!(&err, AuthorFactsError::InvalidFactValue { message } if message.contains(path)),
                "{err}"
            );
        }
    }

    #[test]
    fn retains_optional_defaults_and_valid_fact_aliases() {
        let facts = serde_json::json!({
            "roll_tables": { "loot": [{ "id": "common", "value": 7 }] },
            "roll_bias": { "roll_modifier": "ADVANTAGE" },
            "actor": {
                "resource": { "hp": { "min": 2 } },
                "has_capability": { "sneak": true, "loud": false }
            },
            "peers_near_target": [{}],
            "occupants_at": [{}],
            "participant_tag_group_counts": [{}]
        });

        let ctx = build_evaluation_context(&facts, &[], &[]).expect("valid aliases and defaults");
        assert!(ctx.capabilities.iter().any(|f| f.id == "sneak"));
        assert!(!ctx.capabilities.iter().any(|f| f.id == "loud"));
        assert!(matches!(
            ctx.roll_modifier_bias,
            Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Advantage)
            })
        ));
        assert!(ctx
            .resource_bounds
            .iter()
            .any(|f| f.field_id == "hp" && f.min_value == 2 && f.max_value == 0));
        assert!(ctx.roll_table_entries.iter().any(|f| f.table_id == "loot"
            && f.entry_id == "common"
            && f.weight == 1
            && f.result_ref == "7"));
        assert_eq!(ctx.peers_near_target[0].count, 0);
        assert_eq!(ctx.occupants_at[0].scope, "");
        assert_eq!(ctx.participant_tag_group_counts[0].prefix, "");
    }

    #[test]
    fn maps_every_error_variant_exhaustively() {
        let budget = 40;
        let dice_len = 5;

        let variants: Vec<(EvaluationError, &str)> = vec![
            (
                EvaluationError::DrawStreamExhausted {
                    node_id: 1,
                    attempted: 3,
                },
                "dice_exhausted",
            ),
            (
                EvaluationError::DrawLimitExceeded {
                    attempted: 45,
                    max_draws: 20,
                },
                "bound_exceeded",
            ),
            (
                EvaluationError::DepthLimitExceeded {
                    node_id: 2,
                    max_depth: 32,
                },
                "bound_exceeded",
            ),
            (
                EvaluationError::NodeLimitExceeded {
                    actual: 300,
                    max_nodes: 256,
                },
                "bound_exceeded",
            ),
            (
                EvaluationError::UnknownField {
                    subject: SubjectRef::Actor,
                    field_id: "demo/x".into(),
                },
                "missing_fact",
            ),
            (
                EvaluationError::NoItemDslSourceFacts {
                    node_id: 3,
                    subject: SubjectRef::ActorWieldedItem,
                    slot: "demo/atk".into(),
                },
                "missing_fact",
            ),
            (
                EvaluationError::MissingItemDslSource {
                    node_id: 4,
                    subject: SubjectRef::ActorWieldedItem,
                    slot: "demo/atk".into(),
                },
                "missing_fact",
            ),
            (
                EvaluationError::UnsupportedInPredicate { node_id: 5 },
                "unsupported_operator",
            ),
            (
                EvaluationError::EmptyRollTable {
                    node_id: 6,
                    table_id: "core/t".into(),
                },
                "empty_roll_table",
            ),
            (
                EvaluationError::NonUniformRollTableIntEntry {
                    node_id: 7,
                    table_id: "core/t".into(),
                    entry_id: "e1".into(),
                    result_ref: "r1".into(),
                },
                "non_uniform_roll_table_entry",
            ),
            (
                EvaluationError::DivisionByZero { node_id: 8 },
                "division_by_zero",
            ),
            (
                EvaluationError::CycleDetected { node_id: 9 },
                "cycle_detected",
            ),
            (
                EvaluationError::TypeMismatch {
                    node_id: 10,
                    expected: ValueType::Int,
                    actual: ValueType::Bool,
                },
                "type_mismatch",
            ),
            (EvaluationError::MissingNode { node_id: 11 }, "missing_node"),
            (
                EvaluationError::UnknownBinding {
                    binding: "foo".into(),
                },
                "unknown_binding",
            ),
            (
                EvaluationError::DslSourceRecursionLimit {
                    node_id: 12,
                    subject: SubjectRef::ActorWieldedItem,
                    slot: "demo/s".into(),
                },
                "recursion_limit",
            ),
            (
                EvaluationError::Validation(ValidationError::RootMissing { root_node_id: 0 }),
                "validation_error",
            ),
        ];

        for (err, expected_kind) in variants {
            let rej = map_evaluation_error(&err, dice_len, budget);
            assert_eq!(
                rej.kind, expected_kind,
                "err {:?} must map to kind {}",
                err, expected_kind
            );
        }
    }

    #[test]
    fn exhaustion_inside_item_source_slot_reports_global_draw_number() {
        // Expression: 1d20 + actor.wielded_item.source('extra')
        // slot 'extra': 1d6 + 1d4
        // Total dice needed: 3 (1 in root, 2 in slot).
        // Provide 2 dice: [12, 5]. Root consumes 12. Slot consumes 5. Second die in slot finds list empty.
        // Must report draw_number: 3 (length of dice list + 1).
        let facts = serde_json::json!({
            "item_sources": {
                "actor.wielded_item": {
                    "extra": "1d6 + 1d4"
                }
            }
        });
        let res_json = evaluate_dsl_expression_json_core(
            "1d20 + actor.wielded_item.source('extra')",
            "int",
            &facts.to_string(),
            "[12, 5]",
            None,
        )
        .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], false);
        assert_eq!(res["rejection"]["kind"], "dice_exhausted");
        assert_eq!(res["rejection"]["draw_number"], 3);
    }

    #[test]
    fn draw_bound_inside_item_source_slot_reports_expression_budget() {
        let expr =
            compile_int_expression_with_macros("1d20 + actor.wielded_item.source('extra')", &[])
                .expect("expr parses");
        let budget = EvaluationLimits::for_expression(&expr).max_draws;

        let err = EvaluationError::DrawLimitExceeded {
            attempted: 2,
            max_draws: 1,
        };
        let rej = map_evaluation_error(&err, 5, budget);
        assert_eq!(rej.kind, "bound_exceeded");
        assert_eq!(rej.bound.as_deref(), Some("draw"));
        assert_eq!(rej.budget, Some(budget));
    }

    // --- Task 2.2 Tests ---

    #[test]
    fn roll_with_fixed_dice() {
        let facts = serde_json::json!({
            "actor": { "stat": { "demo/bonus": 3 } },
            "target": { "stat": { "demo/threshold": 15 } }
        });
        let res_json = evaluate_dsl_expression_json_core(
            "1d20 + actor.stat('demo/bonus') >= target.stat('demo/threshold')",
            "predicate",
            &facts.to_string(),
            "[12]",
            None,
        )
        .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], true);
        assert_eq!(res["value"]["type"], "Bool");
        assert_eq!(res["value"]["value"], true);
        assert_eq!(res["draws"].as_array().expect("draws array").len(), 1);
        assert_eq!(res["draws"][0]["value"], 12);
        assert!(!res["trace"].as_array().expect("trace array").is_empty());
    }

    #[test]
    fn false_predicate_is_successful_result() {
        let facts = serde_json::json!({
            "actor": { "stat": { "demo/bonus": 3 } },
            "target": { "stat": { "demo/threshold": 15 } }
        });
        let res_json = evaluate_dsl_expression_json_core(
            "1d20 + actor.stat('demo/bonus') >= target.stat('demo/threshold')",
            "predicate",
            &facts.to_string(),
            "[11]",
            None,
        )
        .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], true);
        assert_eq!(res["value"]["type"], "Bool");
        assert_eq!(res["value"]["value"], false);
    }

    #[test]
    fn missing_fact_rejects_and_names_it() {
        let facts = serde_json::json!({
            "actor": { "stat": { "demo/bonus": 3 } }
        });
        let res_json = evaluate_dsl_expression_json_core(
            "1d20 + actor.stat('demo/bonus') >= target.stat('demo/threshold')",
            "predicate",
            &facts.to_string(),
            "[12]",
            None,
        )
        .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], false);
        assert_eq!(res["rejection"]["kind"], "missing_fact");
        assert_eq!(res["rejection"]["subject"], "target");
        assert_eq!(res["rejection"]["field_id"], "demo/threshold");
    }

    #[test]
    fn exhausted_dice_rejects() {
        let res_json = evaluate_dsl_expression_json_core("1d20 + 1d6", "int", "{}", "[10]", None)
            .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], false);
        assert_eq!(res["rejection"]["kind"], "dice_exhausted");
        assert_eq!(res["rejection"]["draw_number"], 2);
    }

    #[test]
    fn expression_over_draw_ceiling_rejected_naming_bound() {
        let res_json = evaluate_dsl_expression_json_core("257d6", "int", "{}", "[]", None)
            .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], false);
        assert_eq!(res["rejection"]["kind"], "bound_exceeded");
        assert_eq!(res["rejection"]["bound"], "draw");
    }

    #[test]
    fn limits_built_with_for_expression() {
        let expr = compile_int_expression_with_macros("2d8", &[]).expect("parses");
        let limits = EvaluationLimits::for_expression(&expr);
        assert!(limits.max_draws >= 2 && limits.max_draws <= 40);
    }

    #[test]
    fn macro_with_and_without_manifest() {
        let manifest_toml = r#"
            #:schema https://spellkave.example/schema/manifest-v1.json
            schema_version = "1.0.0"
            [[bundles]]
            bundle_id = "test"
            display_name = "Test"
            author = "test"
            version = "1.0.0"
            depends_on = []

            [[dsl_macro_definitions]]
            macro_id = "demo/macro_bonus"
            bundle_id = "test"
            return_type = "Int"
            parameters = []
            body_source = "5"
        "#;

        let res_with = evaluate_dsl_expression_json_core(
            "10 + macro('demo/macro_bonus')",
            "int",
            "{}",
            "[]",
            Some(manifest_toml),
        )
        .expect("should return result json");
        let res1: serde_json::Value = serde_json::from_str(&res_with).expect("valid json");
        assert_eq!(res1["ok"], true);
        assert_eq!(res1["value"]["type"], "Int");
        assert_eq!(res1["value"]["value"], 15);

        let res_without = evaluate_dsl_expression_json_core(
            "10 + macro('demo/macro_bonus')",
            "int",
            "{}",
            "[]",
            None,
        )
        .expect("should return result json");
        let res2: serde_json::Value = serde_json::from_str(&res_without).expect("valid json");
        assert_eq!(res2["ok"], false);
        assert_eq!(res2["rejection"]["kind"], "unknown_macro");
        assert_eq!(res2["rejection"]["macro_id"], "demo/macro_bonus");
    }

    #[test]
    fn supplied_roll_table_wins_over_manifest() {
        let manifest_toml = r#"
            #:schema https://spellkave.example/schema/manifest-v1.json
            schema_version = "1.0.0"
            [[bundles]]
            bundle_id = "test"
            display_name = "Test"
            author = "test"
            version = "1.0.0"
            depends_on = []

            [[roll_tables]]
            roll_table_id = "demo/table"
            bundle_id = "test"
            display_name = "Demo Table"
            result_kind = "Int"

            [[roll_table_entries]]
            roll_table_entry_id = "demo/table:manifest_entry"
            roll_table_id = "demo/table"
            authored_order = 1
            weight = 1
            result_ref = "100"
        "#;

        let facts = serde_json::json!({
            "roll_tables": {
                "demo/table": [
                    { "entry_id": "demo/table:facts_entry", "weight": 1, "result_ref": "200" }
                ]
            }
        });

        let res_json = evaluate_dsl_expression_json_core(
            "roll_table('demo/table')",
            "int",
            &facts.to_string(),
            "[1]",
            Some(manifest_toml),
        )
        .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], true);
        assert_eq!(res["value"]["type"], "Int");
        assert_eq!(res["value"]["value"], 200);
        let trace = res["trace"].as_array().expect("trace");
        assert!(trace.iter().any(|e| {
            e["table_entries"]
                .as_array()
                .is_some_and(|te| te.iter().any(|t| t["entry_id"] == "demo/table:facts_entry"))
        }));
    }

    #[test]
    fn item_source_slot_with_scoped_entries() {
        let facts = serde_json::json!({
            "item_sources": {
                "actor.wielded_item": {
                    "damage": "1d8 + 3"
                }
            }
        });

        let res_json = evaluate_dsl_expression_json_core(
            "actor.wielded_item.source('damage')",
            "int",
            &facts.to_string(),
            "[7]",
            None,
        )
        .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], true);
        assert_eq!(res["value"]["type"], "Int");
        assert_eq!(res["value"]["value"], 10);
        let trace = res["trace"].as_array().expect("trace");
        assert!(trace.iter().any(|e| {
            e["scope"]
                .as_array()
                .is_some_and(|scopes| scopes.iter().any(|s| s["slot"] == "damage"))
        }));
    }

    #[test]
    fn outcome_only_operator_is_rejected() {
        let res_json =
            evaluate_dsl_expression_json_core("outcome_sequence()", "predicate", "{}", "[]", None)
                .expect("should return result json");

        let res: serde_json::Value = serde_json::from_str(&res_json).expect("valid json");
        assert_eq!(res["ok"], false);
    }
}
