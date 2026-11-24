//! Complete canonical choice and spell-menu population/tuple checks.
use crate::{
    dm_duties::{DutySource, SpellOption},
    dm_duty_references::CanonicalWorlds,
    dm_reference_locators::ReferenceLocators,
    dm_reference_metadata::ReferenceMetadata,
};
use std::collections::BTreeSet;

pub fn validate_canonical_menus(
    duties: &DutySource,
    locators: &ReferenceLocators,
    metadata: &ReferenceMetadata,
    worlds: &CanonicalWorlds,
) -> Result<(), Vec<String>> {
    let mut errors = vec![];
    // Identity display names are recognized as spoken picks after confirm /
    // discard. Those aliases must not intercept a canonical identity choice.
    let aliases: BTreeSet<_> = duties
        .creation
        .presentation_vocabulary
        .spoken_confirm_aliases
        .iter()
        .chain(
            &duties
                .creation
                .presentation_vocabulary
                .spoken_discard_aliases,
        )
        .map(String::as_str)
        .collect();
    for reference in duties
        .creation
        .classes
        .iter()
        .map(|row| &row.entity_definition_id)
        .chain(
            duties
                .creation
                .species
                .iter()
                .map(|row| &row.definition_ref),
        )
        .chain(
            duties
                .creation
                .backgrounds
                .iter()
                .map(|row| &row.definition_ref),
        )
    {
        match worlds.row("entity_definitions", reference) {
            Ok(row) => {
                if row["display_name"]
                    .as_str()
                    .is_some_and(|name| aliases.contains(name.to_ascii_lowercase().as_str()))
                {
                    errors.push("dm-menu-references: creation spoken alias collides with a canonical display name".into());
                }
            }
            Err(error) => errors.push(error),
        }
    }
    for class in &duties.creation.classes {
        let admitted = (|| -> Result<(), String> {
            if class.sheet_action_id
                != format!("{}{}", locators.creation_sheet_action_prefix, class.slug)
            {
                return Err("class sheet action differs from its captured class selector".into());
            }
            let source = &class.catalog.choice_groups;
            let canonical = metadata.choice_groups_for(&class.slug, 1);
            if source.len() != canonical.len()
                || source.iter().zip(&canonical).any(|(a, b)| {
                    a.choice_group != b.choice_group
                        || a.choose_n != b.choose_n
                        || a.member_capability_ids != b.member_capability_ids
                        || a.member_grant_refs != b.member_grant_refs
                        || a.recommended != b.recommended
                        || a.rechoose != b.rechoose
                })
            {
                return Err("choice groups differ from canonical metadata projection".into());
            }
            let profile = metadata
                .caster_profiles
                .values()
                .find(|row| row.class == class.slug);
            let cantrips = profile
                .and_then(|row| row.cantrips_known_by_level.as_ref())
                .and_then(|map| map.get("1"))
                .copied()
                .unwrap_or(0);
            let prepared = profile
                .and_then(|row| row.prepared_spells_by_level.get("1"))
                .copied()
                .unwrap_or(0);
            for (level, count, selected_count, options, suffix) in [
                (
                    0,
                    cantrips,
                    class.catalog.cantrips_known,
                    &class.catalog.cantrip_options,
                    &locators.cantrip_capability_suffix,
                ),
                (
                    1,
                    prepared,
                    class.catalog.prepared_count,
                    &class.catalog.prepared_options,
                    &locators.prepared_spell_capability_suffix,
                ),
            ] {
                let expected = if count == 0 {
                    BTreeSet::new()
                } else {
                    let tag = locators
                        .spell_list_tags
                        .iter()
                        .find(|row| row.class_slug == class.slug)
                        .ok_or("missing caster list locator")?;
                    let level_tag = locators
                        .spell_level_tags
                        .iter()
                        .find(|row| row.level == level)
                        .ok_or("missing spell-level locator")?;
                    let members = |tag: &str| -> Result<BTreeSet<String>, String> {
                        Ok(worlds
                            .rows(&locators.entity_definition_tag_table)?
                            .filter(|row| {
                                row["bundle_id"] == locators.ruleset_bundle_id && row["tag"] == tag
                            })
                            .filter_map(|row| row["entity_def_id"].as_str().map(str::to_owned))
                            .collect())
                    };
                    members(&tag.tag)?
                        .intersection(&members(&level_tag.tag)?)
                        .cloned()
                        .collect()
                };
                if u64::from(selected_count) != count.min(expected.len() as u64) {
                    return Err("spell pick count differs from canonical profile/list clamp".into());
                }
                let selected: BTreeSet<_> = options
                    .iter()
                    .filter_map(|row| row.definition_ref.clone())
                    .collect();
                if selected != expected || selected.len() != options.len() {
                    return Err("spell menu omits, duplicates or invents canonical members".into());
                }
                for option in options {
                    validate_spell_tuple(option, suffix, locators, worlds)?;
                }
            }
            for menu in &class.catalog.equipment_choices {
                let kit = class
                    .catalog
                    .kit
                    .options
                    .iter()
                    .find(|row| row.letter == menu.letter)
                    .ok_or("equipment menu references an unknown kit letter")?;
                if !kit.description.contains(&menu.source_text) {
                    return Err("equipment query text is absent from its source kit".into());
                }
                let tags: Vec<_> = [
                    (
                        &locators.equipment_category_phrases.instrument,
                        &locators.equipment_category_tags.instrument,
                    ),
                    (
                        &locators.equipment_category_phrases.artisan,
                        &locators.equipment_category_tags.artisan,
                    ),
                ]
                .into_iter()
                .filter(|(phrase, _)| menu.source_text.contains(phrase.as_str()))
                .map(|(_, tag)| tag.as_str())
                .collect();
                let expected: BTreeSet<_> = worlds
                    .rows(&locators.entity_definition_tag_table)?
                    .filter(|row| {
                        row["bundle_id"] == locators.ruleset_bundle_id
                            && row["tag"].as_str().is_some_and(|tag| tags.contains(&tag))
                    })
                    .filter_map(|row| row["entity_def_id"].as_str())
                    .collect();
                let selected: BTreeSet<_> = menu
                    .options
                    .iter()
                    .map(|row| row.entity_def_id.as_str())
                    .collect();
                if selected != expected || selected.len() != menu.options.len() {
                    return Err(
                        "equipment menu differs from complete canonical category population".into(),
                    );
                }
                for option in &menu.options {
                    let definition =
                        worlds.row(&locators.entity_definitions_table, &option.entity_def_id)?;
                    let proficiency = metadata
                        .equipment_proficiencies
                        .get(&option.entity_def_id)
                        .map(|row| &row.capability_id);
                    if definition["bundle_id"] != locators.ruleset_bundle_id
                        || definition["display_name"] != option.display_name
                        || option.proficiency_capability_id.as_ref() != proficiency
                    {
                        return Err(
                            "equipment name/proficiency tuple differs from canonical rows".into(),
                        );
                    }
                }
            }
            Ok(())
        })();
        if let Err(error) = admitted {
            errors.push(format!("dm-menu-references: {}: {error}", class.slug));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_spell_tuple(
    option: &SpellOption,
    suffix: &str,
    locators: &ReferenceLocators,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let id = option
        .definition_ref
        .as_ref()
        .ok_or("missing spell definition reference")?;
    if worlds.row(&locators.entity_definitions_table, id)?["bundle_id"]
        != locators.ruleset_bundle_id
        || id != &format!("{}{}", locators.spell_definition_prefix, option.slug)
        || id.rsplit('/').next() != Some(option.slug.as_str())
        || option.capability_id
            != format!(
                "{}{}{}",
                locators.spell_capability_prefix, option.slug, suffix
            )
    {
        return Err("spell tuple identity differs from canonical member/captured mapping".into());
    }
    let display = option
        .slug
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if option.display_name != display {
        return Err("spell display differs from source slug projection".into());
    }
    Ok(())
}
