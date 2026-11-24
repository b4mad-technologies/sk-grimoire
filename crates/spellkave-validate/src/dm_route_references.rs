//! Original act selector and spatial duty relations. Reader bindings never
//! participate. Runtime observations/target coverage still fence actual acts.
use crate::{
    dm_document_authority::{Binding, PrincipalPlan},
    dm_duties::DutySource,
    dm_duty_references::CanonicalWorlds,
};
use serde_json::json;
use std::collections::BTreeSet;

pub fn validate_duty_routes(
    source: &DutySource,
    original: &PrincipalPlan,
    worlds: &CanonicalWorlds,
) -> Result<(), Vec<String>> {
    let admitted = (|| -> Result<(), String> {
        if source.bundle_id != original.bundle_id || source.body_key != original.body_key {
            return Err("duty identity differs from original principal plan".into());
        }
        let routes = &source.routes;
        let selected: BTreeSet<_> = [
            &routes.region_binding_id,
            &routes.creation_binding_id,
            &routes.advancement_binding_id,
            &routes.graph_author_binding_id,
            &routes.bundle_author_binding_id,
        ]
        .into_iter()
        .map(String::as_str)
        .collect();
        let plan_ids: BTreeSet<_> = original
            .bindings
            .iter()
            .map(|row| row.binding_id.as_str())
            .collect();
        if selected.len() != 5 || selected != plan_ids || original.bindings.len() != 5 {
            return Err("duty routes must name exactly the five original act bindings".into());
        }
        let region = binding(original, &routes.region_binding_id)?;
        let creation = binding(original, &routes.creation_binding_id)?;
        let advancement = binding(original, &routes.advancement_binding_id)?;
        let author = binding(original, &routes.graph_author_binding_id)?;
        let bundle = binding(original, &routes.bundle_author_binding_id)?;
        for route in [region, creation, advancement, author] {
            if route.extent_kind_ref != "place_graph" {
                return Err("graph duty route has a non-graph extent".into());
            }
            worlds.row("place_graphs", &route.extent_ref)?;
        }
        if author.extent_ref != region.extent_ref
            || bundle.extent_kind_ref != "bundle"
            || bundle.extent_ref != source.bundle_id
        {
            return Err("author routes differ from declared region/bundle".into());
        }
        for (node, route) in [
            (&source.vessels.rest_node_id, region),
            (&source.vessels.creation_station_node_id, creation),
            (&source.advancement.limbo_node_id, advancement),
            (&source.advancement.return_node_id, region),
            (&source.companions.rogue.spawn_node_id, region),
        ] {
            require_node_in_route(node, route, worlds)?;
        }
        worlds.row("entity_definitions", &source.vessels.candidate_blueprint_id)?;
        validate_candidate_spawn(source, worlds)?;
        if let Ok(candidate) = worlds.row("entities", &source.vessels.candidate_key) {
            if candidate["entity_def_id"] != source.vessels.candidate_blueprint_id
                || candidate["place_node_id"] != source.vessels.creation_station_node_id
            {
                return Err("candidate blueprint/station differs from canonical entity".into());
            }
        }
        let body = worlds.row("entities", &source.body_key)?;
        if body["bundle_id"] != source.bundle_id
            || body["place_node_id"] != source.vessels.rest_node_id
        {
            return Err("body home differs from declared rest station".into());
        }
        if !selected.contains(source.watches.initiative.binding_id.as_str()) {
            return Err("initiative names a non-act binding".into());
        }
        Ok(())
    })();
    admitted.map_err(|error| vec![format!("dm-route-references: {error}")])
}

