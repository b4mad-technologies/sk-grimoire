//! Kernel bootstrap vocabulary shared by admission, offline validation and explanation.
//! Kept as an inline literal: no repository-file dependency in deployed WASM.

pub const CORE_BOOTSTRAP_MANIFEST: &str = r#"#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json
schema_version = "1.0.0"
[metadata]
profile = "core-bootstrap"

# Vocabulary for the core bundle, also covered by the frozen mechanics fixtures.

[[entity_definitions]]
entity_def_id = "core/presence"
bundle_id = "core"
display_name = "Role presence"

[[capability_definitions]]
capability_id = "core/world_admin"
bundle_id = "core"
actor_kind = "operator"
display_name = "World admin"
description = "Authorizes operator/DM act-as through typed extent-bounded grants."

# Core capability for applying authored manifest fragments.
# `capability_id`) is a same-value no-op, never a collision or duplicate.
[[capability_definitions]]
capability_id = "frontier.apply_manifest"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Apply World Manifest"
description = "Authoring capability that allows a bound actor to apply manifest fragments after genesis."

# Core capability for spawning an instance from an entity definition.
# the dual recipe's later core-bundle apply is a same-value no-op.
[[capability_definitions]]
capability_id = "frontier.spawn_entity_definition"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Spawn Entity Definition"
description = "Authoring capability that allows a bound actor to spawn a fresh instance of an entity_definition at runtime."

[[capability_definitions]]
actor_kind = "entity"
bundle_id = "core"
capability_id = "frontier.pace_time"
default_visibility = "Public"
description = "Allows a bound embodied caller to pace the world clock regardless of grant extent."
display_name = "Pace World Time"

# Core control condition and the stacking and removal policies it references.
[[condition_remove_policy_definitions]]
policy_id = "core/remove_policy/independent_lifetime"
bundle_id = "core"
display_name = "Independent lifetime"
remove_predicate_source = "0 == 1"

[[stacking_discipline_definitions]]
discipline_id = "core/stacking/single"
bundle_id = "core"
display_name = "Single active row"
stack_count_outcome_source = "stack_count"
expires_at_tick_outcome_source = "-1"

[[condition_definitions]]
condition_definition_id = "core/condition/controlled"
bundle_id = "core"
description = "Steering claim-lock (Item 20): applied to a remotely-steered entity while an autonomy claim holds it; the world_tick autonomy dispatcher skips any entity carrying an active instance of this condition. Not a PLAYER-facing condition (sk-eky) — infrastructure for claim/autonomy only."
display_name = "Controlled"
gate_predicate_source = ""
grants_capability_while_active = []
suppresses_capability_while_active = []
hooks = []
implies = []
modifier_template_id = ""
observable = false
remove_time_policy = "core/remove_policy/independent_lifetime"
stacking_rule = "core/stacking/single"

[[affordance_templates]]
template_id = "core/perceive_world_object"
bundle_id = "core"
niche_id = ""
capability_id = ""
display_name = "Perceive world object"
predicate_bool = true
outcome_template_id = ""

[[affordance_templates]]
template_id = "core/perceive_item_grounded"
bundle_id = "core"
niche_id = ""
capability_id = ""
display_name = "Perceive grounded item"
predicate_bool = true
outcome_template_id = ""

[[outcome_templates]]
outcome_template_id = "core/equip_item/outcome"
bundle_id = "core"
display_name = "Equip item"
outcome_bool = true
manipulation_action_id = "core/equip_item"

[[outcome_templates]]
outcome_template_id = "core/unequip_item/outcome"
bundle_id = "core"
display_name = "Unequip item"
outcome_bool = true
manipulation_action_id = "core/unequip_item"

[[affordance_templates]]
template_id = "core/equip_item/affordance"
bundle_id = "core"
niche_id = ""
capability_id = ""
display_name = "Equip item"
predicate_bool = true
outcome_template_id = "core/equip_item/outcome"

[[affordance_templates]]
template_id = "core/unequip_item/affordance"
bundle_id = "core"
niche_id = ""
capability_id = ""
display_name = "Unequip item"
predicate_bool = true
outcome_template_id = "core/unequip_item/outcome"

[[consequence_templates]]
template_id = "core/equip_item/success"
bundle_id = "core"
effect_refs_csv = ""
traces_json = "[]"

[[consequence_templates]]
template_id = "core/unequip_item/success"
bundle_id = "core"
effect_refs_csv = ""
traces_json = "[]"

[[consequence_frame_writes]]
template_id = "core/equip_item/success"
ordinal = 0
frame_write = { EquipItem = { actor_ref = "Actor", slot_id = "core/main_hand", source = { TargetItem = {} } } }

[[consequence_frame_writes]]
template_id = "core/unequip_item/success"
ordinal = 0
frame_write = { UnequipItem = { actor_ref = "Actor", slot_id = "core/main_hand", source = { TargetItem = {} } } }

[[action_definitions]]
action_id = "core/equip_item"
bundle_id = "core"
required_capability = ""
on_success = "core/equip_item/success"
display_name = "Equip item"
cost_time_ms = 500

[[action_definitions]]
action_id = "core/equip_item/affordance"
bundle_id = "core"
required_capability = ""
on_success = "core/equip_item/success"
display_name = "Equip item"
cost_time_ms = 500

[[action_definitions]]
action_id = "core/unequip_item"
bundle_id = "core"
required_capability = ""
on_success = "core/unequip_item/success"
display_name = "Unequip item"
cost_time_ms = 500

