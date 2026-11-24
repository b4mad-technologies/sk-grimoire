//! Explicit typed duty FK inventory. Relationships, authority and exact
//! consequences are checked separately; existence alone is not admission.
use crate::{dm_duties::DutySource, dm_duty_references::CanonicalWorlds};

pub fn validate_duty_reference_keys(
    source: &DutySource,
    worlds: &CanonicalWorlds,
) -> Result<(), Vec<String>> {
    let mut errors = vec![];
    let mut rows = |table: &str, ids: &[&str]| {
        for id in ids {
            if let Err(error) = worlds.row(table, id) {
                errors.push(error);
            }
        }
    };
    let c = &source.creation;
    let a = &source.advancement;
    let d = &source.dialog;
    let o = &c.observation_vocabulary;
    let r = &source.companions.rogue;
    let offer = &source.companions.offer;
    let b = &source.watches.bond;
    let xp = &source.watches.kill_xp;
    rows("bundles", &[&source.bundle_id, &c.starter_kit.bundle_id]);
    rows("entities", &[&source.body_key, &b.companion_key]);
    // The late companion may be blueprint-known without an instantiated row.
    for id in source
        .vessels
        .party_companion_keys
        .iter()
        .chain(&source.companions.level_sync_keys)
        .chain(&source.watches.initiative.hostile_actor_keys)
    {
        if id != &r.entity_key {
            rows("entities", &[id]);
        }
    }
    rows(
        "entity_definitions",
        &[
            &source.vessels.candidate_blueprint_id,
            &r.blueprint_id,
            &b.stone_definition_id,
            &c.starter_kit.weapon_definition_id,
            &c.starter_kit.field_kit_definition_id,
            &c.starter_kit.torch_definition_id,
            &c.starter_kit.rations_definition_id,
        ],
    );
    rows(
        "place_nodes",
        &[
            &source.vessels.creation_station_node_id,
            &source.vessels.rest_node_id,
            &a.limbo_node_id,
            &a.return_node_id,
            &r.spawn_node_id,
        ],
    );
    let initiative = &source.watches.initiative;
    for capability in &initiative.trigger_capability_ids {
        rows("capability_definitions", &[capability]);
    }
    rows(
        "tag_value_definitions",
        &[&initiative.party_tag_id, &initiative.hostile_tag_id],
    );
    rows("tag_value_definitions", &[&source.persona.tag]);
    rows("niche_definitions", &[&source.persona.niche_id]);
    rows(
        "session_definitions",
        &[&source.watches.initiative.session_definition_id],
    );
    rows("trace_kind_definitions", &[&o.speech_trace_kind_id]);
    rows(
        "equipment_slot_definitions",
        &[&c.starter_kit.weapon_slot_id],
    );
    rows(
        "condition_definitions",
        &[
            &o.drafting_condition_id,
            &a.phase_condition_id,
            &b.bond_condition_id,
            &xp.dead_condition_id,
        ],
    );
    if let Some(on_sight) = &source.watches.initiative.on_sight {
        let conditions: Vec<&str> = on_sight
            .unseen_condition_ids
            .iter()
            .chain(&on_sight.peace_condition_ids)
            .map(String::as_str)
            .collect();
        rows("condition_definitions", &conditions);
    }
    rows(
        "resource_definitions",
        &[
            &source.persona.applied_level_state_id,
            &c.kit_granted_state_id,
            &c.wizard.phase_state_id,
            &o.level_state_id,
            &o.hp_state_id,
            &o.xp_state_id,
            &o.speed_state_id,
            &a.accepted_state_id,
            &a.pending_hp_state_id,
            &offer.agreement_state_id,
            &r.agreement_state_id,
            &r.spawned_state_id,
            &b.carried_state_id,
            &b.recorded_state_id,
            &b.companion_bonded_state_id,
            &xp.xp_value_state_id,
            &xp.xp_state_id,
            &d.open_state_id,
            &d.prompt_state_id,
        ],
    );
    rows(
        "object_state_definitions",
        &[&o.appearance_state_id, &o.backstory_state_id],
    );
    for ability in &o.ability_states {
        rows("resource_definitions", &[&ability.state_id]);
    }
    for pending in [
        &c.pending.scores,
        &c.pending.name,
        &c.pending.appearance,
        &c.pending.backstory,
    ] {
        rows("condition_definitions", &[&pending.condition_id]);
        rows(
            "action_definitions",
            &[&pending.open_action_id, &pending.close_action_id],
        );
    }
    rows(
        "action_definitions",
        &[
            &c.open_seat_action_id,
            &c.open_action_id,
            &c.finish_action_id,
            &c.induct_action_id,
            &c.identify.class_action_id,
            &c.identify.species_action_id,
            &c.identify.background_action_id,
            &c.text.name_action_id,
            &c.text.appearance_action_id,
            &c.text.backstory_action_id,
            &c.wizard.discard_action_id,
            &c.wizard.back_action_id,
            &c.wizard.forward_action_id,
            &c.wizard.enter_name_action_id,
            &c.wizard.describe_appearance_action_id,
            &c.wizard.describe_backstory_action_id,
            &c.wizard.set_scores_action_id,
            &c.wizard.adopt_scores_action_id,
            &c.wizard.adopt_name_action_id,
            &c.wizard.adopt_appearance_action_id,
            &c.wizard.adopt_backstory_action_id,
            &a.enroll_action_id,
            &a.offer_action_id,
            &a.apply_action_id,
            &a.draft_open_action_id,
            &a.draft_close_action_id,
            &a.accept_action_id,
            &a.decline_action_id,
            &a.steady_action_id,
            &a.daring_action_id,
            &a.confirm_action_id,
            &a.discard_action_id,
            &a.ask_action_id,
            &a.telepathy_action_id,
            &a.teleport_action_id,
            &a.return_action_id,
            &d.say_action_id,
            &d.speech_action_id,
            &offer.accept_action_id,
            &offer.decline_action_id,
            &offer.reopen_action_id,
            &r.join_action_id,
            &source.watches.initiative.call_action_id,
            &xp.spend_action_id,
            &xp.award_action_id,
            &b.record_action_id,
            &b.mark_action_id,
        ],
    );
    for actions in [
        &c.wizard.phase_actions,
        &d.ask_menu_actions,
        &d.open_actions,
        &d.prompt_actions,
        &d.choice_actions,
    ] {
        for action in actions {
            rows("action_definitions", &[&action.action_id]);
        }
    }
    for sheet in &c.sheet_sets {
        rows("resource_definitions", &[&sheet.state_id]);
        rows(
            "action_definitions",
            &[&sheet.set_action_id, &sheet.clear_action_id],
        );
    }
    for class in &c.classes {
        rows("tag_value_definitions", &[&class.identity_tag]);
        rows("entity_definitions", &[&class.entity_definition_id]);
        rows(
            "action_definitions",
            &[&class.advance_action_id, &class.sheet_action_id],
        );
        for level in &class.advancement_tags {
            rows("tag_value_definitions", &[&level.tag]);
        }
        for skill in &class.catalog.skills.skill_capability_ids {
            rows("capability_definitions", &[skill]);
        }
    }
    for origin in c.species.iter().chain(&c.backgrounds) {
        rows("tag_value_definitions", &[&origin.identity_tag]);
        rows("entity_definitions", &[&origin.definition_ref]);
    }
    for training in [
        &c.training.skill,
        &c.training.choice_member,
        &c.training.cantrip,
        &c.training.prepared_spell,
        &c.training.proficiency,
        &c.training.origin_feat,
    ] {
        if let Some(action) = &training.action_id {
            rows("action_definitions", &[action]);
        }
    }
    for kit in &c.kits {
        rows("action_definitions", &[&kit.action_id]);
    }
    if let Err(error) = crate::dm_route_references::validate_candidate_spawn(source, worlds) {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