pub(crate) fn validate_candidate_spawn(
    source: &DutySource,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let action = worlds.row("action_definitions", &source.creation.open_seat_action_id)?;
    let consequence = worlds.row(
        "consequence_templates",
        action["on_success"]
            .as_str()
            .ok_or("open-seat action has no consequence")?,
    )?;
    let effects: Vec<_> = consequence["effect_refs_csv"]
        .as_str()
        .ok_or("open-seat action has no spawn effect")?
        .split(',')
        .map(str::trim)
        .collect();
    if effects.len() != 1 || effects[0].is_empty() {
        return Err("open-seat must select exactly one spawn effect".into());
    }
    let effect = worlds.row("effect_definitions", effects[0])?;
    if effect["primitives_csv"] != "Spawn" {
        return Err("open-seat effect is not the declared spawn primitive".into());
    }
    let parameters = crate::dm_document_authority::unique_json(
        effect["parameters_schema_json"]
            .as_str()
            .ok_or("missing spawn parameters")?,
    )
    .map_err(|errors| errors.join("; "))?;
    if parameters
        != json!({"blueprint_id":source.vessels.candidate_blueprint_id,"location_ref":{"kind":"binding","id":"TargetSpatialRef"},"unbound_actor":true})
    {
        return Err(
            "open-seat spawn parameters differ from the candidate blueprint/station binding".into(),
        );
    }
    Ok(())
}

fn binding<'a>(plan: &'a PrincipalPlan, id: &str) -> Result<&'a Binding, String> {
    plan.bindings
        .iter()
        .find(|row| row.binding_id == id)
        .ok_or_else(|| format!("missing original act binding {id}"))
}

fn require_node_in_route(
    node: &str,
    route: &Binding,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let mut graph = worlds.row("place_nodes", node)?["place_graph_id"]
        .as_str()
        .ok_or("node has no graph")?;
    let mut seen = BTreeSet::new();
    while !graph.is_empty() {
        if !seen.insert(graph) {
            return Err("cyclic place graph ancestry".into());
        }
        let row = worlds.row("place_graphs", graph)?;
        if graph == route.extent_ref {
            return Ok(());
        }
        graph = row["parent_graph_id"]
            .as_str()
            .ok_or("graph has no typed parent field")?;
    }
    Err(format!(
        "node {node} lies outside duty route {}",
        route.binding_id
    ))
}

