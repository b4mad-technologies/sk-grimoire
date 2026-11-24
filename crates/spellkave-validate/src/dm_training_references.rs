//! Training availability and exact executable tag-write relations. The full
//! compiler must first admit canonical metadata and independently close menus.
use crate::{
    dm_duties::{DutySource, TrainingKind},
    dm_duty_references::CanonicalWorlds,
    dm_reference_locators::ReferenceLocators,
    dm_reference_metadata::ReferenceMetadata,
};
use serde_json::json;
use std::collections::BTreeSet;

fn ids<'a>(values: impl Iterator<Item = &'a String>) -> BTreeSet<&'a str> {
    values.map(String::as_str).collect()
}

/// Checks the complete typed training member population, rederives executable
/// availability from canonical capabilities, and proves its declared tag and
/// action writes. No grants_ref substitution or missing-member omission.
/// Source catalog-to-canonical menu closure is a separate mandatory step.
pub fn validate_training_references(
    duties: &DutySource,
    metadata: &ReferenceMetadata,
    locators: &ReferenceLocators,
    worlds: &CanonicalWorlds,
) -> Result<(), Vec<String>> {
    let classes = &duties.creation.classes;
    let skill = ids(classes
        .iter()
        .flat_map(|class| class.catalog.skills.skill_capability_ids.iter())
        .chain(
            duties
                .srd
                .backgrounds
                .iter()
                .flat_map(|row| row.skill_capability_ids.iter()),
        ));
    let cantrip = ids(classes.iter().flat_map(|class| {
        class
            .catalog
            .cantrip_options
            .iter()
            .map(|row| &row.capability_id)
    }));
    let prepared = ids(classes.iter().flat_map(|class| {
        class
            .catalog
            .prepared_options
            .iter()
            .map(|row| &row.capability_id)
    }));
    let proficiency = ids(classes.iter().flat_map(|class| {
        class.catalog.equipment_choices.iter().flat_map(|menu| {
            menu.options
                .iter()
                .filter_map(|option| option.proficiency_capability_id.as_ref())
        })
    }));
    let origins = ids(duties.srd.backgrounds.iter().map(|row| &row.feat_entity_id));
    let choices: BTreeSet<_> = metadata
        .advancement_choices
        .keys()
        .map(String::as_str)
        .collect();
    let training = &duties.creation.training;
    let mut errors = vec![];
    // The four class-derived populations are collected from the creation menu,
    // which may offer a subset of the SRD classes (sk-jay0.1.55.60.19). Their
    // members must cover what the offered classes can train; the exact action
    // write below still ties every member to the world's declared domain.
    // Origin feats and choice members come from complete lists and stay equal.
    for (kind, expected, complete, label) in [
        (&training.skill, skill, false, "skill"),
        (&training.cantrip, cantrip, false, "cantrip"),
        (&training.prepared_spell, prepared, false, "prepared spell"),
        (&training.proficiency, proficiency, false, "proficiency"),
        (&training.origin_feat, origins, true, "origin feat"),
        (&training.choice_member, choices, true, "choice member"),
    ] {
        if let Err(error) = validate_kind(kind, &expected, complete, locators, worlds) {
            errors.push(format!("dm-training-references: {label}: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_kind(
    kind: &TrainingKind,
    expected: &BTreeSet<&str>,
    complete: bool,
    locators: &ReferenceLocators,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let selected: BTreeSet<_> = kind
        .members
        .iter()
        .map(|row| row.capability_id.as_str())
        .collect();
    let covered = if complete {
        selected == *expected
    } else {
        expected.is_subset(&selected)
    };
    if !covered || selected.len() != kind.members.len() {
        return Err("training members differ from complete catalog/metadata population".into());
    }
    let mut grant_tags = vec![];
    for member in &kind.members {
        if member.feature_tag
            != format!(
                "{}{}",
                locators.feature_tag_prefix,
                member.capability_id.replace('/', ".")
            )
        {
            return Err("feature tag differs from captured identity mapping".into());
        }
        let grantable = worlds
            .row("capability_definitions", &member.capability_id)
            .is_ok();
        if member.availability
            != if grantable {
                "grantable"
            } else {
                "source-only"
            }
        {
            return Err(format!(
                "availability differs from canonical capability: {}",
                member.capability_id
            ));
        }
        if !grantable {
            continue;
        }
        worlds.row("tag_value_definitions", &member.feature_tag)?;
        worlds.keyed_row(
            "tag_capability_grant",
            &json!({"tag":member.feature_tag,"capability_id":member.capability_id}),
        )?;
        let bridged: BTreeSet<_> = worlds
            .rows("tag_capability_grant")?
            .filter(|row| row["tag"] == member.feature_tag)
            .filter_map(|row| row["capability_id"].as_str())
            .collect();
        if bridged != BTreeSet::from([member.capability_id.as_str()]) {
            return Err("training feature tag grants additional capabilities".into());
        }
        grant_tags.push(member.feature_tag.as_str());
    }
    if grant_tags.is_empty() {
        return if kind.action_id.is_none() {
            Ok(())
        } else {
            Err("source-only group names an executable training action".into())
        };
    }
    let action = kind
        .action_id
        .as_ref()
        .ok_or("grantable training group has no action")?;
    let writes = worlds.pure_action_writes(action)?;
    if writes.len() != 1
        || writes[0].frame_write
            != json!({"SetEntityTag":{"entity":"TargetActor","tag":{"allowed_ids":grant_tags},"present":true}})
    {
        return Err("training action is not the exact selected-feature tag write".into());
    }
    Ok(())
}
