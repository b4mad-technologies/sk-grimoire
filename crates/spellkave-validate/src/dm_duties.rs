//! Typed source/runtime duty shapes for the opt-in DM carrier. Parsing these
//! structs is only a shape check; reference/authority/publication admission is
//! separate. No kernel reads, callbacks, executable expressions or live facts.
use serde::Deserialize;
use std::collections::BTreeSet;

pub use crate::dm_guide::*;

pub const MAX_REF_BYTES: usize = 16 * 1024;
pub const MAX_REOPEN_TERM_BYTES: usize = 256;
// Protocol v1 dialog palette size.
const DIALOG_SLOT_COUNT_V1: u32 = 16;

/// Component shape and finite-policy admission only. Canonical references,
/// action consequences, P route equality, catalog linkage and publication
/// derivation are separate mandatory gates of the complete carrier compiler.
pub fn parse_source_shape_json(input: &str) -> Result<DutySource, Vec<String>> {
    let raw = crate::dm_document_authority::unique_json(input)?;
    let source: DutySource = serde_json::from_value(raw.clone())
        .map_err(|error| vec![format!("dm-duties: invalid source shape: {error}")])?;
    let errors = validate_shape(&source, &raw);
    if errors.is_empty() {
        Ok(source)
    } else {
        Err(errors)
    }
}

/// Runtime component has the same finite policies and domains, with recipe
/// items forbidden rather than reconstructed from the Job-only source.
pub(crate) fn validate_runtime_component(raw: &serde_json::Value) -> Vec<String> {
    match serde_json::from_value::<DutyComponent<RuntimeKit>>(raw.clone()) {
        Ok(component) => validate_shape(&component, raw),
        Err(error) => vec![format!("dm-duties: invalid runtime component: {error}")],
    }
}

