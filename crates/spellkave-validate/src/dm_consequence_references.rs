//! State homes, phase handles, identity vocabularies and menu write relations.
use crate::{dm_duties::DutySource, dm_duty_references::CanonicalWorlds};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn exact(action: &str, expected: Vec<Value>, worlds: &CanonicalWorlds) -> Result<(), String> {
    let actual = worlds.pure_action_writes(action)?;
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(&expected)
            .enumerate()
            .any(|(ordinal, (row, value))| {
                row.ordinal as usize != ordinal || row.frame_write != *value
            })
    {
        return Err(format!(
            "action write set differs from declared state/quantity relation: {action}"
        ));
    }
    Ok(())
}
fn modify(actor: &str, resource: &str, delta: String) -> Value {
    json!({"ModifyResource":{"actor":actor,"resource":resource,"delta_source":delta}})
}
fn condition(
    action: &str,
    id: &str,
    present: bool,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let write = if present {
        json!({"ApplyStatus":{"target_kind":"entity","target_ref":"TargetActor","condition_definition_id":id}})
    } else {
        json!({"RemoveCondition":{"target_kind":"entity","target_ref":"TargetActor","condition_definition_id":id,"source_match":"Any"}})
    };
    exact(action, vec![write], worlds)
}
fn tags(action: &str, expected: BTreeSet<&str>, worlds: &CanonicalWorlds) -> Result<(), String> {
    tag_domain(action, expected, true, worlds)
}
/// `complete`: the declared tag domain must equal `expected`. Otherwise it
/// must contain it: the creation menu may offer a non-empty subset of the
/// classes whose identity tags the world's act declares (sk-jay0.1.55.60.19).
fn tag_domain(
    action: &str,
    expected: BTreeSet<&str>,
    complete: bool,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let writes = worlds.pure_action_writes(action)?;
    if writes.len() != 1 {
        return Err("identity/advancement action has additional writes".into());
    }
    let write = &writes[0].frame_write;
    let values = write["SetEntityTag"]["tag"]["allowed_ids"]
        .as_array()
        .ok_or("identity/advancement action has no tag domain")?;
    let actual: BTreeSet<_> = values.iter().filter_map(Value::as_str).collect();
    let covered = if complete {
        actual == expected
    } else {
        !expected.is_empty() && expected.is_subset(&actual)
    };
    if !covered
        || actual.len() != values.len()
        || *write
            != json!({"SetEntityTag":{"entity":"TargetActor","tag":{"allowed_ids":values},"present":true}})
    {
        return Err(
            "identity/advancement tag write differs from the complete declared domain".into(),
        );
    }
    Ok(())
}

