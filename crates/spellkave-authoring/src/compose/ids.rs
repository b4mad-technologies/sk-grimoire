//! Slice 4.1 (#1968 R7, ADR-0087 task 4.1): `ids()` — the stable id
//! derivation every `ActorBehavior` shape shares. Ported VERBATIM from
//! `compose.ts`'s private `ids(b, cfg)` function: same namespace default
//! (`"srd"`), same `capabilityId` whole-id override, same five derived
//! strings. This is the piece the `ids()` hand-mirror incident (ADR-0087
//! Context) came from — `spellkave_define_behavior`'s first implementation
//! hand-copied this exact derivation rather than reusing it, because
//! `compose.ts` offered no bundle-generic namespace override at the time.
//! Porting it to Rust first (and alone) closes that class of bug by
//! construction: every future caller — TS or otherwise — reaches the SAME
//! compiled function, not a fresh hand-transcription.

use serde::Deserialize;
use serde_json::Value;

use crate::patch::support::{row, PatchResult};

/// The subset of `ActorBehavior` fields `ids()` reads. Grows in later
/// slices as more shapes need their own subset of the same JSON payload —
/// unknown fields are ignored (no `deny_unknown_fields`), so the full
/// `ActorBehavior` JSON a TS caller already has can be passed through as-is.
#[derive(Deserialize)]
pub struct IdsBehaviorInput {
    pub key: String,
    #[serde(rename = "capabilityId", default)]
    pub capability_id: Option<String>,
}

/// The subset of `BehaviorConfig` fields `ids()` reads.
#[derive(Deserialize)]
pub struct IdsConfigInput {
    #[serde(rename = "capPrefix")]
    pub cap_prefix: String,
    #[serde(rename = "actionPrefix")]
    pub action_prefix: String,
    #[serde(rename = "idNamespace", default)]
    pub id_namespace: Option<String>,
}

/// The five (six, counting `affordance`) stable ids one behavior derives:
/// capability, niche, action, outcome, affordance, success template ids.
/// Field names match `compose.ts`'s returned object verbatim.
pub struct ComposeIds {
    pub cap: String,
    pub niche: String,
    pub action: String,
    pub outcome: String,
    pub affordance: String,
    pub success: String,
}

/// `compose.ts`'s `ids(b, cfg)`, verbatim: `namespace = cfg.idNamespace ??
/// "srd"`; `cap = b.capabilityId ?? "<namespace>/<capPrefix>/<key>"`;
/// `action = "<namespace>/<actionPrefix><key>"`; the remaining four derive
/// from `cap`/`action` by fixed suffix.
pub fn compose_ids(behavior: &IdsBehaviorInput, cfg: &IdsConfigInput) -> ComposeIds {
    let namespace = cfg.id_namespace.as_deref().unwrap_or("srd");
    let cap = behavior
        .capability_id
        .clone()
        .unwrap_or_else(|| format!("{namespace}/{}/{}", cfg.cap_prefix, behavior.key));
    // `${namespace}/${cfg.actionPrefix}${b.key}` — `actionPrefix` already
    // carries its own trailing content (e.g. "cast_" or ""), so no extra
    // separator is inserted after it, same as compose.ts.
    let action = format!("{namespace}/{}{}", cfg.action_prefix, behavior.key);
    ComposeIds {
        niche: format!("capability:{cap}"),
        outcome: format!("{action}/outcome"),
        affordance: format!("{action}/affordance"),
        success: format!("{action}/success"),
        cap,
        action,
    }
}

/// Parses `behavior_json`/`cfg_json`, derives ids, and returns them as a JSON
/// object `{"cap":...,"niche":...,"action":...,"outcome":...,
/// "affordance":...,"success":...}`.
pub fn compose_ids_json(behavior_json: &str, cfg_json: &str) -> PatchResult<String> {
    let behavior: IdsBehaviorInput = serde_json::from_str(behavior_json)
        .map_err(|err| format!("compose ids behavior JSON parse failed: {err}"))?;
    let cfg: IdsConfigInput = serde_json::from_str(cfg_json)
        .map_err(|err| format!("compose ids config JSON parse failed: {err}"))?;
    let ids = compose_ids(&behavior, &cfg);
    let value = row([
        ("cap", Value::String(ids.cap)),
        ("niche", Value::String(ids.niche)),
        ("action", Value::String(ids.action)),
        ("outcome", Value::String(ids.outcome)),
        ("affordance", Value::String(ids.affordance)),
        ("success", Value::String(ids.success)),
    ]);
    serde_json::to_string(&value)
        .map_err(|err| format!("compose ids output JSON serialization failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_namespace_derives_srd_prefixed_ids() {
        let behavior = IdsBehaviorInput {
            key: "firebolt".to_owned(),
            capability_id: None,
        };
        let cfg = IdsConfigInput {
            cap_prefix: "spell".to_owned(),
            action_prefix: "cast_".to_owned(),
            id_namespace: None,
        };
        let ids = compose_ids(&behavior, &cfg);
        assert_eq!(ids.cap, "srd/spell/firebolt");
        assert_eq!(ids.niche, "capability:srd/spell/firebolt");
        assert_eq!(ids.action, "srd/cast_firebolt");
        assert_eq!(ids.outcome, "srd/cast_firebolt/outcome");
        assert_eq!(ids.affordance, "srd/cast_firebolt/affordance");
        assert_eq!(ids.success, "srd/cast_firebolt/success");
    }

    #[test]
    fn capability_id_override_wins_over_derived_cap() {
        let behavior = IdsBehaviorInput {
            key: "firebolt".to_owned(),
            capability_id: Some("srd/spell/fire-bolt".to_owned()),
        };
        let cfg = IdsConfigInput {
            cap_prefix: "spell".to_owned(),
            action_prefix: "cast_".to_owned(),
            id_namespace: None,
        };
        let ids = compose_ids(&behavior, &cfg);
        assert_eq!(ids.cap, "srd/spell/fire-bolt");
        assert_eq!(ids.niche, "capability:srd/spell/fire-bolt");
        // action derivation is independent of the capabilityId override.
        assert_eq!(ids.action, "srd/cast_firebolt");
    }

    #[test]
    fn bundle_generic_namespace_override_replaces_srd() {
        let behavior = IdsBehaviorInput {
            key: "aoe_blast".to_owned(),
            capability_id: None,
        };
        let cfg = IdsConfigInput {
            cap_prefix: "spell".to_owned(),
            action_prefix: "cast_".to_owned(),
            id_namespace: Some("aoe_blast".to_owned()),
        };
        let ids = compose_ids(&behavior, &cfg);
        assert_eq!(ids.cap, "aoe_blast/spell/aoe_blast");
        assert_eq!(ids.action, "aoe_blast/cast_aoe_blast");
    }
}
