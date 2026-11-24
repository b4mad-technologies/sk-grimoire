//! Job-only metadata query locators. These are never runtime document inputs.
use crate::{
    dm_document_authority::unique_json,
    dm_duties::{DutySource, MAX_REF_BYTES},
    dm_duty_references::CanonicalWorlds,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceLocators {
    pub schema_version: u32,
    pub ruleset_bundle_id: String,
    pub creation_sheet_action_prefix: String,
    pub spell_definition_prefix: String,
    pub spell_capability_prefix: String,
    pub cantrip_capability_suffix: String,
    pub prepared_spell_capability_suffix: String,
    pub feature_tag_prefix: String,
    pub metadata_attachments_table: String,
    pub capability_definitions_table: String,
    pub entity_definitions_table: String,
    pub entity_definition_tag_table: String,
    pub advancement_grant_namespace: String,
    pub advancement_choice_namespace: String,
    pub advancement_grant_target_kind: String,
    pub spellcasting_profile_namespace: String,
    pub spell_slot_grant_namespace: String,
    pub equipment_proficiency_namespace: String,
    pub equipment_proficiency_target_kind: String,
    pub spell_list_tags: Vec<SpellListTag>,
    pub spell_level_tag_prefix: String,
    pub spell_level_tags: Vec<SpellLevelTag>,
    pub equipment_category_tags: EquipmentCategoryTags,
    pub equipment_category_phrases: EquipmentCategoryPhrases,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpellListTag {
    pub class_slug: String,
    pub tag: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpellLevelTag {
    pub level: u32,
    pub tag: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentCategoryTags {
    pub instrument: String,
    pub artisan: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentCategoryPhrases {
    pub instrument: String,
    pub artisan: String,
}

/// Locator FK/domain facet only. Typed metadata payload admission and complete
/// duty/action/consequence relations remain mandatory in the full compiler.
pub fn admit_reference_locators_json(
    input: &str,
    duties: &DutySource,
    worlds: &CanonicalWorlds,
) -> Result<ReferenceLocators, Vec<String>> {
    let locators: ReferenceLocators = serde_json::from_value(unique_json(input)?)
        .map_err(|error| vec![format!("dm-locators: invalid shape: {error}")])?;
    let mut errors = vec![];
    let mut require = |valid: bool, message: &str| {
        if !valid {
            errors.push(format!("dm-locators: {message}"));
        }
    };
    require(locators.schema_version == 1, "unsupported schema version");
    require(
        nonblank(&locators.equipment_category_phrases.instrument)
            && nonblank(&locators.equipment_category_phrases.artisan)
            && locators.equipment_category_phrases.instrument
                != locators.equipment_category_phrases.artisan,
        "blank, oversized or aliased equipment query phrases",
    );
    for component in [
        &locators.creation_sheet_action_prefix,
        &locators.spell_definition_prefix,
        &locators.spell_capability_prefix,
        &locators.cantrip_capability_suffix,
        &locators.prepared_spell_capability_suffix,
        &locators.feature_tag_prefix,
    ] {
        require(nonblank(component), "blank or oversized identity component");
    }
    require(
        locators.cantrip_capability_suffix != locators.prepared_spell_capability_suffix,
        "spell capability kinds alias",
    );
    require(
        nonblank(&locators.ruleset_bundle_id),
        "missing ruleset bundle",
    );
    require(
        worlds.row("bundles", &locators.ruleset_bundle_id).is_ok(),
        "ruleset bundle FK missing",
    );
    for (actual, expected) in [
        (&locators.metadata_attachments_table, "metadata_attachments"),
        (
            &locators.capability_definitions_table,
            "capability_definitions",
        ),
        (&locators.entity_definitions_table, "entity_definitions"),
        (
            &locators.entity_definition_tag_table,
            "entity_definition_tag",
        ),
    ] {
        require(
            actual == expected,
            "table locator must name its registered manifest row kind",
        );
    }
    require(
        locators.advancement_grant_target_kind == "entity_definition"
            && locators.equipment_proficiency_target_kind == "entity_definition",
        "unsupported metadata target kind",
    );
    let namespaces = [
        &locators.advancement_grant_namespace,
        &locators.advancement_choice_namespace,
        &locators.spellcasting_profile_namespace,
        &locators.spell_slot_grant_namespace,
        &locators.equipment_proficiency_namespace,
    ];
    require(
        namespaces.iter().collect::<BTreeSet<_>>().len() == namespaces.len(),
        "namespace locators alias",
    );
    for namespace in namespaces {
        require(nonblank(namespace), "blank namespace locator");
        require(
            worlds
                .keyed_row("metadata_namespaces", &serde_json::json!({"namespace":namespace,"bundle_id":locators.ruleset_bundle_id}))
                .is_ok_and(|row| row["bundle_id"] == locators.ruleset_bundle_id),
            "namespace FK or owning bundle mismatch",
        );
    }
    let caster_classes: BTreeSet<_> = duties
        .srd
        .caster_profiles
        .iter()
        .map(|row| row.class_slug.as_str())
        .collect();
    let selected_classes: BTreeSet<_> = locators
        .spell_list_tags
        .iter()
        .map(|row| row.class_slug.as_str())
        .collect();
    require(
        selected_classes == caster_classes
            && selected_classes.len() == locators.spell_list_tags.len(),
        "spell-list locator domain must exactly match caster profiles",
    );
    let tags: Vec<_> = locators
        .spell_list_tags
        .iter()
        .map(|row| row.tag.as_str())
        .chain(locators.spell_level_tags.iter().map(|row| row.tag.as_str()))
        .chain([
            locators.equipment_category_tags.instrument.as_str(),
            locators.equipment_category_tags.artisan.as_str(),
        ])
        .collect();
    require(
        tags.iter().collect::<BTreeSet<_>>().len() == tags.len(),
        "tag locators alias",
    );
    for tag in tags {
        require(nonblank(tag), "blank tag locator");
        require(
            worlds
                .row("tag_value_definitions", tag)
                .is_ok_and(|row| row["bundle_id"] == locators.ruleset_bundle_id),
            "tag FK or owning bundle mismatch",
        );
    }
    require(
        !locators.spell_level_tags.is_empty()
            && locators
                .spell_level_tags
                .iter()
                .enumerate()
                .all(|(index, row)| {
                    row.level as usize == index
                        && row.tag == format!("{}{}", locators.spell_level_tag_prefix, row.level)
                }),
        "spell-level locators must form a contiguous unique domain",
    );
    require(
        nonblank(&locators.spell_level_tag_prefix),
        "blank spell-level query prefix",
    );
    let available: BTreeSet<_> = worlds
        .rows("tag_value_definitions")
        .map_err(|error| vec![error])?
        .filter(|row| {
            row["bundle_id"] == locators.ruleset_bundle_id
                && row["tag"]
                    .as_str()
                    .is_some_and(|tag| tag.starts_with(&locators.spell_level_tag_prefix))
        })
        .filter_map(|row| row["tag"].as_str())
        .collect();
    let selected: BTreeSet<_> = locators
        .spell_level_tags
        .iter()
        .map(|row| row.tag.as_str())
        .collect();
    require(
        selected == available,
        "spell-level locators must retain the complete canonical prefix population",
    );
    if errors.is_empty() {
        Ok(locators)
    } else {
        Err(errors)
    }
}
fn nonblank(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_REF_BYTES
}