fn validate_shape<K: KitShape>(source: &DutyComponent<K>, raw: &serde_json::Value) -> Vec<String> {
    let mut errors = crate::dm_srd::validate_tables_json(&raw["srd"].to_string());
    let mut require = |valid: bool, message: &str| {
        if !valid {
            errors.push(format!("dm-duties: {message}"));
        }
    };
    require(source.schema_version == 1, "unsupported schema version");
    require(
        unique(source.quest_board.quest_refs.iter().map(String::as_str)),
        "quest board must contain nonempty distinct bounded references",
    );
    let topology = &source.layer_topology;
    require(
        unique(topology.ordered_ids.iter().map(String::as_str)) && topology.ordered_ids.len() == 2,
        "layer ids must contain exactly two distinct bounded references",
    );
    require(
        topology.companion_offer_id != topology.quest_board_id
            && topology.ordered_ids.contains(&topology.companion_offer_id)
            && topology.ordered_ids.contains(&topology.quest_board_id),
        "layer roles must be distinct declared ids",
    );
    require(
        distinct(topology.companion_yields_to.iter().map(String::as_str))
            && topology.companion_yields_to.iter().all(|id| {
                topology.ordered_ids.contains(id)
                    && id == &topology.quest_board_id
                    && id != &topology.companion_offer_id
            }),
        "companion yield ids must be distinct declared quest-board references",
    );

    require(
        distinct(
            source
                .quest_prerequisites
                .iter()
                .map(|row| row.quest_ref.as_str()),
        ),
        "duplicate or blank quest gate",
    );
    for gate in &source.quest_prerequisites {
        require(
            !gate.requires_rewarded_quests.is_empty()
                && unique(gate.requires_rewarded_quests.iter().map(String::as_str))
                && !gate.requires_rewarded_quests.contains(&gate.quest_ref),
            "invalid rewarded-quest prerequisites",
        );
    }
    require(
        nonblank(&source.bundle_id) && nonblank(&source.body_key),
        "missing source identity",
    );
    require(
        source.vessels.hero_resolution == "claimed-vessels/v1",
        "unsupported vessel policy",
    );
    require(
        source.advancement.draft_opener_policy == "hero-level-receipt/v1",
        "unsupported draft opener policy",
    );
    require(
        source.watches.quest_xp.recipient_policy == "legacy-party-and-claimed-heroes/v1",
        "unsupported quest recipient policy",
    );
    require(
        source.watches.threshold.hero_policy == "seat-current-vessel/v1",
        "unsupported threshold hero policy",
    );
    require(
        ["legacy", "controller"].contains(&source.watches.initiative.mode.as_str()),
        "unsupported initiative owner",
    );
    let initiative = &source.watches.initiative;
    require(
        unique(initiative.trigger_capability_ids.iter().map(String::as_str)),
        "initiative trigger capabilities must be nonempty, distinct bounded references",
    );
    require(
        unique([
            initiative.party_tag_id.as_str(),
            initiative.hostile_tag_id.as_str(),
        ]),
        "initiative party and hostile tags must be distinct bounded references",
    );
    if let Some(on_sight) = &initiative.on_sight {
        require(
            initiative.mode == "legacy",
            "on-sight initiative belongs to the legacy owner only",
        );
        require(
            unique(on_sight.actor_keys.iter().map(String::as_str)),
            "on-sight actors must be nonempty, distinct bounded references",
        );
        require(
            on_sight
                .actor_keys
                .iter()
                .all(|key| initiative.hostile_actor_keys.contains(key)),
            "on-sight actor is not a listed hostile",
        );
        require(
            distinct(
                on_sight
                    .unseen_condition_ids
                    .iter()
                    .chain(&on_sight.peace_condition_ids)
                    .map(String::as_str),
            ),
            "on-sight unseen and peace conditions must be distinct bounded references",
        );
    }
    require(!source.watches.quest_loot_enabled, "quest loot is inactive");
    let routes = &source.routes;
    require(
        unique([
            routes.region_binding_id.as_str(),
            routes.creation_binding_id.as_str(),
            routes.advancement_binding_id.as_str(),
            routes.graph_author_binding_id.as_str(),
            routes.bundle_author_binding_id.as_str(),
        ]),
        "duplicate or blank route selector",
    );
    require(
        source.watches.initiative.binding_id == routes.graph_author_binding_id,
        "initiative must retain its graph-author route",
    );
    let creation = &source.creation;
    for values in [
        &source.vessels.party_companion_keys,
        &source.companions.level_sync_keys,
        &source.watches.initiative.hostile_actor_keys,
        &source.watches.quest_xp.quest_refs,
    ] {
        require(
            distinct(values.iter().map(String::as_str)),
            "duplicate or blank key array member",
        );
    }
    require(
        (1..=9_007_199_254_740_991).contains(&creation.kit_granted_value),
        "invalid creation kit granted value",
    );
    let keys = &creation.input_field_keys;
    require(
        distinct([
            keys.name.as_str(),
            keys.appearance.as_str(),
            keys.backstory.as_str(),
        ]) && [&keys.name, &keys.appearance, &keys.backstory]
            .into_iter()
            .all(|key| {
                key.len() <= 64 && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            }),
        "invalid creation input field keys",
    );
    let states = &creation.observation_vocabulary.ability_states;
    let presentation = &creation.presentation_vocabulary;
    require(
        [
            presentation.menu_labels.class.as_str(),
            presentation.menu_labels.background.as_str(),
            presentation.menu_labels.species.as_str(),
            presentation.menu_labels.skill.as_str(),
            presentation.menu_labels.cantrip.as_str(),
            presentation.menu_labels.prepared_spell.as_str(),
            presentation.menu_labels.starting_kit.as_str(),
            presentation.menu_labels.name.as_str(),
            presentation.menu_labels.ability_scores.as_str(),
            presentation.menu_labels.confirmation.as_str(),
        ]
        .into_iter()
        .all(nonblank),
        "blank or oversized creation menu_labels",
    );
    require(
        [
            presentation.missing_phrases.class.as_str(),
            presentation.missing_phrases.background.as_str(),
            presentation.missing_phrases.species.as_str(),
            presentation.missing_phrases.name.as_str(),
            presentation.missing_phrases.skills.as_str(),
            presentation.missing_phrases.kit.as_str(),
        ]
        .into_iter()
        .all(nonblank),
        "blank or oversized creation missing_phrases",
    );
    require(
        nonblank(&presentation.list.empty)
            && [
                presentation.list.pair_separator.as_str(),
                presentation.list.separator.as_str(),
                presentation.list.last_separator.as_str(),
            ]
            .into_iter()
            .all(|value| !value.is_empty() && value.len() <= MAX_REF_BYTES),
        "empty or oversized creation list vocabulary",
    );
    let templates = &presentation.templates;
    require(
        valid_presentation_template(&templates.count_kind, &["kind", "remaining"])
            && valid_presentation_template(&templates.kit_option, &["letter", "description"])
            && valid_presentation_template(
                &templates.equipment_page,
                &["source_text", "page", "pages"],
            )
            && valid_presentation_template(&templates.equipment_more, &["page", "pages"])
            && valid_presentation_template(&templates.adopt_name, &["name"])
            && valid_presentation_template(&templates.adopt_array, &[])
            && valid_presentation_template(&templates.confirm, &[])
            && valid_presentation_template(&templates.remaining_one, &["remaining"])
            && valid_presentation_template(&templates.remaining_many, &["remaining"])
            && valid_presentation_template(&templates.answer_option, &["index"]),
        "invalid creation presentation template placeholders",
    );
    let mut spoken = BTreeSet::new();
    require(
        [
            &presentation.spoken_confirm_aliases,
            &presentation.spoken_discard_aliases,
        ]
        .into_iter()
        .all(|values| {
            !values.is_empty()
                && values.len() <= 64
                && values.iter().all(|alias| {
                    !alias.is_empty()
                        && alias.len() <= 256
                        && alias.trim() == alias
                        && alias
                            .bytes()
                            .all(|byte| byte.is_ascii_lowercase() || b" '-".contains(&byte))
                        && alias.bytes().any(|byte| byte.is_ascii_lowercase())
                        && spoken.insert(alias.as_str())
                })
        }),
        "missing, malformed or ambiguous creation spoken aliases",
    );
    let mut text_aliases: BTreeSet<String> = states
        .iter()
        .flat_map(|row| &row.aliases)
        .map(|alias| alias.to_ascii_lowercase())
        .collect();
    require(
        [
            &presentation.text_field_aliases.name,
            &presentation.text_field_aliases.appearance,
            &presentation.text_field_aliases.backstory,
        ]
        .into_iter()
        .all(|values| {
            !values.is_empty()
                && values.len() <= 64
                && values.iter().all(|alias| {
                    !alias.is_empty()
                        && alias.len() <= 64
                        && alias
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                        && text_aliases.insert(alias.to_ascii_lowercase())
                })
        }),
        "missing, malformed or ambiguous creation text field aliases",
    );
    let menu = &creation.observation_vocabulary.menu_tokens;
    let prefixes = menu.relative_prefixes();
    let literal_prefix = |value: &str| {
        !value.is_empty()
            && value.ends_with('/')
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
    };
    require(
        literal_prefix(&menu.namespace_prefix)
            && prefixes
                .iter()
                .all(|value| literal_prefix(value) && !value.starts_with('/'))
            && prefixes.iter().enumerate().all(|(i, value)| {
                prefixes
                    .iter()
                    .enumerate()
                    .all(|(j, other)| i == j || !value.starts_with(other))
            })
            && menu
                .array_adopt_token
                .as_str()
                .starts_with(&menu.namespace_prefix)
            && menu.array_adopt_token.as_str().len() > menu.namespace_prefix.len()
            && menu
                .array_adopt_token
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
            && prefixes.iter().all(|prefix| {
                !menu.array_adopt_token.as_str()[menu.namespace_prefix.len()..].starts_with(prefix)
            }),
        "missing, malformed or ambiguous creation menu token prefix",
    );
    require(
        menu.codecs.slug == "ascii-lowercase-hyphen/v1"
            && menu.codecs.reference == "js-dot-plus/v1"
            && menu.codecs.choice == "base64url-utf8-lossy/v1"
            && menu.codecs.kit == "ascii-uppercase-one/v1"
            && menu.codecs.adopted_text == "lowercase-hex-utf8-fatal/v1",
        "unsupported creation menu transport codec",
    );
    require(
        (1..=20).contains(&menu.score_digits)
            && creation.limits.point_buy_max.to_string().len() <= menu.score_digits as usize
            && unique(menu.score_ability_order.iter().map(String::as_str))
            && menu.score_ability_order.len() == states.len()
            && menu
                .score_ability_order
                .iter()
                .all(|key| states.iter().any(|row| &row.ability == key)),
        "incomplete or invalid creation menu score transport",
    );
    require(
        creation
            .classes
            .iter()
            .map(|row| (&row.slug, &row.pick_token, &menu.class_prefix))
            .chain(
                creation
                    .species
                    .iter()
                    .map(|row| (&row.slug, &row.pick_token, &menu.species_prefix)),
            )
            .chain(
                creation
                    .backgrounds
                    .iter()
                    .map(|row| (&row.slug, &row.pick_token, &menu.background_prefix)),
            )
            .all(|(slug, token, prefix)| {
                !slug.is_empty()
                    && slug.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                    && token.as_str() == format!("{}{prefix}{slug}", menu.namespace_prefix)
            }),
        "creation pick token differs from declared menu grammar",
    );
    require(
        unique(states.iter().map(|row| row.ability.as_str()))
            && unique(states.iter().map(|row| row.state_id.as_str())),
        "duplicate or blank ability state",
    );
    // The existing English input grammar uses ASCII word aliases and ignores
    // ASCII case. A collision must not let one
    // utterance select two distinct canonical ability states.
    let mut aliases = std::collections::BTreeSet::new();
    require(
        states.iter().all(|row| {
            !row.aliases.is_empty()
                && row
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(&row.ability))
                && row.aliases.iter().all(|alias| {
                    !alias.is_empty()
                        && alias
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                        && aliases.insert(alias.to_ascii_lowercase())
                })
        }),
        "missing, malformed or ambiguous ability aliases",
    );
    require(
        unique(
            creation
                .classes
                .iter()
                .map(|row| row.pick_token.as_str())
                .chain(creation.species.iter().map(|row| row.pick_token.as_str()))
                .chain(
                    creation
                        .backgrounds
                        .iter()
                        .map(|row| row.pick_token.as_str()),
                )
                .chain(std::iter::once(creation.wizard.confirm_token.as_str()))
                .chain([
                    creation.wizard.discard_action_id.as_str(),
                    creation.wizard.back_action_id.as_str(),
                    creation.wizard.forward_action_id.as_str(),
                    creation.wizard.enter_name_action_id.as_str(),
                    creation.wizard.describe_appearance_action_id.as_str(),
                    creation.wizard.describe_backstory_action_id.as_str(),
                    creation.wizard.set_scores_action_id.as_str(),
                    creation.wizard.adopt_scores_action_id.as_str(),
                    creation.wizard.adopt_name_action_id.as_str(),
                    creation.wizard.adopt_appearance_action_id.as_str(),
                    creation.wizard.adopt_backstory_action_id.as_str(),
                ])
                .chain(std::iter::once(menu.array_adopt_token.as_str())),
        ),
        "duplicate or blank creation input token",
    );
    for error in crate::guide::validate(&creation.guide, &CREATION_SOURCES, &CREATION_INPUTS) {
        require(false, &error);
    }
    require(
        contiguous(&creation.wizard.phase_actions, 0),
        "wizard phase actions must have a contiguous domain",
    );
    let dialog = &source.dialog;
    require(
        dialog.slot_count == DIALOG_SLOT_COUNT_V1
            && dialog.runtime_prompt_selector == DIALOG_SLOT_COUNT_V1,
        "unsupported dialog domain",
    );
    for (values, start, end) in [
        (&dialog.ask_menu_actions, 1, dialog.slot_count),
        (&dialog.open_actions, 0, dialog.slot_count),
        (&dialog.prompt_actions, 0, dialog.runtime_prompt_selector),
        (&dialog.choice_actions, 1, dialog.slot_count),
    ] {
        require(
            contiguous(values, start) && values.last().is_some_and(|row| row.value == end),
            "incomplete or duplicate dialog action domain",
        );
    }
    require(
        unique(creation.classes.iter().map(|row| row.slug.as_str())),
        "empty or duplicate classes",
    );
    // The creation menu may offer fewer classes than the admitted SRD data
    // (sk-jay0.1.55.60.19: the solo beta offers four). Every offered class
    // must be an admitted one; an empty menu is refused just above. The SRD
    // class tables stay complete for bodies that level outside the menu.
    {
        let admitted: BTreeSet<_> = source
            .srd
            .classes
            .iter()
            .map(|row| row.slug.as_str())
            .collect();
        require(
            creation
                .classes
                .iter()
                .all(|row| admitted.contains(row.slug.as_str())),
            "creation class is outside admitted SRD data",
        );
    }
    for (selected, canonical) in [
        (
            creation
                .species
                .iter()
                .map(|row| row.slug.as_str())
                .collect::<BTreeSet<_>>(),
            source
                .srd
                .species
                .iter()
                .map(|row| row.slug.as_str())
                .collect::<BTreeSet<_>>(),
        ),
        (
            creation
                .backgrounds
                .iter()
                .map(|row| row.slug.as_str())
                .collect(),
            source
                .srd
                .backgrounds
                .iter()
                .map(|row| row.slug.as_str())
                .collect(),
        ),
    ] {
        require(
            selected == canonical,
            "creation origin domain differs from admitted SRD data",
        );
    }
    require(
        unique(creation.species.iter().map(|row| row.slug.as_str())),
        "empty or duplicate species",
    );
    require(
        unique(creation.backgrounds.iter().map(|row| row.slug.as_str())),
        "empty or duplicate backgrounds",
    );
    require(
        unique(creation.kits.iter().map(KitShape::action_id)),
        "empty or duplicate kit actions",
    );
    require(
        unique(creation.sheet_sets.iter().map(|row| row.state_id.as_str())),
        "empty or duplicate sheet homes",
    );
    require(
        nonblank(&creation.observation_vocabulary.class_tag_prefix)
            && creation.classes.iter().all(|class| {
                !class.slug.is_empty()
                    && !class.slug.contains('.')
                    && class.identity_tag
                        == format!(
                            "{}{}",
                            creation.observation_vocabulary.class_tag_prefix, class.slug
                        )
            }),
        "class identity query prefix differs from admitted class tags",
    );
    for class in &creation.classes {
        require(
            class.catalog.class_slug == class.slug,
            "class catalog slug mismatch",
        );
        require(
            !class.advancement_tags.is_empty()
                && class
                    .advancement_tags
                    .iter()
                    .enumerate()
                    .all(|(index, row)| row.level as usize == index + 1)
                && unique(class.advancement_tags.iter().map(|row| row.tag.as_str())),
            "duplicate or noncontiguous advancement ladder",
        );
        require(
            class.advancement_tags.iter().all(|row| {
                distinct(
                    row.missing_capability_definitions
                        .iter()
                        .map(String::as_str),
                )
            }),
            "blank, oversized or duplicate missing advancement capability definition",
        );
        require(
            distinct(
                class
                    .catalog
                    .choice_groups
                    .iter()
                    .map(|row| row.choice_group.as_str()),
            ),
            "duplicate choice group",
        );
        for group in &class.catalog.choice_groups {
            require(
                ["none", "long_rest_one"].contains(&group.rechoose.as_str()),
                "unsupported choice rechoose policy",
            );
            require(
                distinct(group.member_capability_ids.iter().map(String::as_str)),
                "duplicate choice member",
            );
        }
    }
    for kit in &creation.kits {
        require(
            creation
                .classes
                .iter()
                .any(|class| class.slug == kit.class_slug()),
            "kit class is outside the menu",
        );
        let Some(items) = kit.items() else { continue };
        require(!items.is_empty(), "kit recipe is empty");
        for (index, item) in items.iter().enumerate() {
            require(item.count > 0, "kit item count must be positive");
            require(
                item.parent_index
                    .is_none_or(|parent| (parent as usize) < index),
                "kit parent must precede child",
            );
            require(
                item.slot_id.as_ref().is_none_or(|slot| nonblank(slot)),
                "blank kit equipment slot",
            );
        }
    }
    let training = &creation.training;
    for kind in [
        &training.skill,
        &training.choice_member,
        &training.cantrip,
        &training.prepared_spell,
        &training.proficiency,
        &training.origin_feat,
    ] {
        require(
            kind.action_id.is_some()
                == kind
                    .members
                    .iter()
                    .any(|row| row.availability == "grantable"),
            "training action must be present iff a member is grantable",
        );
        require(
            kind.action_id.as_ref().is_none_or(|id| nonblank(id)),
            "blank training action",
        );
        let mut seen = BTreeSet::new();
        for member in &kind.members {
            require(
                nonblank(&member.capability_id) && seen.insert(&member.capability_id),
                "duplicate or blank training member",
            );
            require(
                ["grantable", "source-only"].contains(&member.availability.as_str()),
                "unsupported training availability",
            );
        }
    }
    let limits = &creation.limits;
    require(
        limits.point_buy_min <= limits.point_buy_max && limits.point_buy_budget > 0,
        "invalid point-buy bounds",
    );
    require(
        limits.max_name_chars > 0
            && limits.max_appearance_chars > 0
            && limits.max_backstory_chars > 0,
        "invalid creation text limits",
    );
    require(source.advancement.max_level > 0, "missing maximum level");
    let rogue = &source.companions.rogue;
    require(
        rogue.initial_level > 0
            && rogue.initial_level <= rogue.resume_level
            && rogue.resume_level <= source.advancement.max_level
            && rogue.hp_at_resume_level > 0,
        "invalid companion resume quantities",
    );
    let offer = &source.companions.offer;
    require(
        offer.minimum_level > 0
            && offer.accept_slot > 0
            && offer.decline_slot > 0
            && offer.accept_slot != offer.decline_slot
            && offer.accept_slot <= offer.open_count
            && offer.decline_slot <= offer.open_count,
        "invalid companion offer slots or level",
    );
    require(
        unique(offer.reopen_terms.iter().map(String::as_str)),
        "empty or duplicate reopen terms",
    );
    require(
        offer.reopen_terms.iter().all(|term| {
            term.len() <= MAX_REOPEN_TERM_BYTES
                && term
                    .bytes()
                    .all(|byte| byte.is_ascii_alphabetic() || byte == b' ')
                && term.trim() == term
        }),
        "reopen terms must be finite ASCII words",
    );
    validate_reference_text(raw, "duties", &mut errors);
    errors
}

