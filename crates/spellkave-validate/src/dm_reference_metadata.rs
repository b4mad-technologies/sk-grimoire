//! Typed canonical metadata selected by the admitted Job-only query locators.
//! No runtime authority or complete duty-source admission is implied here.
use crate::{
    dm_document_authority::unique_json,
    dm_duties::{DutySource, MAX_REF_BYTES},
    dm_duty_references::CanonicalWorlds,
    dm_reference_locators::ReferenceLocators,
    dm_srd::CasterPayload,
};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

type ChoiceGroups<'a> = BTreeMap<(&'a str, u32, &'a str), Vec<(&'a str, &'a AdvancementChoice)>>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancementGrant {
    pub class: String,
    pub level: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancementChoice {
    pub class: String,
    pub level: u32,
    pub choice_group: String,
    pub choose_n: u32,
    pub rechoose: String,
    pub grants_ref: String,
    #[serde(default, deserialize_with = "present_string")]
    pub recommended: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotGrant {
    pub class: String,
    pub level: u32,
    pub count: u32,
    #[serde(default, deserialize_with = "present_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub recovery: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotGrants {
    grants: Vec<SlotGrant>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentProficiency {
    #[serde(rename = "capabilityId")]
    pub capability_id: String,
}

/// Keys retain metadata target identity. In particular choice proxy targets
/// remain distinct from grants_ref; a proxy is not promoted to a capability.
#[derive(Debug)]
pub struct ReferenceMetadata {
    pub advancement_grants: BTreeMap<String, AdvancementGrant>,
    pub advancement_choices: BTreeMap<String, AdvancementChoice>,
    pub advancement_choice_order: Vec<String>,
    pub caster_profiles: BTreeMap<String, CasterPayload>,
    pub slot_grants: BTreeMap<String, Vec<SlotGrant>>,
    pub equipment_proficiencies: BTreeMap<String, EquipmentProficiency>,
}

impl ReferenceMetadata {
    /// Mirrors the unchanged TS grouper: canonical source row order controls
    /// last-non-null recommendation, then groups/members are presented sorted.
    /// No map/hash iteration may choose the recommendation.
    pub fn choice_groups_for(&self, class: &str, level: u32) -> Vec<crate::dm_duties::ChoiceGroup> {
        let mut groups: BTreeMap<&str, crate::dm_duties::ChoiceGroup> = BTreeMap::new();
        for id in &self.advancement_choice_order {
            let row = &self.advancement_choices[id];
            if row.class != class || row.level != level {
                continue;
            }
            let group =
                groups
                    .entry(&row.choice_group)
                    .or_insert_with(|| crate::dm_duties::ChoiceGroup {
                        choice_group: row.choice_group.clone(),
                        choose_n: row.choose_n,
                        member_capability_ids: vec![],
                        member_grant_refs: vec![],
                        recommended: None,
                        rechoose: "none".into(),
                    });
            group.member_capability_ids.push(id.clone());
            group.member_grant_refs.push(row.grants_ref.clone());
            if row.recommended.is_some() {
                group.recommended.clone_from(&row.recommended);
            }
            if row.rechoose == "long_rest_one" {
                group.rechoose.clone_from(&row.rechoose);
            }
        }
        groups
            .into_values()
            .map(|mut group| {
                let mut members: Vec<_> = group
                    .member_capability_ids
                    .drain(..)
                    .zip(group.member_grant_refs.drain(..))
                    .collect();
                members.sort();
                (group.member_capability_ids, group.member_grant_refs) =
                    members.into_iter().unzip();
                group
            })
            .collect()
    }
}

fn present_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

fn nonblank(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_REF_BYTES
}

fn typed<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("invalid metadata payload: {error}"))
}

fn target<'a>(
    row: &'a Value,
    kind: &str,
    table: &str,
    locators: &ReferenceLocators,
    worlds: &CanonicalWorlds,
) -> Result<&'a str, String> {
    let id = row["target_ref"]
        .as_str()
        .filter(|id| nonblank(id))
        .ok_or("missing metadata target")?;
    if row["target_kind"] != kind
        || worlds.row(table, id)?["bundle_id"] != locators.ruleset_bundle_id
    {
        return Err("metadata target kind or owning bundle mismatch".into());
    }
    Ok(id)
}