pub fn validate_consequence_references(
    source: &DutySource,
    worlds: &CanonicalWorlds,
) -> Result<(), Vec<String>> {
    let c = &source.creation;
    let a = &source.advancement;
    let d = &source.dialog;
    let mut errors = vec![];
    let mut check = |result: Result<(), String>| {
        if let Err(error) = result {
            errors.push(format!("dm-consequence-references: {error}"));
        }
    };
    for action in &c.wizard.phase_actions {
        check(exact(
            &action.action_id,
            vec![modify(
                "Actor",
                &c.wizard.phase_state_id,
                format!(
                    "{} - actor.stat('{}')",
                    action.value, c.wizard.phase_state_id
                ),
            )],
            worlds,
        ));
    }
    check(exact(
        &a.enroll_action_id,
        vec![
            modify("Actor", &source.persona.applied_level_state_id, "0".into()),
            modify("TargetActor", &a.pending_hp_state_id, "0".into()),
            modify("TargetActor", &a.accepted_state_id, "0".into()),
        ],
        worlds,
    ));
    for (action, destination) in [
        (&a.teleport_action_id, &a.limbo_node_id),
        (&a.return_action_id, &a.return_node_id),
    ] {
        check(exact(action, ["TargetActor", "Actor"].into_iter().map(|actor|
            json!({"RelocateEntity":{"target_ref":actor,"destination":{"kind":"place_node","id":destination}}})
        ).collect(), worlds));
    }
    let bond = &source.watches.bond;
    check(exact(
        &bond.record_action_id,
        vec![
            json!({"ApplyStatus":{"target_kind":"entity","target_ref":"TargetActor","condition_definition_id":bond.bond_condition_id}}),
            modify("TargetActor", &bond.recorded_state_id, "1".into()),
        ],
        worlds,
    ));
    check(exact(
        &bond.mark_action_id,
        vec![modify(
            "TargetActor",
            &bond.companion_bonded_state_id,
            "1".into(),
        )],
        worlds,
    ));
    let xp = &source.watches.kill_xp;
    check(exact(
        &xp.spend_action_id,
        vec![modify(
            "TargetActor",
            &xp.xp_value_state_id,
            format!("-target.stat('{}')", xp.xp_value_state_id),
        )],
        worlds,
    ));
    check(exact(
        &source.watches.initiative.call_action_id,
        vec![
            json!({"OpenSession":{"session_def_id":source.watches.initiative.session_definition_id}}),
        ],
        worlds,
    ));
    check(condition(
        &c.open_action_id,
        &c.observation_vocabulary.drafting_condition_id,
        true,
        worlds,
    ));
    check(condition(
        &c.finish_action_id,
        &c.observation_vocabulary.drafting_condition_id,
        false,
        worlds,
    ));
    check(condition(
        &a.draft_open_action_id,
        &a.phase_condition_id,
        true,
        worlds,
    ));
    check(condition(
        &a.draft_close_action_id,
        &a.phase_condition_id,
        false,
        worlds,
    ));
    for pending in [
        &c.pending.scores,
        &c.pending.name,
        &c.pending.appearance,
        &c.pending.backstory,
    ] {
        check(condition(
            &pending.open_action_id,
            &pending.condition_id,
            true,
            worlds,
        ));
        check(condition(
            &pending.close_action_id,
            &pending.condition_id,
            false,
            worlds,
        ));
    }
    for sheet in &c.sheet_sets {
        check(exact(
            &sheet.set_action_id,
            vec![modify(
                "TargetActor",
                &sheet.state_id,
                format!(
                    "actor.stat('core/dispatch_arg_quantity') - target.stat('{}')",
                    sheet.state_id
                ),
            )],
            worlds,
        ));
        check(exact(
            &sheet.clear_action_id,
            vec![modify(
                "TargetActor",
                &sheet.state_id,
                format!("-target.stat('{}')", sheet.state_id),
            )],
            worlds,
        ));
    }
    check(exact(
        &c.text.name_action_id,
        vec![
            json!({"SetEntityText":{"entity":"TargetActor","destination":"EntityName","max_chars":c.limits.max_name_chars}}),
        ],
        worlds,
    ));
    for (action, state, limit) in [
        (
            &c.text.appearance_action_id,
            &c.observation_vocabulary.appearance_state_id,
            c.limits.max_appearance_chars,
        ),
        (
            &c.text.backstory_action_id,
            &c.observation_vocabulary.backstory_state_id,
            c.limits.max_backstory_chars,
        ),
    ] {
        check(exact(
            action,
            vec![
                json!({"SetEntityText":{"entity":"TargetActor","destination":{"ObjectStateText":{"state_id":state}},"max_chars":limit}}),
            ],
            worlds,
        ));
    }
    check(tag_domain(
        &c.identify.class_action_id,
        c.classes
            .iter()
            .map(|row| row.identity_tag.as_str())
            .collect(),
        false,
        worlds,
    ));
    check(tags(
        &c.identify.species_action_id,
        c.species
            .iter()
            .map(|row| row.identity_tag.as_str())
            .collect(),
        worlds,
    ));
    check(tags(
        &c.identify.background_action_id,
        c.backgrounds
            .iter()
            .map(|row| row.identity_tag.as_str())
            .collect(),
        worlds,
    ));
    for class in &c.classes {
        check(tags(
            &class.advance_action_id,
            class
                .advancement_tags
                .iter()
                .map(|row| row.tag.as_str())
                .collect(),
            worlds,
        ));
    }
    for (actions, state) in [
        (&d.open_actions, &d.open_state_id),
        (&d.prompt_actions, &d.prompt_state_id),
    ] {
        for action in actions {
            check(exact(
                &action.action_id,
                vec![modify(
                    "Actor",
                    state,
                    format!("{} - actor.stat('{state}')", action.value),
                )],
                worlds,
            ));
        }
    }
    for action in &d.ask_menu_actions {
        check(exact(
            &action.action_id,
            vec![
                json!({"Inscribe":{"kind":c.observation_vocabulary.speech_trace_kind_id,"location_kind":"binding","location_ref":"TargetSpatialRef","payload_json":"{}","carry_dispatch_text":true,"carry_addressee":true,"carry_dispatch_options":true,"carry_dispatch_context":true}}),
                modify(
                    "Actor",
                    &d.prompt_state_id,
                    format!(
                        "{} - actor.stat('{}')",
                        d.runtime_prompt_selector, d.prompt_state_id
                    ),
                ),
                modify(
                    "Actor",
                    &d.open_state_id,
                    format!("{} - actor.stat('{}')", action.value, d.open_state_id),
                ),
            ],
            worlds,
        ));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
