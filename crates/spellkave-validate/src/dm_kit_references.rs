//! Finite kit selections and whole canonical recipe consequence relations.
use crate::{
    dm_duties::DutySource, dm_duty_references::CanonicalWorlds,
    equip_predicate::incompatibility_reason,
};
use serde_json::json;
use std::collections::BTreeSet;

pub fn validate_kit_references(
    duties: &DutySource,
    worlds: &CanonicalWorlds,
) -> Result<(), Vec<String>> {
    let mut errors = vec![];
    let marker = &duties.creation.kit_granted_state_id;
    match worlds.row("resource_definitions", marker) {
        Err(error) => errors.push(error),
        Ok(row) => {
            let bounds = row["min_value"]
                .as_i64()
                .zip(
                    row["initial_value"]
                        .as_i64()
                        .or_else(|| row["min_value"].as_i64()),
                )
                .zip(row["max_value"].as_i64());
            let granted = duties.creation.kit_granted_value as i64;
            if !bounds.is_some_and(|((min, initial), max)| {
                min <= initial && initial < granted && granted <= max
            }) {
                errors.push("dm-kit-references: completion marker must satisfy min <= initial < granted <= max".into());
            }
        }
    }
    let mut actions = BTreeSet::new();
    for kit in &duties.creation.kits {
        let admitted = (|| -> Result<(), String> {
            if !actions.insert(&kit.action_id) {
                return Err("kit action is reused".into());
            }
            let mut expected = vec![];
            let mut occupied = BTreeSet::new();
            for (index, item) in kit.items.iter().enumerate() {
                let definition = worlds.row("entity_definitions", &item.entity_definition_id)?;
                if item.count == 0 || (definition["stackable"] != true && item.count != 1) {
                    return Err("kit quantity violates canonical stackability".into());
                }
                let destination = if let Some(parent) = item.parent_index {
                    if parent as usize >= index {
                        return Err("kit parent is not an earlier inserted item".into());
                    }
                    json!({"ObjectContainer":{"object_ref":format!("InsertedItem:{parent}")}})
                } else {
                    json!({"ActorInventory":{"actor_ref":"TargetActor"}})
                };
                expected.push(json!({"InsertItem":{"item_def_id":item.entity_definition_id,"count":item.count,"destination":destination}}));
                if let Some(slot) = &item.slot_id {
                    worlds.row("equipment_slot_definitions", slot)?;
                    if item.parent_index.is_some() || !occupied.insert(slot) {
                        return Err("nested or multiply occupied kit equipment slot".into());
                    }
                    let predicate = definition["slot_compatibility_predicate"]
                        .as_str()
                        .ok_or("missing typed slot predicate")?;
                    if incompatibility_reason(predicate, slot).is_some() {
                        return Err("kit item is incompatible with its selected slot".into());
                    }
                    expected.push(json!({"EquipItem":{"actor_ref":"TargetActor","slot_id":slot,"source":{"ActorInventoryItem":{"actor_ref":"TargetActor","item_def_id":item.entity_definition_id}}}}));
                }
            }
            for choice in &kit.choices {
                if !kit
                    .items
                    .iter()
                    .any(|item| item.entity_definition_id == choice.entity_definition_id)
                {
                    return Err("chosen equipment is absent from its kit recipe".into());
                }
            }
            expected.push(json!({"ModifyResource":{"actor":"TargetActor","resource":marker,"delta_source":format!("{} - target.stat('{marker}')", duties.creation.kit_granted_value)}}));
            let writes = worlds.pure_action_writes(&kit.action_id)?;
            if writes.len() != expected.len()
                || writes
                    .iter()
                    .zip(&expected)
                    .enumerate()
                    .any(|(ordinal, (actual, expected))| {
                        actual.ordinal as usize != ordinal || actual.frame_write != *expected
                    })
            {
                return Err(
                    "kit recipe differs from the whole canonical action consequence".into(),
                );
            }
            Ok(())
        })();
        if let Err(error) = admitted {
            errors.push(format!("dm-kit-references: {}: {error}", kit.action_id));
        }
    }
    for class in &duties.creation.classes {
        for option in &class.catalog.kit.options {
            let admitted = (|| -> Result<(), String> {
                let kits: Vec<_> = duties
                    .creation
                    .kits
                    .iter()
                    .filter(|kit| kit.class_slug == class.slug && kit.letter == option.letter)
                    .collect();
                let menus: Vec<_> = class
                    .catalog
                    .equipment_choices
                    .iter()
                    .filter(|menu| menu.letter == option.letter)
                    .collect();
                let mut expected: BTreeSet<Vec<(String, String)>> = BTreeSet::from([vec![]]);
                for menu in menus {
                    let choices: Vec<_> = menu
                        .options
                        .iter()
                        .filter(|row| {
                            !menu.uses_tool_proficiency || row.proficiency_capability_id.is_some()
                        })
                        .collect();
                    // Bound enumeration by the already bounded source kit
                    // population, refusing impossible products before allocating.
                    let count = expected
                        .len()
                        .checked_mul(choices.len())
                        .filter(|count| *count > 0 && *count <= kits.len())
                        .ok_or(
                            "kit selections are missing or exceed the bounded source population",
                        )?;
                    let mut next = BTreeSet::new();
                    for prior in &expected {
                        for choice in &choices {
                            let mut selection = prior.clone();
                            selection
                                .push((menu.source_text.clone(), choice.entity_def_id.clone()));
                            next.insert(selection);
                        }
                    }
                    if next.len() != count {
                        return Err("ambiguous duplicate equipment selection".into());
                    }
                    expected = next;
                }
                let selected: BTreeSet<_> = kits
                    .iter()
                    .map(|kit| {
                        kit.choices
                            .iter()
                            .map(|row| (row.source_text.clone(), row.entity_definition_id.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                if selected != expected || selected.len() != kits.len() {
                    return Err(
                        "kit corpus does not cover every complete menu selection exactly once"
                            .into(),
                    );
                }
                Ok(())
            })();
            if let Err(error) = admitted {
                errors.push(format!(
                    "dm-kit-references: {} {}: {error}",
                    class.slug, option.letter
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