[[action_definitions]]
action_id = "core/unequip_item/affordance"
bundle_id = "core"
required_capability = ""
on_success = "core/unequip_item/success"
display_name = "Unequip item"
cost_time_ms = 500

[[capability_definitions]]
capability_id = "frontier.read_hidden"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Read Hidden"
description = "Defines authority to read hidden world rows inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.read_internal"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Read Internal"
description = "Defines authority to read internal world rows inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.prune"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Prune Authored Content"
description = "Defines authority to prune authored content inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.escalate"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Escalate Authority"
description = "Defines authority to author a capability set beyond the caller’s own realized authority."

[[capability_definitions]]
capability_id = "frontier.impersonate"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Impersonate Actor"
description = "Defines authority to steer another actor inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.drive"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Assign NPC Driver"
description = "Defines authority to assign durable Service drivers to unoffered NPC bodies inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.break_seal"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Break Seal"
description = "Defines authority to open a sealed extent inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.bind.viewer"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Bind viewer"
description = "Defines authority to bind the viewer role inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.bind.timekeeper"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Bind timekeeper"
description = "Defines authority to bind the timekeeper role inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.bind.dm"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Bind dm"
description = "Defines authority to bind the dm role inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.bind.adventure-author"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Bind adventure-author"
description = "Defines authority to bind the adventure-author role inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.bind.system-author"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Bind system-author"
description = "Defines authority to bind the system-author role inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.despawn"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Despawn Entity"
description = "Defines authority to remove an entity through an authored act inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.set_state"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Set State"
description = "Defines authority to change world state through an authored act inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.transfer"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Transfer Entity"
description = "Defines authority to transfer an entity through an authored act inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.afflict"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Afflict Entity"
description = "Defines authority to apply a condition through an authored act inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.inscribe"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Inscribe Trace"
description = "Defines authority to inscribe a trace through an authored act inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.deliver_private_hint"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Deliver Private Hint"
description = "Defines authority to deliver a private hint through an authored act inside a realized binding extent."

[[capability_definitions]]
capability_id = "frontier.open_session"
bundle_id = "core"
actor_kind = "entity"
default_visibility = "Public"
display_name = "Open Session"
description = "Defines authority to open a session through an authored act inside a realized binding extent."

[[role_definitions]]
role_id = "warden"
bundle_id = "core"
display_name = "Warden"
description = "Breaks a place seal through an authored act inside a realized place-graph binding extent."
capability_ids = ["frontier.break_seal"]

[[role_definitions]]
role_id = "viewer"
bundle_id = "core"
display_name = "Viewer"
description = "Reads hidden rows inside a realized binding extent."
capability_ids = ["frontier.read_hidden"]

[[role_definitions]]
role_id = "timekeeper"
bundle_id = "core"
display_name = "Timekeeper"
description = "Paces the world clock."
capability_ids = ["frontier.pace_time"]

[[role_definitions]]
role_id = "dm"
bundle_id = "core"
display_name = "Dungeon Master"
description = "Runs a prepared region through scoped authored acts and reads its hidden and internal rows."
capability_ids = ["frontier.read_hidden", "frontier.read_internal", "frontier.spawn_entity_definition", "frontier.despawn", "frontier.set_state", "frontier.transfer", "frontier.afflict", "frontier.inscribe", "frontier.open_session", "frontier.impersonate"]

[[role_definitions]]
role_id = "adventure-author"
bundle_id = "core"
display_name = "Adventure Author"
description = "Authors adventure content and assigns the DM role inside a realized binding extent."
capability_ids = ["frontier.read_hidden", "frontier.read_internal", "frontier.apply_manifest", "frontier.prune", "frontier.bind.dm"]

[[role_definitions]]
role_id = "system-author"
bundle_id = "core"
display_name = "System Author"
description = "Authors system content inside a realized binding extent."
capability_ids = ["frontier.read_hidden", "frontier.read_internal", "frontier.apply_manifest", "frontier.prune"]

[[role_definitions]]
role_id = "npc-host"
bundle_id = "core"
display_name = "NPC Host"
description = "Assigns and releases durable Service drivers to unoffered NPC bodies within the host's realized binding extent."
capability_ids = ["frontier.drive"]

"#;

pub struct CoreExtentKindSeed {
    pub kind_id: &'static str,
    pub resolution_shape: &'static str,
    pub description: &'static str,
}

pub const CORE_EXTENT_KINDS: &[CoreExtentKindSeed] = &[
    CoreExtentKindSeed {
        kind_id: "bundle",
        resolution_shape: "Bundle",
        description: "authored entity membership only; excludes graphs, players and runtime spawns",
    },
    CoreExtentKindSeed {
        kind_id: "place_graph",
        resolution_shape: "PlaceGraph",
        description: "place_graph_definition extent, including child combat grids",
    },
    // Membership tag equals the raw extent_ref: any entity carrying that exact
    // tag IS a member. Choose party refs from a namespace nothing else uses
    // (e.g. srd.party.*). sk-ntz.7 retired the member_of:<party> prefix.
    CoreExtentKindSeed {
        kind_id: "party",
        resolution_shape: "Party",
        description: "party membership-tag extent (extent_ref is the entity tag) plus current encounter union; any entity carrying that exact tag is a member — choose party refs from a namespace nothing else uses (e.g. srd.party.*)",
    },
];