fn nonblank(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_REF_BYTES
}
fn distinct<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .all(|value| nonblank(value) && seen.insert(value))
}
fn contiguous(values: &[ValueAction], start: u32) -> bool {
    !values.is_empty()
        && values.iter().enumerate().all(|(index, row)| {
            u32::try_from(index)
                .ok()
                .and_then(|index| start.checked_add(index))
                == Some(row.value)
        })
        && distinct(values.iter().map(|row| row.action_id.as_str()))
}
fn unique<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut count = 0;
    let mut seen = BTreeSet::new();
    values.into_iter().all(|value| {
        count += 1;
        nonblank(value) && seen.insert(value)
    }) && count > 0
}
fn validate_reference_text(value: &serde_json::Value, path: &str, errors: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                let nested = format!("{path}.{key}");
                if (key.ends_with("_id")
                    || key.ends_with("_key")
                    || key.ends_with("_ref")
                    || key.ends_with("_namespace")
                    || [
                        "slug",
                        "tag",
                        "identity_tag",
                        "feature_tag",
                        "letter",
                        "ability",
                    ]
                    .contains(&key.as_str()))
                    && !value.is_null()
                    && !value.as_str().is_some_and(nonblank)
                {
                    errors.push(format!(
                        "dm-duties: {nested}: expected bounded nonblank reference"
                    ));
                }
                validate_reference_text(value, &nested, errors);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_reference_text(value, &format!("{path}[{index}]"), errors);
            }
        }
        _ => {}
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DutyComponent<K> {
    pub schema_version: u32,
    pub bundle_id: String,
    pub body_key: String,
    pub quest_prerequisites: Vec<QuestPrerequisite>,
    pub quest_board: QuestBoard,
    pub layer_topology: LayerTopology,
    pub routes: Routes,
    pub vessels: Vessels,
    pub persona: Persona,
    pub creation: Creation<K>,
    pub advancement: Advancement,
    pub dialog: Dialog,
    pub companions: Companions,
    pub watches: Watches,
    pub srd: crate::dm_srd::Tables,
}
pub type DutySource = DutyComponent<JobKit>;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DutyDocument {
    pub schema_version: u32,
    pub bundle_id: String,
    pub body_key: String,
    pub quest_prerequisites: Vec<QuestPrerequisite>,
    pub quest_board: QuestBoard,
    pub layer_topology: LayerTopology,
    pub catalog_namespace: String,
    pub catalog_content_hash: String,
    pub catalog_publication_id: String,
    pub presentation_content_hash: String,
    pub routes: Routes,
    pub vessels: Vessels,
    pub persona: Persona,
    pub creation: Creation<RuntimeKit>,
    pub advancement: Advancement,
    pub dialog: Dialog,
    pub companions: Companions,
    pub watches: Watches,
    pub srd: crate::dm_srd::Tables,
    pub content_hash: String,
    pub publication_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestBoard {
    /// Authored selection order; never sort or infer from catalog iteration.
    pub quest_refs: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerTopology {
    pub ordered_ids: Vec<String>,
    pub companion_offer_id: String,
    pub quest_board_id: String,
    pub companion_yields_to: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestPrerequisite {
    pub quest_ref: String,
    pub requires_rewarded_quests: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routes {
    pub region_binding_id: String,
    pub creation_binding_id: String,
    pub advancement_binding_id: String,
    pub graph_author_binding_id: String,
    pub bundle_author_binding_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vessels {
    pub candidate_key: String,
    pub candidate_blueprint_id: String,
    pub creation_station_node_id: String,
    pub rest_node_id: String,
    pub hero_resolution: String,
    pub party_companion_keys: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    pub slug: String,
    pub tag: String,
    pub niche_id: String,
    pub applied_level_state_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creation<K> {
    pub open_seat_action_id: String,
    pub open_action_id: String,
    pub finish_action_id: String,
    pub induct_action_id: String,
    pub identify: Identify,
    pub text: TextActions,
    pub pending: Pending,
    pub kit_granted_state_id: String,
    pub kit_granted_value: u64,
    pub wizard: Wizard,
    /// The order of the creation questions. No order is built in.
    pub guide: crate::guide::Guide,
    pub sheet_sets: Vec<SheetSet>,
    pub classes: Vec<Class>,
    pub species: Vec<Origin>,
    pub backgrounds: Vec<Origin>,
    pub training: Training,
    pub kits: Vec<K>,
    pub starter_kit: StarterKit,
    pub input_field_keys: CreationInputFieldKeys,
    pub presentation_vocabulary: CreationPresentationVocabulary,
    pub observation_vocabulary: ObservationVocabulary,
    pub limits: Limits,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identify {
    pub class_action_id: String,
    pub species_action_id: String,
    pub background_action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextActions {
    pub name_action_id: String,
    pub appearance_action_id: String,
    pub backstory_action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub scores: PendingField,
    pub name: PendingField,
    pub appearance: PendingField,
    pub backstory: PendingField,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingField {
    pub condition_id: String,
    pub open_action_id: String,
    pub close_action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityState {
    pub ability: String,
    /// Literal score-input aliases, exported from the unchanged TS parser.
    pub aliases: Vec<String>,
    pub state_id: String,
}
/// Wire syntax observed in candidate speech. None of these tokens is an
/// executable action reference or a source of dispatch authority.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuTokens {
    pub namespace_prefix: String,
    pub array_adopt_token: InputToken,
    pub class_prefix: String,
    pub species_prefix: String,
    pub background_prefix: String,
    pub skill_prefix: String,
    pub choice_prefix: String,
    pub cantrip_prefix: String,
    pub prepared_prefix: String,
    pub kit_prefix: String,
    pub parsed_scores_prefix: String,
    pub parsed_name_prefix: String,
    pub parsed_appearance_prefix: String,
    pub parsed_backstory_prefix: String,
    pub codecs: MenuCodecs,
    pub score_digits: u32,
    pub score_ability_order: Vec<String>,
}
impl MenuTokens {
    fn relative_prefixes(&self) -> [&str; 12] {
        [
            &self.class_prefix,
            &self.species_prefix,
            &self.background_prefix,
            &self.skill_prefix,
            &self.choice_prefix,
            &self.cantrip_prefix,
            &self.prepared_prefix,
            &self.kit_prefix,
            &self.parsed_scores_prefix,
            &self.parsed_name_prefix,
            &self.parsed_appearance_prefix,
            &self.parsed_backstory_prefix,
        ]
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuCodecs {
    pub slug: String,
    pub reference: String,
    pub choice: String,
    pub kit: String,
    pub adopted_text: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wizard {
    pub confirm_token: InputToken,
    pub discard_action_id: String,
    /// Reopens the question before the open one; no draft field is cleared.
    pub back_action_id: String,
    /// Leaves one reopened question as answered.
    pub forward_action_id: String,
    pub enter_name_action_id: String,
    pub describe_appearance_action_id: String,
    pub describe_backstory_action_id: String,
    pub set_scores_action_id: String,
    pub adopt_scores_action_id: String,
    pub adopt_name_action_id: String,
    pub adopt_appearance_action_id: String,
    pub adopt_backstory_action_id: String,
    pub phase_state_id: String,
    pub phase_actions: Vec<ValueAction>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueAction {
    pub value: u32,
    pub action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetSet {
    pub state_id: String,
    pub set_action_id: String,
    pub clear_action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub slug: String,
    pub identity_tag: String,
    pub entity_definition_id: String,
    pub pick_token: InputToken,
    pub advance_action_id: String,
    pub sheet_action_id: String,
    pub advancement_tags: Vec<AdvancementTag>,
    pub catalog: ClassCatalog,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancementTag {
    pub level: u32,
    pub tag: String,
    pub missing_capability_definitions: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClassCatalog {
    pub class_slug: String,
    pub skills: Skills,
    pub choice_groups: Vec<ChoiceGroup>,
    pub cantrips_known: u32,
    pub cantrip_options: Vec<SpellOption>,
    pub prepared_count: u32,
    pub prepared_options: Vec<SpellOption>,
    pub kit: KitMenu,
    pub equipment_choices: Vec<EquipmentMenu>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Skills {
    pub choose_n: u32,
    pub skill_names: Vec<String>,
    pub skill_capability_ids: Vec<String>,
    pub source_text: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChoiceGroup {
    pub choice_group: String,
    pub choose_n: u32,
    pub member_capability_ids: Vec<String>,
    /// The definition each member grants (`srd.advancement_choice`
    /// `grants_ref`), parallel to `member_capability_ids`. Absent in a
    /// document sealed before sk-jay0.1.76.19.20; readers then fall back to
    /// the member id.
    #[serde(default)]
    pub member_grant_refs: Vec<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub recommended: Option<String>,
    pub rechoose: String,
}

impl ChoiceGroup {
    /// The definition a member grants, when the document carries it.
    pub fn grant_ref(&self, member: &str) -> Option<&str> {
        if self.member_grant_refs.len() != self.member_capability_ids.len() {
            return None;
        }
        self.member_capability_ids
            .iter()
            .position(|id| id == member)
            .and_then(|index| self.member_grant_refs.get(index))
            .map(String::as_str)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SpellOption {
    pub slug: String,
    pub capability_id: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub definition_ref: Option<String>,
    pub display_name: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KitMenu {
    pub options: Vec<KitOption>,
    pub source_text: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitOption {
    pub letter: String,
    pub description: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EquipmentMenu {
    pub letter: String,
    pub source_text: String,
    pub uses_tool_proficiency: bool,
    pub options: Vec<EquipmentOption>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EquipmentOption {
    pub entity_def_id: String,
    pub display_name: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub proficiency_capability_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub slug: String,
    pub identity_tag: String,
    pub pick_token: InputToken,
    pub definition_ref: String,
}

/// Observed input vocabulary, deliberately distinct from executable action
/// references. A token confers no dispatch authority or kernel-row existence.
#[derive(Clone, Debug, Deserialize)]
#[serde(transparent)]
pub struct InputToken(String);
impl InputToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Training {
    pub skill: TrainingKind,
    pub choice_member: TrainingKind,
    pub cantrip: TrainingKind,
    pub prepared_spell: TrainingKind,
    pub proficiency: TrainingKind,
    pub origin_feat: TrainingKind,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingKind {
    #[serde(deserialize_with = "Option::deserialize")]
    pub action_id: Option<String>,
    pub members: Vec<TrainingMember>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingMember {
    pub capability_id: String,
    pub feature_tag: String,
    pub availability: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitChoice {
    pub source_text: String,
    pub entity_definition_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeKit {
    pub class_slug: String,
    pub letter: String,
    pub choices: Vec<KitChoice>,
    pub action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobKit {
    pub class_slug: String,
    pub letter: String,
    pub choices: Vec<KitChoice>,
    pub action_id: String,
    pub items: Vec<KitItem>,
}

trait KitShape {
    fn action_id(&self) -> &str;
    fn class_slug(&self) -> &str;
    fn items(&self) -> Option<&[KitItem]>;
}
impl KitShape for JobKit {
    fn action_id(&self) -> &str {
        &self.action_id
    }
    fn class_slug(&self) -> &str {
        &self.class_slug
    }
    fn items(&self) -> Option<&[KitItem]> {
        Some(&self.items)
    }
}
impl KitShape for RuntimeKit {
    fn action_id(&self) -> &str {
        &self.action_id
    }
    fn class_slug(&self) -> &str {
        &self.class_slug
    }
    fn items(&self) -> Option<&[KitItem]> {
        None
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitItem {
    pub entity_definition_id: String,
    pub count: u32,
    #[serde(deserialize_with = "Option::deserialize")]
    pub parent_index: Option<u32>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub slot_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarterKit {
    pub bundle_id: String,
    pub weapon_definition_id: String,
    pub weapon_slot_id: String,
    pub field_kit_definition_id: String,
    pub torch_definition_id: String,
    pub rations_definition_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationVocabulary {
    pub class_tag_prefix: String,
    pub menu_tokens: MenuTokens,
    pub speech_trace_kind_id: String,
    pub drafting_condition_id: String,
    pub level_state_id: String,
    pub hp_state_id: String,
    pub xp_state_id: String,
    pub appearance_state_id: String,
    pub backstory_state_id: String,
    pub ability_states: Vec<AbilityState>,
    pub speed_state_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub point_buy_min: u32,
    pub point_buy_max: u32,
    pub point_buy_budget: u32,
    pub max_name_chars: u32,
    pub max_appearance_chars: u32,
    pub max_backstory_chars: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advancement {
    pub max_level: u32,
    pub enroll_action_id: String,
    pub offer_action_id: String,
    pub apply_action_id: String,
    pub draft_open_action_id: String,
    pub draft_close_action_id: String,
    pub accepted_state_id: String,
    pub pending_hp_state_id: String,
    pub accept_action_id: String,
    pub decline_action_id: String,
    pub steady_action_id: String,
    pub daring_action_id: String,
    pub confirm_action_id: String,
    pub discard_action_id: String,
    pub ask_action_id: String,
    pub phase_condition_id: String,
    pub telepathy_action_id: String,
    pub teleport_action_id: String,
    pub return_action_id: String,
    pub limbo_node_id: String,
    pub return_node_id: String,
    pub draft_opener_policy: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dialog {
    pub say_action_id: String,
    pub speech_action_id: String,
    pub open_state_id: String,
    pub prompt_state_id: String,
    pub ask_menu_actions: Vec<ValueAction>,
    pub open_actions: Vec<ValueAction>,
    pub prompt_actions: Vec<ValueAction>,
    pub choice_actions: Vec<ValueAction>,
    pub runtime_prompt_selector: u32,
    pub slot_count: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Companions {
    pub offer: CompanionOffer,
    pub rogue: Rogue,
    pub level_sync_keys: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionOffer {
    pub accept_slot: u32,
    pub decline_slot: u32,
    pub open_count: u32,
    pub minimum_level: u32,
    pub agreement_state_id: String,
    pub accept_action_id: String,
    pub decline_action_id: String,
    pub reopen_action_id: String,
    pub reopen_terms: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rogue {
    pub entity_key: String,
    pub blueprint_id: String,
    pub spawn_node_id: String,
    pub agreement_state_id: String,
    pub spawned_state_id: String,
    pub join_action_id: String,
    pub initial_level: u32,
    pub resume_level: u32,
    pub hp_at_resume_level: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watches {
    pub initiative: Initiative,
    pub kill_xp: KillXp,
    pub quest_xp: QuestXp,
    pub bond: Bond,
    pub threshold: Threshold,
    pub quest_loot_enabled: bool,
    pub labels: WatchLabels,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Initiative {
    pub mode: String,
    pub binding_id: String,
    pub call_action_id: String,
    pub session_definition_id: String,
    pub trigger_capability_ids: Vec<String>,
    pub party_tag_id: String,
    pub hostile_tag_id: String,
    pub hostile_actor_keys: Vec<String>,
    /// Opt-in: the DM also opens initiative when one of these listed hostiles
    /// and a party body come to share a node (sk-jay0.3.27.4.2). Absent = off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_sight: Option<OnSight>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnSight {
    /// Members of `hostile_actor_keys` that open a fight on a meeting.
    pub actor_keys: Vec<String>,
    /// Conditions on a party body that keep its arrival from opening one.
    pub unseen_condition_ids: Vec<String>,
    /// Conditions on the hostile that switch the trigger off for it.
    pub peace_condition_ids: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KillXp {
    pub dead_condition_id: String,
    pub dead_condition_suffix: String,
    pub xp_value_state_id: String,
    pub xp_state_id: String,
    pub spend_action_id: String,
    pub award_action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestXp {
    pub quest_refs: Vec<String>,
    pub recipient_policy: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bond {
    pub companion_key: String,
    pub stone_definition_id: String,
    pub carried_state_id: String,
    pub recorded_state_id: String,
    pub companion_bonded_state_id: String,
    pub bond_condition_id: String,
    pub record_action_id: String,
    pub mark_action_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Threshold {
    pub hero_policy: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatchLabels {
    pub log_prefix: String,
    pub initiative_handle: String,
    pub kill_xp_handle: String,
    pub quest_xp_handle: String,
    pub bond_handle: String,
    pub threshold_handle: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationMenuLabels {
    pub class: String,
    pub background: String,
    pub species: String,
    pub skill: String,
    pub cantrip: String,
    pub prepared_spell: String,
    pub starting_kit: String,
    pub name: String,
    pub ability_scores: String,
    pub confirmation: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationPresentationTemplates {
    pub count_kind: String,
    pub kit_option: String,
    pub equipment_page: String,
    pub equipment_more: String,
    pub adopt_name: String,
    pub adopt_array: String,
    pub confirm: String,
    pub remaining_one: String,
    pub remaining_many: String,
    pub answer_option: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationMissingPhrases {
    pub class: String,
    pub background: String,
    pub species: String,
    pub name: String,
    pub skills: String,
    pub kit: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationListVocabulary {
    pub empty: String,
    pub pair_separator: String,
    pub separator: String,
    pub last_separator: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationInputFieldKeys {
    pub name: String,
    pub appearance: String,
    pub backstory: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationTextFieldAliases {
    pub name: Vec<String>,
    pub appearance: Vec<String>,
    pub backstory: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreationPresentationVocabulary {
    pub menu_labels: CreationMenuLabels,
    pub templates: CreationPresentationTemplates,
    pub missing_phrases: CreationMissingPhrases,
    pub list: CreationListVocabulary,
    pub spoken_confirm_aliases: Vec<String>,
    pub spoken_discard_aliases: Vec<String>,
    pub text_field_aliases: CreationTextFieldAliases,
}

// Placeholder names are protocol argument slots; prose and ordering are data.
fn valid_presentation_template(value: &str, expected: &[&str]) -> bool {
    if !nonblank(value) {
        return false;
    }
    let mut seen = BTreeSet::new();
    let mut rest = value;
    while let Some(start) = rest.find(['{', '}']) {
        if rest.as_bytes()[start] != b'{' {
            return false;
        }
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return false;
        };
        let key = &after[..end];
        if !expected.contains(&key) || !seen.insert(key) {
            return false;
        }
        rest = &after[end + 1..];
    }
    seen.len() == expected.len()
}
