//! Slice 4.1 (#1968 R7, ADR-0087 task 4.1): the shared live-wiring
//! emission every `ActorBehavior` shape carries regardless of its own
//! topology — ported verbatim from the tail of `compose.ts`'s `clusterRows`
//! loop body (the "── Live wiring ──" comment block): the caster holding the
//! capability, the niche attached to its node, plus the optional
//! caster-entity/caster-actor/`bindToDefinition`/`defInstance` rows.
//!
//! Reuses `compose_ids` (this module's sibling) to derive `cap`/`niche` —
//! the same values `clusterRows` computes once per behavior and reuses
//! throughout, so this function never re-derives them independently.

use serde::Deserialize;
use serde_json::Value;

use super::ids::{compose_ids, IdsBehaviorInput, IdsConfigInput};
use crate::patch::support::{patched_tables, row, s, PatchResult};

/// `ActorBehavior.defInstance` — the #728 proof-instance sub-object.
#[derive(Deserialize)]
pub struct DefInstance {
    #[serde(rename = "entityKey")]
    pub entity_key: String,
    pub name: String,
    #[serde(default)]
    pub node: Option<String>,
}

/// The subset of `ActorBehavior` fields the live-wiring block reads, plus
/// the `ids()` inputs (`key`/`capabilityId`) it needs internally.
#[derive(Deserialize)]
pub struct LiveWiringBehaviorInput {
    pub key: String,
    #[serde(rename = "capabilityId", default)]
    pub capability_id: Option<String>,
    #[serde(default)]
    pub caster: Option<String>,
    #[serde(rename = "casterNode", default)]
    pub caster_node: Option<String>,
    #[serde(rename = "emitCasterEntity", default)]
    pub emit_caster_entity: bool,
    #[serde(rename = "casterEntityDefId", default)]
    pub caster_entity_def_id: Option<String>,
    #[serde(rename = "casterName", default)]
    pub caster_name: Option<String>,
    #[serde(rename = "casterKind", default)]
    pub caster_kind: Option<String>,
    #[serde(rename = "emitCasterActor", default)]
    pub emit_caster_actor: bool,
    #[serde(rename = "bindToDefinition", default)]
    pub bind_to_definition: Option<String>,
    #[serde(rename = "defInstance", default)]
    pub def_instance: Option<DefInstance>,
}

/// The subset of `BehaviorConfig` fields the live-wiring block (plus the
/// `ids()` it calls) reads.
#[derive(Deserialize)]
pub struct LiveWiringConfigInput {
    #[serde(rename = "bundleId")]
    pub bundle_id: String,
    #[serde(rename = "capPrefix")]
    pub cap_prefix: String,
    #[serde(rename = "actionPrefix")]
    pub action_prefix: String,
    #[serde(rename = "idNamespace", default)]
    pub id_namespace: Option<String>,
    #[serde(rename = "defaultCaster")]
    pub default_caster: String,
    #[serde(rename = "defaultCasterNode")]
    pub default_caster_node: String,
}