fn class_level(duties: &DutySource, class: &str, level: u32) -> Result<(), String> {
    if !duties.srd.classes.iter().any(|row| row.slug == class)
        || level == 0
        || level > duties.advancement.max_level
    {
        return Err("foreign metadata class/level".into());
    }
    Ok(())
}

/// Requires locators already admitted with admit_reference_locators_json.
/// Validates every selected canonical payload before later menu/availability
/// rederivation. This facet intentionally does not compare source menus yet.
pub fn admit_reference_metadata(
    locators: &ReferenceLocators,
    duties: &DutySource,
    worlds: &CanonicalWorlds,
) -> Result<ReferenceMetadata, Vec<String>> {
    let mut result = ReferenceMetadata {
        advancement_grants: BTreeMap::new(),
        advancement_choices: BTreeMap::new(),
        advancement_choice_order: vec![],
        caster_profiles: BTreeMap::new(),
        slot_grants: BTreeMap::new(),
        equipment_proficiencies: BTreeMap::new(),
    };
    let selected = [
        locators.advancement_grant_namespace.as_str(),
        locators.advancement_choice_namespace.as_str(),
        locators.spellcasting_profile_namespace.as_str(),
        locators.spell_slot_grant_namespace.as_str(),
        locators.equipment_proficiency_namespace.as_str(),
    ];
    let mut errors = vec![];
    let mut slot_keys = BTreeSet::new();
    let mut caster_classes = BTreeSet::new();
    for row in worlds
        .rows(&locators.metadata_attachments_table)
        .map_err(|error| vec![error])?
    {
        if row["bundle_id"] != locators.ruleset_bundle_id {
            continue;
        }
        let Some(namespace) = row["namespace"].as_str().filter(|ns| selected.contains(ns)) else {
            continue;
        };
        let admitted = (|| -> Result<(), String> {
            let payload = row["payload_json"]
                .as_str()
                .ok_or("missing metadata payload JSON")?;
            let payload = unique_json(payload).map_err(|errors| errors.join("; "))?;
            if namespace == locators.advancement_grant_namespace {
                let id = target(
                    row,
                    "entity_definition",
                    &locators.entity_definitions_table,
                    locators,
                    worlds,
                )?;
                let grant: AdvancementGrant = typed(payload)?;
                class_level(duties, &grant.class, grant.level)?;
                if result.advancement_grants.insert(id.into(), grant).is_some() {
                    return Err("duplicate advancement grant target".into());
                }
            } else if namespace == locators.advancement_choice_namespace {
                let id = target(
                    row,
                    "entity_definition",
                    &locators.entity_definitions_table,
                    locators,
                    worlds,
                )?;
                let choice: AdvancementChoice = typed(payload)?;
                class_level(duties, &choice.class, choice.level)?;
                if !nonblank(&choice.choice_group)
                    || choice.choose_n == 0
                    || !matches!(choice.rechoose.as_str(), "none" | "long_rest_one")
                    || choice
                        .recommended
                        .as_ref()
                        .is_some_and(|value| !nonblank(value))
                {
                    return Err("invalid advancement choice domain".into());
                }
                if !nonblank(&choice.grants_ref) {
                    return Err("missing advancement choice grants_ref".into());
                }
                if worlds.row(&locators.entity_definitions_table, &choice.grants_ref)?["bundle_id"]
                    != locators.ruleset_bundle_id
                {
                    return Err("advancement choice grants_ref owner mismatch".into());
                }
                result.advancement_choice_order.push(id.into());
                if result
                    .advancement_choices
                    .insert(id.into(), choice)
                    .is_some()
                {
                    return Err("duplicate advancement choice target".into());
                }
            } else if namespace == locators.spellcasting_profile_namespace {
                let id = target(
                    row,
                    "entity_definition",
                    &locators.entity_definitions_table,
                    locators,
                    worlds,
                )?;
                let profile: CasterPayload = typed(payload)?;
                class_level(duties, &profile.class, 1)?;
                if !caster_classes.insert(profile.class.clone()) {
                    return Err("duplicate caster class".into());
                }
                let source = duties
                    .srd
                    .caster_payloads
                    .get(&profile.class)
                    .ok_or("missing source caster profile")?;
                // Typed equality, including complete level/count maps and the
                // optional cantrip map, without hardcoded SRD class/feature IDs.
                if profile.class != source.class
                    || profile.ability != source.ability
                    || profile.feature != source.feature
                    || profile.ritual_casting != source.ritual_casting
                    || profile.prepared_spells_by_level != source.prepared_spells_by_level
                    || profile.cantrips_known_by_level != source.cantrips_known_by_level
                {
                    return Err("source caster profile differs from canonical metadata".into());
                }
                result.caster_profiles.insert(id.into(), profile);
            } else if namespace == locators.spell_slot_grant_namespace {
                let id = target(
                    row,
                    "resource_definition",
                    "resource_definitions",
                    locators,
                    worlds,
                )?;
                // Two explicitly supported deployed schemas, selected only by
                // the declared grants field. An invalid aggregate never falls
                // back to a scalar or loses unknown fields.
                let grants = if payload.get("grants").is_some() {
                    typed::<SlotGrants>(payload)?.grants
                } else {
                    vec![typed::<SlotGrant>(payload)?]
                };
                if grants.is_empty() {
                    return Err("empty slot-grant aggregation".into());
                }
                for grant in &grants {
                    class_level(duties, &grant.class, grant.level)?;
                    if grant.count == 0
                        || grant.kind.as_ref().is_some_and(|value| !nonblank(value))
                        || grant
                            .recovery
                            .as_ref()
                            .is_some_and(|value| !nonblank(value))
                    {
                        return Err("invalid slot grant count/kind/recovery".into());
                    }
                    if !slot_keys.insert((id.to_owned(), grant.class.clone(), grant.level)) {
                        return Err("duplicate resource/class/level slot grant".into());
                    }
                }
                result.slot_grants.insert(id.into(), grants);
            } else {
                let id = target(
                    row,
                    "entity_definition",
                    &locators.entity_definitions_table,
                    locators,
                    worlds,
                )?;
                let proficiency: EquipmentProficiency = typed(payload)?;
                if !nonblank(&proficiency.capability_id)
                    || worlds.row(
                        &locators.capability_definitions_table,
                        &proficiency.capability_id,
                    )?["bundle_id"]
                        != locators.ruleset_bundle_id
                {
                    return Err("equipment proficiency capability FK/owner mismatch".into());
                }
                result
                    .equipment_proficiencies
                    .insert(id.into(), proficiency);
            }
            Ok(())
        })();
        if let Err(error) = admitted {
            errors.push(format!(
                "dm-reference-metadata: {namespace} {}: {error}",
                row["target_ref"]
            ));
        }
    }
    let expected: BTreeSet<_> = duties
        .srd
        .caster_profiles
        .iter()
        .map(|row| row.class_slug.clone())
        .collect();
    if caster_classes != expected {
        errors.push("dm-reference-metadata: incomplete canonical caster class population".into());
    }
    // The TS grouper takes choose_n from the first member. Recommendation
    // values can legitimately differ (the canonical choose-two expertise menu
    // recommends two different members); preserve that row order for the
    // existing last-non-null recommendation projection rather than repairing it.
    let mut groups = ChoiceGroups::new();
    for (target, choice) in &result.advancement_choices {
        groups
            .entry((&choice.class, choice.level, &choice.choice_group))
            .or_default()
            .push((target, choice));
    }
    for ((class, level, group), members) in groups {
        let counts: BTreeSet<_> = members.iter().map(|(_, row)| row.choose_n).collect();
        let recommendations: BTreeSet<_> = members
            .iter()
            .filter_map(|(_, row)| row.recommended.as_deref())
            .collect();
        let targets: BTreeSet<_> = members.iter().map(|(target, _)| *target).collect();
        if counts.len() != 1
            || counts.iter().any(|count| *count as usize > members.len())
            || !recommendations.is_subset(&targets)
        {
            errors.push(format!(
                "dm-reference-metadata: conflicting choice group {class}/{level}/{group}"
            ));
        }
    }
    if errors.is_empty() {
        Ok(result)
    } else {
        Err(errors)
    }
}