/// Checks a principal-dispatched action against its declared original selector.
/// Hero input vocabulary and body fiction actions use different relations.
pub fn validate_principal_action(
    action: &str,
    selector: &str,
    original: &PrincipalPlan,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let route = binding(original, selector)?;
    let action = worlds.row("action_definitions", action)?;
    let capability = action["required_capability"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("action has no required capability")?;
    worlds.row("capability_definitions", capability)?;
    if !original
        .role_capabilities
        .get(&route.role_id)
        .is_some_and(|ids| ids.iter().any(|id| id == capability))
    {
        return Err("action capability is absent from its original selected role".into());
    }
    Ok(())
}

/// Explicit dispatch inventory. Observed hero input tokens/form action IDs are
/// not treated as principal-dispatched commands merely because they are IDs.
pub fn validate_dispatched_action_routes(
    source: &DutySource,
    original: &PrincipalPlan,
    worlds: &CanonicalWorlds,
) -> Result<(), Vec<String>> {
    let c = &source.creation;
    let a = &source.advancement;
    let r = &source.routes;
    let mut errors = vec![];
    let mut principal = |action: &str, selector: &str| {
        if let Err(error) = validate_principal_action(action, selector, original, worlds) {
            errors.push(format!("dm-act-references: {action}: {error}"));
        }
    };
    for id in [
        &c.open_seat_action_id,
        &c.open_action_id,
        &c.finish_action_id,
        &c.identify.class_action_id,
        &c.identify.species_action_id,
        &c.identify.background_action_id,
        &c.text.name_action_id,
        &c.text.appearance_action_id,
        &c.text.backstory_action_id,
    ] {
        principal(id, &r.creation_binding_id);
    }
    for pending in [
        &c.pending.scores,
        &c.pending.name,
        &c.pending.appearance,
        &c.pending.backstory,
    ] {
        principal(&pending.open_action_id, &r.creation_binding_id);
        principal(&pending.close_action_id, &r.creation_binding_id);
    }
    for sheet in &c.sheet_sets {
        principal(&sheet.set_action_id, &r.creation_binding_id);
        principal(&sheet.clear_action_id, &r.creation_binding_id);
    }
    for class in &c.classes {
        principal(&class.advance_action_id, &r.creation_binding_id);
        principal(&class.sheet_action_id, &r.creation_binding_id);
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
            principal(action, &r.creation_binding_id);
        }
    }
    for kit in &c.kits {
        principal(&kit.action_id, &r.creation_binding_id);
    }
    for action in [&a.draft_open_action_id, &a.draft_close_action_id] {
        principal(action, &r.advancement_binding_id);
    }
    principal(
        &source.watches.initiative.call_action_id,
        &source.watches.initiative.binding_id,
    );
    for action in [
        &source.watches.kill_xp.spend_action_id,
        &source.watches.kill_xp.award_action_id,
        &source.watches.bond.mark_action_id,
        &source.watches.bond.record_action_id,
        &source.companions.rogue.join_action_id,
    ] {
        principal(action, &r.region_binding_id);
    }
    // Fiction powers remain on the assigned body. A source cannot move one to
    // the principal or claim a new body power by naming an arbitrary action.
    for action in [
        &c.induct_action_id,
        &a.enroll_action_id,
        &a.offer_action_id,
        &a.apply_action_id,
        &a.telepathy_action_id,
        &a.teleport_action_id,
        &a.return_action_id,
    ] {
        let checked = (|| -> Result<(), String> {
            let row = worlds.row("action_definitions", action)?;
            let capability = row["required_capability"]
                .as_str()
                .ok_or("missing body action capability")?;
            worlds.row("capability_definitions", capability)?;
            if !original
                .fiction_capabilities
                .iter()
                .any(|id| id == capability)
            {
                return Err("body action exceeds original fiction powers".into());
            }
            Ok(())
        })();
        if let Err(error) = checked {
            errors.push(format!("dm-act-references: {action}: {error}"));
        }
    }
    // Dialog menus are self-only body acts granted by existing talker tags,
    // not frontier commands on a hero through the principal role.
    for action in source
        .dialog
        .ask_menu_actions
        .iter()
        .chain(&source.dialog.open_actions)
        .chain(&source.dialog.prompt_actions)
        .map(|row| row.action_id.as_str())
        .chain(
            source
                .creation
                .wizard
                .phase_actions
                .iter()
                .map(|row| row.action_id.as_str()),
        )
        .chain([source.dialog.speech_action_id.as_str()])
    {
        if let Err(error) = validate_existing_body_action(action, source, worlds) {
            errors.push(format!("dm-body-act-references: {action}: {error}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_existing_body_action(
    action: &str,
    source: &DutySource,
    worlds: &CanonicalWorlds,
) -> Result<(), String> {
    let action = worlds.row("action_definitions", action)?;
    let capability = action["required_capability"]
        .as_str()
        .ok_or("missing body capability")?;
    worlds.row("capability_definitions", capability)?;
    if worlds
        .rows("actor_capabilities")?
        .any(|row| row["actor_entity_key"] == source.body_key && row["capability_id"] == capability)
    {
        return Ok(());
    }
    let body = worlds.row("entities", &source.body_key)?;
    let definition = body["entity_def_id"]
        .as_str()
        .ok_or("body has no blueprint")?;
    if worlds
        .rows("entity_definition_initial_capability")?
        .any(|row| row["entity_def_id"] == definition && row["capability_id"] == capability)
    {
        return Ok(());
    }
    let mut tags = BTreeSet::new();
    for (table, key, id) in [
        ("entity_tag", "entity_key", source.body_key.as_str()),
        ("entity_definition_tag", "entity_def_id", definition),
    ] {
        for row in worlds.rows(table)?.filter(|row| row[key] == id) {
            if let Some(tag) = row["tag"].as_str() {
                tags.insert(tag);
            }
        }
    }
    if worlds.rows("tag_capability_grant")?.any(|row| {
        row["capability_id"] == capability
            && row["tag"].as_str().is_some_and(|tag| tags.contains(tag))
    }) {
        return Ok(());
    }
    Err("body action has no existing direct, blueprint or tag-derived capability".into())
}