/// Emits the live-wiring rows for one behavior: `actor_capabilities` +
/// `spatial_niche_attachments` unconditionally, `entities` +
/// `actor_spatial_locations` when `emitCasterEntity` or `defInstance` is
/// set, `actors` when `emitCasterActor` or `defInstance` is set, and
/// `entity_definition_initial_capability` when `bindToDefinition` is set.
/// Returns a JSON object of only the table(s) touched (same "changed tables
/// only" contract `patch::support`'s module doc describes for the patch
/// ops) — every table key here is filtered to non-empty before return, since
/// (unlike a patch op upserting into a possibly-nonempty existing table) a
/// single-behavior live-wiring call either has rows to contribute or does
/// not.
pub fn compose_live_wiring_json(behavior_json: &str, cfg_json: &str) -> PatchResult<String> {
    let behavior: LiveWiringBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("compose live-wiring behavior JSON parse failed: {err}"))?;
    let cfg: LiveWiringConfigInput = serde_json::from_str(cfg_json)
        .map_err(|err| format!("compose live-wiring config JSON parse failed: {err}"))?;

    let ids_behavior = IdsBehaviorInput {
        key: behavior.key.clone(),
        capability_id: behavior.capability_id.clone(),
    };
    let ids_cfg = IdsConfigInput {
        cap_prefix: cfg.cap_prefix.clone(),
        action_prefix: cfg.action_prefix.clone(),
        id_namespace: cfg.id_namespace.clone(),
    };
    let ids = compose_ids(&ids_behavior, &ids_cfg);

    let caster = behavior.caster.clone().unwrap_or(cfg.default_caster);
    let caster_node = behavior
        .caster_node
        .clone()
        .unwrap_or(cfg.default_caster_node);

    let mut entities: Vec<Value> = Vec::new();
    let mut actor_spatial_locations: Vec<Value> = Vec::new();
    let mut actors: Vec<Value> = Vec::new();

    if behavior.emit_caster_entity {
        let mut entity_fields = vec![
            ("entity_key", s(caster.clone())),
            ("bundle_id", s(cfg.bundle_id.clone())),
        ];
        if let Some(def_id) = &behavior.caster_entity_def_id {
            entity_fields.push(("entity_def_id", s(def_id.clone())));
        }
        entity_fields.push((
            "name",
            s(behavior
                .caster_name
                .clone()
                .unwrap_or_else(|| caster.clone())),
        ));
        entity_fields.push((
            "kind",
            s(behavior
                .caster_kind
                .clone()
                .unwrap_or_else(|| "npc".to_owned())),
        ));
        entity_fields.push(("place_node_id", s(caster_node.clone())));
        entities.push(row(entity_fields));

        actor_spatial_locations.push(row([
            ("entity_key", s(caster.clone())),
            ("place_node_id", s(caster_node.clone())),
            ("bundle_id", s(cfg.bundle_id.clone())),
            ("selection_path", s(format!("manifest:{caster}"))),
        ]));
    }

    if behavior.emit_caster_actor {
        actors.push(row([
            ("entity_key", s(caster.clone())),
            ("bundle_id", s(cfg.bundle_id.clone())),
            (
                "name",
                s(behavior
                    .caster_name
                    .clone()
                    .unwrap_or_else(|| caster.clone())),
            ),
        ]));
    }

    let actor_capabilities = vec![row([
        ("actor_entity_key", s(caster.clone())),
        ("capability_id", s(ids.cap.clone())),
        ("source", s(caster.clone())),
        ("bundle_id", s(cfg.bundle_id.clone())),
    ])];

    let spatial_niche_attachments = vec![row([
        ("bundle_id", s(cfg.bundle_id.clone())),
        ("place_node_id", s(caster_node.clone())),
        ("niche_id", s(ids.niche)),
    ])];

    let mut entity_definition_initial_capability: Vec<Value> = Vec::new();
    if let Some(bind_to_definition) = &behavior.bind_to_definition {
        entity_definition_initial_capability.push(row([
            ("entity_def_id", s(bind_to_definition.clone())),
            ("capability_id", s(ids.cap.clone())),
            ("bundle_id", s(cfg.bundle_id.clone())),
        ]));
    }

    if let Some(def_instance) = &behavior.def_instance {
        // `b.defInstance.node ?? casterNode` — the SAME `casterNode` local
        // (`behavior.casterNode ?? cfg.defaultCasterNode`) the niche
        // attachment above already used, not `cfg.defaultCasterNode` alone.
        let node = def_instance
            .node
            .clone()
            .unwrap_or_else(|| caster_node.clone());
        entities.push(row([
            ("entity_key", s(def_instance.entity_key.clone())),
            ("bundle_id", s(cfg.bundle_id.clone())),
            (
                "entity_def_id",
                s(behavior.bind_to_definition.clone().unwrap_or_default()),
            ),
            ("name", s(def_instance.name.clone())),
            ("kind", s("npc".to_owned())),
            ("place_node_id", s(node.clone())),
        ]));
        actor_spatial_locations.push(row([
            ("entity_key", s(def_instance.entity_key.clone())),
            ("place_node_id", s(node)),
            ("bundle_id", s(cfg.bundle_id.clone())),
            (
                "selection_path",
                s(format!("manifest:{}", def_instance.entity_key)),
            ),
        ]));
        actors.push(row([
            ("entity_key", s(def_instance.entity_key.clone())),
            ("bundle_id", s(cfg.bundle_id.clone())),
            ("name", s(def_instance.name.clone())),
        ]));
    }

    let mut tables: Vec<(&'static str, Vec<Value>)> = vec![
        ("actor_capabilities", actor_capabilities),
        ("spatial_niche_attachments", spatial_niche_attachments),
    ];
    if !entities.is_empty() {
        tables.push(("entities", entities));
    }
    if !actor_spatial_locations.is_empty() {
        tables.push(("actor_spatial_locations", actor_spatial_locations));
    }
    if !actors.is_empty() {
        tables.push(("actors", actors));
    }
    if !entity_definition_initial_capability.is_empty() {
        tables.push((
            "entity_definition_initial_capability",
            entity_definition_initial_capability,
        ));
    }

    let value = patched_tables(tables);
    serde_json::to_string(&value)
        .map_err(|err| format!("compose live-wiring output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn always_emits_actor_capabilities_and_niche_attachment() {
        let behavior = json!({ "key": "firebolt" }).to_string();
        let cfg = json!({
            "bundleId": "srd",
            "capPrefix": "spell",
            "actionPrefix": "cast_",
            "defaultCaster": "srd/regdar",
            "defaultCasterNode": "srd_outdoor/cell_a",
        })
        .to_string();
        let out = compose_live_wiring_json(&behavior, &cfg).expect("should succeed");
        let parsed: Value = serde_json::from_str(&out).expect("valid JSON");
        let caps = parsed["actor_capabilities"].as_array().expect("array");
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0]["actor_entity_key"], json!("srd/regdar"));
        assert_eq!(caps[0]["capability_id"], json!("srd/spell/firebolt"));
        let niches = parsed["spatial_niche_attachments"]
            .as_array()
            .expect("array");
        assert_eq!(
            niches[0]["niche_id"],
            json!("capability:srd/spell/firebolt")
        );
        assert!(parsed.get("entities").is_none());
        assert!(parsed.get("actors").is_none());
    }

    #[test]
    fn emit_caster_entity_and_actor_add_their_rows() {
        let behavior = json!({
            "key": "goblin-bite",
            "emitCasterEntity": true,
            "emitCasterActor": true,
            "caster": "srd/goblin-1",
            "casterEntityDefId": "srd/monster/goblin",
            "casterKind": "npc",
        })
        .to_string();
        let cfg = json!({
            "bundleId": "srd",
            "capPrefix": "monster-action",
            "actionPrefix": "",
            "defaultCaster": "srd/regdar",
            "defaultCasterNode": "srd_outdoor/cell_a",
        })
        .to_string();
        let out = compose_live_wiring_json(&behavior, &cfg).expect("should succeed");
        let parsed: Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(parsed["entities"].as_array().expect("array").len(), 1);
        assert_eq!(parsed["actors"].as_array().expect("array").len(), 1);
        assert_eq!(
            parsed["entities"][0]["entity_def_id"],
            json!("srd/monster/goblin")
        );
    }

    #[test]
    fn bind_to_definition_and_def_instance_add_their_rows() {
        let behavior = json!({
            "key": "test-strike",
            "bindToDefinition": "srd/monster/cat",
            "defInstance": { "entityKey": "srd/proof-cat", "name": "Proof Cat" },
        })
        .to_string();
        let cfg = json!({
            "bundleId": "srd",
            "capPrefix": "monster-action",
            "actionPrefix": "",
            "defaultCaster": "srd/regdar",
            "defaultCasterNode": "srd_outdoor/cell_a",
        })
        .to_string();
        let out = compose_live_wiring_json(&behavior, &cfg).expect("should succeed");
        let parsed: Value = serde_json::from_str(&out).expect("valid JSON");
        let cap_grant = &parsed["entity_definition_initial_capability"][0];
        assert_eq!(cap_grant["entity_def_id"], json!("srd/monster/cat"));
        assert_eq!(
            cap_grant["capability_id"],
            json!("srd/monster-action/test-strike")
        );
        // def_instance's entities row appends AFTER any emitCasterEntity row —
        // here emitCasterEntity is unset, so it is the sole entities row.
        let proof_entity = &parsed["entities"][0];
        assert_eq!(proof_entity["entity_key"], json!("srd/proof-cat"));
        assert_eq!(proof_entity["entity_def_id"], json!("srd/monster/cat"));
        assert_eq!(proof_entity["place_node_id"], json!("srd_outdoor/cell_a"));
        assert_eq!(parsed["actors"][0]["entity_key"], json!("srd/proof-cat"));
    }
}
