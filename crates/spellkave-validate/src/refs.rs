//! Static cross-row reference lint for the manifest authoring surface.
//!
//! Reference checks for authored manifest rows (including these examples):
//!
//! 1. `roll_table_entry.result_ref` — resolved against
//!    `outcome_template.outcome_template_id` when the parent
//!    `roll_table.result_kind == "outcome_graph"`, or against `i64` parse
//!    when `"int"`. Other `result_kind` values are reserved namespaces and
//!    skipped (so the lint never false-positives on a future kind).
//!    → `MF-VAL-210`, **Fatal**.
//!
//! 2. DSL `roll_table('<id>')` calls (`Op::RollTable`) inside any predicate
//!    or outcome DSL source — resolved against `roll_table.roll_table_id`.
//!    → `MF-VAL-211`, **Fatal**.
//!
//! 3. DSL string-literal RHS of a comparison whose LHS is
//!    `$event.detail.template_id` — resolved against the union of authored
//!    `affordance_template.template_id`, `action_definition.action_id`,
//!    `outcome_template.outcome_template_id`. Many production fixtures
//!    compare against *synthesized* Layer-2 event payload strings that are
//!    never authored as rows (e.g. `'core/ability_check'`,
//!    `'srd/attack_roll'`); to avoid drowning them in false positives the
//!    class emits **Warning**, not Fatal. Warnings are surfaced by
//!    `manifest_validation_warnings()` and printed by `just seed`, but do
//!    not reject admission.
//!    → `MF-VAL-212`, **Warning**.
//!
//! 4. `condition_hook.expression_source` / `detail_filter_source` —
//!    parse-check already lives in `validate_condition_authoring`. This
//!    class walks the *grafted* AST for Classes 2 + 3 references that the
//!    base predicate walk would otherwise miss.
//!    → `MF-VAL-213`, **Fatal** for `roll_table()` refs, **Warning** for
//!    template_id compares (same rationale as Class 3).
//!
//! 5. Cross-bundle scoping — every resolution above must respect the
//!    owning row's `bundle_id` and the transitive `depends_on` closure of
//!    the bundles it declares. A reference that resolves only in a bundle
//!    outside that closure fails.
//!    → `MF-VAL-214`, **Fatal**.
//!
//! 6. `consequence_frame_write.frame_write` JSON — externally-tagged
//!    `FrameWriteOp` whose `ModifyResource.resource` is a free-form string
//!    at the kernel level. Walk the JSON, extract `resource`, resolve it
//!    against `resource_definitions.resource_definition_id`, fall through
//!    to Class 5 bundle-scope on hit. Added by Direction H (Phase 14.b)
//!    after Acid Splash demonstrated that authoring `resource = "hp"`
//!    with no `[[resource_definitions]]` row admits silently.
//!    → `MF-VAL-217`, **Fatal**.
//!
//! 7. `condition_definition.implies` — each element must resolve to an
//!    authored `condition_definition.condition_definition_id` (in the
//!    same bundle or a transitive dependency, per Class 5 scope). The
//!    `implies` field landed in Direction R (Phase 24 of the SRD probe)
//!    as the cascade-composition primitive (Paralyzed → Incapacitated);
//!    without this lint a typo like `implies = ["core/condition/incapcitated"]`
//!    passes static validation cleanly and only fails at apply time
//!    inside `apply_implied_cascade`. `implied_by_count('<id>')` predicate
//!    operands resolve through the same condition-definition closure.
//!    Sibling of Class 5 silent-pass holes called out by issue #259-family.
//!    → `MF-VAL-226`, **Fatal**.

pub mod condition_extents;
pub(crate) mod party_extents;
pub mod place_seals;
pub mod privileged_namespace;
pub mod privileged_templates;
pub mod role_bindings;
pub mod roles;

use std::collections::{BTreeMap, BTreeSet};

use spellkave_dsl::{
    compile_predicate_with_macros, DslMacroDefinition, Expression, Op, SubjectRef, ValueType,
};
use spellkave_schema::{EntityKindRow, Manifest};

use crate::containment_vocabulary::Vocabulary;
use crate::reference_scope::{resolve_scoped_ref, BundleScope, RefResolution};
use crate::{Diagnostic, Severity, MF_VAL_FK_CLOSURE};

pub const MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF: &str = "MF-VAL-210";
/// An int `roll_table` whose entries are exactly the identity map 1..N each
/// with `weight = 1` is a smuggled die (sk-fr4 / sk-s1e / #2184) — unless the
/// table carries an intentional `die` tag (the substrate `core/d20` shape still
/// consumed via `roll_table('core/d20')`). Untagged identity tables (the retired
/// damage-profile class) must author a dice expression (`1dN` / `d20`) or an
/// `entity_definition_item_dsl_source` slot (sk-84i.2 `.source`) instead.
/// → **Fatal**.
pub const MF_VAL_ROLL_TABLE_DICE_SMUGGLING: &str = "MF-VAL-317";
/// Tag that marks an intentional named-die roll_table (exempt from MF-VAL-317).
const ROLL_TABLE_DIE_TAG: &str = "die";
pub const MF_VAL_DSL_ROLL_TABLE_REF: &str = "MF-VAL-211";
pub const MF_VAL_DSL_TEMPLATE_ID_LITERAL: &str = "MF-VAL-212";
/// An item DSL-source formula or `ModifyResource`/`ModifyChannel`
/// `delta_source` reads `stat`/`effective`/`resource_max`/`resource_min`
/// for a field id that is not a kernel-projected fact and does not resolve
/// to an authored `resource_definitions` or `object_state_definitions` row
/// (sk-84i.7). Unknown macros in the same formulas already fail parse
/// (`MF-VAL-318` / `MF-DSL-MACRO-UNKNOWN`). → **Fatal** when the owning
/// bundle's `depends_on` closure is fully present in the manifest under
/// validation or on the server's live-admission path; **Info** (trusted,
/// with a hint) when the closure is not fully visible offline — the same
/// sk-zqc CR-12 two-tier trust `MF-VAL-217` applies, so a per-file
/// `just validate`, a verb-runner slice, or a doc-evidence snippet that
/// reads `srd/str` declared in another file does not fail closed.
pub const MF_VAL_DSL_FORMULA_STAT_REF: &str = "MF-VAL-329";
/// An exact equipment relation must name an authored slot (ADR-0126).
pub const MF_VAL_DSL_EQUIPMENT_SLOT_REF: &str = "MF-VAL-363";
pub const MF_VAL_HOOK_GRAFTED_REF: &str = "MF-VAL-213";
pub const MF_VAL_CROSS_BUNDLE_SCOPE: &str = "MF-VAL-214";
pub const MF_VAL_ITEM_DEF_SLOT_REF: &str = "MF-VAL-215";
pub const MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF: &str = "MF-VAL-216";
pub const MF_VAL_EQUIP_SLOT_DUP: &str = "MF-VAL-219";
/// `actor_equipment_slots.slot_id` must name an authored equipment slot.
pub const MF_VAL_ACTOR_EQUIP_SLOT_DEF_REF: &str = "MF-VAL-319";
/// sk-ntz.4 (child of sk-9ll.3/sk-84i.3 follow-up epic sk-ntz): instance-level
/// sibling of [`MF_VAL_DEF_SEED_EQUIPMENT_SLOT_COMPATIBILITY`] (MF-VAL-315).
/// An `[[actor_equipment_slots]]` row whose `item_key` resolves to an
/// `[[entities]]` row whose `entity_def_id`'s `entity_definitions.
/// slot_compatibility_predicate` rejects the row's own `slot_id` would fail
/// closed at runtime `EquipItem` apply
/// (`equipment::check_slot_compatibility`) with no authoring-time signal
/// before this check — the same class of gap ADR-0028 D-7 closed for
/// definition-level equipment seeds (MF-VAL-315), now closed for the
/// instance-level authoring path MF-VAL-319 already partially covers (slot_id
/// existence, not item/slot compatibility). → **Fatal**.
pub const MF_VAL_ACTOR_EQUIP_SLOT_COMPATIBILITY: &str = "MF-VAL-326";
pub const MF_VAL_MANIP_ACTION_TAG_SHAPE: &str = "MF-VAL-220";
pub const MF_VAL_MANIP_ACTION_ACTION_REF: &str = "MF-VAL-221";
pub const MF_VAL_MANIP_ACTION_ON_SUCCESS: &str = "MF-VAL-222";
pub const MF_VAL_MANIP_ACTION_TEMPLATE_ACTION_REF: &str = "MF-VAL-223";
pub const MF_VAL_MANIP_ACTION_TAG_TYPO: &str = "MF-VAL-224";
pub const MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG: &str = "MF-VAL-227";
pub const MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR: &str = "MF-VAL-228";
pub const MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR: &str = "MF-VAL-229";
/// Class 9 — `[[entity_tag]].entity_key` MUST resolve to an authored
/// `[[entities]]` instance in the same manifest. The CR-4/CR-7 relational
/// instance-tag write leg replaces the inline `world_objects.tags` CSV
/// (predating the #1968 B-10 retirement of the `[[world_objects]]` row kind
/// itself); a tag pointed at a non-existent key would attach to nothing and
/// read forever-false — fail loudly at authoring time instead.
pub const MF_VAL_ENTITY_TAG_ENTITY_REF: &str = "MF-VAL-242";
/// Class 10 — a `[[place_edge_affordance]]` binding consumed by a
/// `knows_entity` predicate (`actor.knows_entity($bindings.x)`) MUST name an
/// authored `[[entities]]` key. `KnowsEntity` compares numeric entity ids; a
/// binding value that resolves to
/// no authored entity would validate clean, apply clean, and leave the gate
/// silently false forever — the V2 wall (CR-18). A literal numeric id passes
/// (an explicit authored choice); a non-numeric key with no authored entity is
/// rejected loudly here. Manifest apply resolves the surviving keys → numeric
/// ids via the same `Op::entity_ref_binding` classifier this finding walks.
pub const MF_VAL_AFFORDANCE_KNOWS_ENTITY_BINDING_REF: &str = "MF-VAL-244";
/// Class 7 — `modifier_template.applies_to` MUST resolve to either a seeded
/// core `modifier_target_definition.target_id` (one of the 5 legacy targets
/// `ResolutionCheck` / `Damage` / `Dc` / `Duration` / `ResolutionMode`) or
/// a manifest-authored `modifier_target_definition` row. Unknown values
/// previously slipped past every gate and silently no-op'd in the lift path
/// (Issue #314 / Direction O).
pub const MF_VAL_MODIFIER_TEMPLATE_TARGET_REF: &str = "MF-VAL-225";
/// Class 6 — `consequence_frame_write.frame_write` JSON carries
/// `ModifyResource.resource: String` whose value must resolve to an authored
/// `resource_definitions.resource_definition_id`. The runtime decodes
/// `frame_write` only inside `r_apply_consequence` (silent until execution),
/// and `ModifyResource.resource` is intentionally free-form there (Phase F
/// will type it). This lint catches the silent-drop before apply.
/// → **Fatal**.
pub const MF_VAL_FRAME_WRITE_RESOURCE_REF: &str = "MF-VAL-217";

/// Class 6 sibling (sk-ct8.4 / finding #86) — `consequence_frame_write.frame_write`
/// JSON carries `ModifyChannel.channel: String` whose value must resolve to an
/// authored `economy_channel_definitions.id`. Same silent-until-execution
/// posture as `MF_VAL_FRAME_WRITE_RESOURCE_REF` above (the runtime's own
/// fail-closed guard is `r_apply_consequence::modify_channel::prevalidate`);
/// this lint catches the modder-typo before apply. → **Fatal**.
pub const MF_VAL_FRAME_WRITE_CHANNEL_REF: &str = "MF-VAL-312";

/// `consequence_frame_writes`
/// storage key is `(template_id, ordinal)` (task 1.1) and the server replaces
/// the whole ordered list per template on every apply path — a manifest that
/// declares two blocks of one template with the same `ordinal` would abort a
/// whole apply on a primary-key collision instead of failing cleanly at
/// authoring time. Same duplicate-pair shape as `MF_VAL_EQUIP_SLOT_DUP`
/// (MF-VAL-219) below. → **Fatal**.
pub const MF_VAL_FRAME_WRITE_ORDINAL_DUP: &str = "MF-VAL-335";

/// Class 8 — `condition_definition.implies` MUST resolve, per element, to an
/// authored `condition_definition.condition_definition_id`. The runtime
/// applies implied conditions via `apply_implied_cascade` (Phase 24 / Direction
/// R landing); an unresolved id fails fast at apply time but admits the row
/// statically. Same silent-pass family as #259.
/// → **Fatal**.
pub const MF_VAL_CONDITION_IMPLIES_REF: &str = "MF-VAL-226";

/// Class 11 — definition-level instance seeds (#728 / #729 + the wired-in
/// `entity_definition_initial_condition` fossil) MUST reference existing
/// definitions, in the same bundle or a transitive dependency (Class 5 scope):
/// `entity_def_id` → `entity_definition`, `state_id` → `object_state_definition`,
/// `capability_id` → `capability_definition`, `condition_definition_id` →
/// `condition_definition`. A seed pointed at a missing definition would copy
/// nothing (or fail-fast) at instance birth; fail loudly at authoring instead.
/// → **Fatal**. Admission posture (ADR-0044): `entity_definition` /
/// `object_state_definition` / `capability_definition` round-trip losslessly and
/// are augmented from live tables (extend D-1), so those refs resolve
/// in-validator. `MF_VAL_DEF_SEED_CONDITION_REF` cannot be augmented (the
/// condition row's `gate_predicate` is a parsed `DslExpression`) and is deferred
/// to the merge-gate at admission, mirroring MF-VAL-226 (extend D-3).
pub const MF_VAL_DEF_SEED_ENTITY_REF: &str = "MF-VAL-252";
pub const MF_VAL_DEF_SEED_STATE_REF: &str = "MF-VAL-253";
pub const MF_VAL_DEF_SEED_CAPABILITY_REF: &str = "MF-VAL-254";
pub const MF_VAL_DEF_SEED_CONDITION_REF: &str = "MF-VAL-255";
/// sk-84i.3 (parent sk-84i) — `entity_definition_initial_containment.
/// contained_entity_def_id` must reference an existing `entity_definition`.
pub const MF_VAL_DEF_SEED_CONTAINMENT_REF: &str = "MF-VAL-358";
/// sk-84i.3 — `entity_definition_initial_equipment.equipped_entity_def_id` must
/// reference an existing `entity_definition`.
pub const MF_VAL_DEF_SEED_EQUIPMENT_ITEM_REF: &str = "MF-VAL-359";
/// sk-84i.3 — `entity_definition_initial_equipment.slot_id` must reference
/// an existing `equipment_slot_definition`.
pub const MF_VAL_DEF_SEED_EQUIPMENT_SLOT_REF: &str = "MF-VAL-360";
/// sk-84i.3 — at most one `entity_definition_initial_equipment` row per
/// `(entity_def_id, slot_id)`.
pub const MF_VAL_DEF_SEED_EQUIPMENT_SLOT_DUP: &str = "MF-VAL-313";
/// sk-84i.3 — the def-level containment/equipment graph (the UNION of
/// `entity_definition_initial_containment`'s `entity_def_id` ->
/// `contained_entity_def_id` edges AND `entity_definition_initial_equipment`'s
/// `entity_def_id` -> `equipped_entity_def_id` edges) must be acyclic —
/// `apply_definition_seeds` recurses through both seed tables via the same
/// `spawn_and_seed_child` helper (canon-warden finding D-4, PR #2154).
pub const MF_VAL_DEF_SEED_CONTAINMENT_CYCLE: &str = "MF-VAL-314";
/// sk-84i.3 (canon-warden finding A-5, PR #2154) — a definition-level
/// equipment seed's `equipped_entity_def_id` definition's
/// `slot_compatibility_predicate` must admit the seed's `slot_id`, the same
/// invariant the canonical `r_apply_consequence` equip path enforces
/// (`equipment::check_slot_compatibility`).
pub const MF_VAL_DEF_SEED_EQUIPMENT_SLOT_COMPATIBILITY: &str = "MF-VAL-315";

/// sk-ct8.3 / finding #37 (BL-19), canon-warden fix #3 — validator parity
/// for the two row kinds the wear/equip-gating slice added.
/// `entity_definition_equip_modifier.modifier_template_id` MUST reference an
/// authored `modifier_template.modifier_template_id` (its own `entity_def_id`
/// reuses `MF_VAL_DEF_SEED_ENTITY_REF`, the same Class 11 shape its
/// `entity_definition_initial_capability`/`_condition` siblings already use).
/// An unresolved id would silently drop out of
/// `lift_modifier_templates_to_rows` (the same modder-extensible "skip
/// unresolvable, fail loud at authoring instead" posture MF-VAL-225 already
/// takes for `modifier_template.applies_to`) — fail loudly here instead.
/// → **Fatal**.
pub const MF_VAL_DEF_SEED_MODIFIER_TEMPLATE_REF: &str = "MF-VAL-310";
/// `entity_definition.on_equip_consequence_id` / `on_unequip_consequence_id`
/// (when non-empty) MUST reference an authored
/// `consequence_template.template_id` — same shape as
/// `MF_VAL_DEF_SEED_CAPABILITY_REF`/`_CONDITION_REF`, applied to the
/// pre-existing `on_consume_consequence_id`/`on_destroy_consequence_id`
/// column pair’s sibling hooks. Unresolved ids fail at authoring time. → **Fatal**.
pub const MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF: &str = "MF-VAL-311";

/// ADR-0045 COND-4 — `frame_write` carries executable DSL: `ModifyResource.
/// delta_source` (an Int expression) and a non-empty `ForEachActorInRadius.
/// predicate_source` (a Bool predicate). Both are parsed + type-checked at
/// manifest admission so a malformed expression is a static rejection, not a
/// runtime-only `Err` (the §17.2 DSL surface must not be smuggled past the
/// single-source validator — ADR-0023/0025). A parse failure is this code.
/// → **Fatal**.
pub const MF_VAL_FRAME_WRITE_DSL_PARSE: &str = "MF-VAL-256";

/// ADR-0045 COND-4 — the `frame_write` DSL expression parsed but its root
/// value-type is wrong (a Bool where an Int `delta_source` is required, or an
/// Int where a Bool `predicate_source` is required). → **Fatal**.
pub const MF_VAL_FRAME_WRITE_DSL_TYPE: &str = "MF-VAL-257";
pub const MF_VAL_OUTCOME_GRAPH_NODE_REF: &str = "MF-VAL-258";
pub const MF_VAL_OUTCOME_GRAPH_ROOT_REF: &str = "MF-VAL-259";
pub const MF_VAL_OUTCOME_GRAPH_EDGE_REF: &str = "MF-VAL-260";
pub const MF_VAL_OUTCOME_GRAPH_GUARD_PARSE: &str = "MF-VAL-261";
pub const MF_VAL_OUTCOME_GRAPH_BOUNDS: &str = "MF-VAL-262";
/// A self-eligible action that is ALSO wired as a reaction (`reaction_eligibility`
/// and/or `reaction_trigger` set — ADR-0050 self-reaction-resolution-flip) must
/// declare a bounded budget: either the canonical `spends_channel` gate or a
/// legacy `ModifyResource` consequence. Plain self-eligible actions that are not
/// reactions (e.g. a genuinely free self-targeted action, or an ordinary
/// manipulate action's self-target gate) are out of scope — see
/// `validate_self_reaction_resource_refs`.
pub const MF_VAL_SELF_REACTION_RESOURCE_REF: &str = "MF-VAL-263";

/// Convention (#835) — a `srd/spell/<slug>` `capability_definition` should have a
/// matching `srd/spell/<slug>` `entity_definition` (its catalog identity), so a spell
/// is ONE identity: behavior `capability_id` == catalog `entity_def_id`. Surfaces
/// probe-era capabilities that have not adopted the catalog slug as drift. → **Warning**
/// (routing variants and synthetic consumer capabilities may legitimately have no
/// catalog entity, so this never rejects admission).
pub const MF_VAL_SPELL_CAPABILITY_CATALOG_CONVENTION: &str = "MF-VAL-264";

/// Class 12 — `action_definition.required_capability` MUST resolve, when non-empty, to
/// an authored `capability_definition.capability_id` (Class 5 bundle scope). Before #835
/// the field was an unchecked free string (`refs.rs` special-cased only
/// `core/communicate`), so a typo'd or orphaned `required_capability` admitted
/// statically and failed only at selection time — the same silent-pass family as #259.
/// → **Fatal**.
pub const MF_VAL_ACTION_REQUIRED_CAPABILITY_REF: &str = "MF-VAL-265";

/// Each id in `condition_definition.grants_capability_while_active` /
/// `.suppresses_capability_while_active` MUST resolve to an authored
/// `capability_definition.capability_id` (Class 5 bundle scope) — the same
/// discipline `action_definition.required_capability` (MF-VAL-265) and
/// `definition_seed`'s capability grants (MF-VAL-289) already carry.
///
/// Without it a typo'd id validates and seeds cleanly, then silently never
/// matches at fold time in `capability_ids_for_actor` — "validates clean, does
/// nothing", which is the exact failure class bead sk-syo exists to kill (the
/// grant field itself shipped dead for that reason). Caught here rather than
/// left to a runtime that has nothing to report: a suppression that matches
/// nothing is indistinguishable from an actor who simply never had the
/// capability. → **Fatal**.
pub const MF_VAL_CONDITION_CAPABILITY_REF: &str = "MF-VAL-316";

/// `capability_definition.capability_id` MUST be unique across the manifest. The id
/// is a global primary key (the runtime `capability_definition` table rejects a
/// duplicate with **MF-APPLY-020** at apply time), but until now the static validator
/// missed it: `RefIndex` builds a `BTreeMap` keyed on `capability_id`, so a duplicate
/// row silently OVERWRITES the prior one and disappears from every ref check — and a
/// single fragment never duplicates, so the gap only bites on a CROSS-FRAGMENT collision
/// surfaced when `--bundle` merges the include closure (e.g. re-emitting a capability that
/// already exists in another fragment). This rule is the static mirror of MF-APPLY-020 —
/// it catches the same duplicate at validate time instead of at `just seed`. → **Fatal**.
pub const MF_VAL_CAPABILITY_DEFINITION_DUP: &str = "MF-VAL-266";
/// Class 10b — `[[place_edge_affordance]].bindings_json` MUST cover every
/// binding name consumed by its referenced affordance template predicate, both
/// scalar `$bindings.<name>` refs and structured `bindings.<name>.stat(...)`
/// subject reads. Missing coverage would otherwise under-default or fail only at
/// apply time instead of at authoring time. → **Fatal**.
pub const MF_VAL_AFFORDANCE_BINDING_COVERAGE: &str = "MF-VAL-267";
/// Class 10b sibling (ADR-0101 D-1/D-5): the same binding-reference-closure
/// check as [`MF_VAL_AFFORDANCE_BINDING_COVERAGE`], run over each
/// Checks a movement default’s bindings against its affordance predicate.
/// Edge-specific rows can refine the default. → **Fatal**.
pub const MF_VAL_MOVEMENT_DEFAULT_BINDING_COVERAGE: &str = "MF-VAL-322";
/// ADR-0101 D-1/D-5: two `place_edge_affordance` (or two `movement_defaults`)
/// rows sharing one `(edge_key, mode_id)` scope (or one
/// `(bundle_id, place_graph_id, mode_id)` scope) binding the SAME key —
/// authored-rows-only sibling of the commit-time in-mode conflict check.
/// A refine (an edge row naming the SAME template as an in-scope default) is
/// not a conflict — the edge's value simply overlays the default's; this
/// check only fires on TWO DIFFERENT templates sharing a key within one
/// mode, which the merge step cannot resolve unambiguously. → **Fatal**.
pub const MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT: &str = "MF-VAL-323";
/// ADR-0101 D-1: the same `affordance_template_id` attached to one edge
/// under two DIFFERENT `mode_id`s — legal (world-manifest-authoring spec,
/// "Re-authoring the same template in another mode accumulates": both rows
/// exist as two distinct modes) but very likely an authoring slip (a
/// mistyped `mode_id` where a refine was intended). → **Warning**.
pub const MF_VAL_MOVEMENT_MODE_TEMPLATE_REPEATED: &str = "MF-VAL-324";
/// A place-provided niche used by a manipulate affordance has no
/// `spatial_niche_attachments` row in the assembled Manifest (sk-2wg /
/// sk-84i.1). `r_manipulate` then fails closed at `attachment_presence`
/// before resolution — the former runtime-only miss. **Warning, not Fatal**
/// — a reusable bundle (srd alone) legitimately ships niche-gated actions
/// expecting a downstream scene to author the attachment; per-bundle Fatal
/// over-rejects. Combined-bundle-scope posture matches MF-VAL-273: this
/// lint sees whatever Manifest it is handed. Definitions-only manifests
/// (no `place_nodes`) are skipped. Actor-/entity-provided niches and
/// traversal templates (movement_descriptor / place_edge_affordance /
/// movement_defaults) do not need a spatial attachment.
/// → **Warning**.
pub const MF_VAL_PLACE_NICHE_ATTACHMENT: &str = "MF-VAL-328";
/// Class 13 — an `srd.weapon`-tagged `entity_definition` SHOULD carry an
/// `entity_definition_item_dsl_source` row with slot `srd/damage` (#1258,
/// repointed sk-hml / #2184 after sk-84i.2 retired the roll-table
/// `srd/damage_profile` pointer). Attack damage is attacker-sourced via
/// `actor.wielded_item.source('srd/damage')` gated on
/// `actor.wielded_item.has_tag('srd.weapon')`; a weapon-tagged def without the
/// slot materialises the melee/ranged affordance but fails closed at act time
/// with `MissingItemDslSource` (ADR-0035 / ADR-0045 §D-5). Same
/// perceived-but-unactionable footgun class as the original #1258 glass_staff
/// incident, now on the live convention.
///
/// **Warning, not Fatal** — co-occurrence convention (sibling of MF-VAL-264),
/// not a dangling ref. Decorative weapon props may never be wielded into an
/// attack. srd-scoped via bundle_scope. Satisfied only by a def-level
/// `entity_definition_item_dsl_source` row (there is no instance-level item
/// DSL-source table). → **Warning**.
pub const MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION: &str = "MF-VAL-268";

/// A seeded state value (`entity_definition_initial_state` or instance
/// `object_state`) whose `state_id` names a `resource_definition` but whose
/// `value_int` falls outside that resource's `[min_value, max_value]`. The
/// runtime SILENTLY CLAMPS it at commit, so the authored value is NOT the value
/// that lands — a surprise the author never sees. Catalog row **C6**. Ratcheted
/// Warning → **Fatal** (#1359) once the corpus swept clean (0 offenders) at
/// promotion time.
pub const MF_VAL_SEED_RESOURCE_BOUNDS: &str = "MF-VAL-269";

/// Two seed rows write the same `(entity, state)` — a duplicate
/// `entity_definition_initial_state` `(entity_def_id, state_id)` or instance
/// `object_state` `(entity_key, state_id)`. At commit the later row SILENTLY
/// last-write-wins (the Bat HP `5` clobbered to `30` in `srd/coverage/`), so the
/// earlier seed is lost with no diagnostic. Distinct from MF-VAL-266 (capability
/// PK) and MF-VAL-269 (bounds). Catalog row **R6**. Ratcheted Warning → **Fatal**
/// (#1359) once the corpus swept clean (0 offenders) at promotion time.
pub const MF_VAL_DUPLICATE_STATE_SEED: &str = "MF-VAL-270";

/// A `bundle_id` that is not snake_case (snake segments joined by `_`, with `/`
/// allowed as a namespace separator: `tag_authoring/homebrew`). Flags hyphens and
/// uppercase. The convention keeps one charset across the whole id space, since
/// `bundle_id` is the root segment of every `<bundle_id>/...` row id, and aligns
/// with `file-stem == bundle_id == metadata.profile`. Catalog row **C2**. →
/// **Warning** (a convention, not a correctness gate — a third-party bundle may differ).
pub const MF_VAL_BUNDLE_ID_NAMING: &str = "MF-VAL-271";

/// A generator driver and a hand-authored sibling (or two generator passes)
/// can independently emit the SAME id for a definitional row — each fragment
/// is file-clean on its own, and the collision is invisible until `--bundle
/// --dep` merges the whole closure into one `Manifest`: `RefIndex`'s
/// `BTreeMap`s silently keep only the last-inserted row, so every OTHER lint
/// in this file never sees the collision either. The runtime rejects the
/// duplicate identity when the bundle is applied.
/// MF-VAL-266 already catches this for `capability_definitions.capability_id`
/// (catalog row R2); this rule generalises the same raw-`Vec` duplicate scan
/// to every other globally-unique definitional id column authors actually
/// generate content for. Catalog row **R3**. → **Fatal**.
pub const MF_VAL_DUPLICATE_ROW_ID: &str = "MF-VAL-272";

/// Warns when a row defines an id outside its own bundle namespace.
/// A local id equals the bundle id or starts with that id followed by `/`.
pub const MF_VAL_ID_BUNDLE_NAMESPACE: &str = "MF-VAL-291";

/// Live SRD identity tag the melee/ranged attack archetypes gate on
/// (`actor.wielded_item.has_tag('srd.weapon')`) — content convention, not
/// kernel vocabulary. Exact match only (not `srd.weapon.property.*` etc.).
const WEAPON_TAG: &str = "srd.weapon";
/// Item DSL-source slot every weapon-like item's damage magnitude ships under
/// (`actor.wielded_item.source('srd/damage')`, sk-84i.2).
const DAMAGE_SLOT: &str = "srd/damage";

/// Existence-only live index (id → bundle_id) for definition kinds
/// that do not require full-row reconstruction:
/// wholesale `economy_channel_definitions`/`modifier_template` augmentation
/// would splice live rows into `manifest.economy_channel_definitions` /
/// `.modifier_templates` and re-enter THIS validator's own row-level checks
/// (`resets_on`, `applies_to`, …) against a manifest that only has a PARTIAL
/// view of their own cross-refs — confirmed empirically for `modifier_template`
/// by commit a290afbeb. Only the bare `(id, bundle_id)` PAIR is safe to
/// trust: enough to resolve a `ModifyChannel.channel` /
/// `entity_definition_equip_modifier.modifier_template_id` reference, never
/// enough to re-validate the referenced row's own content.
#[derive(Default, Clone)]
pub struct LiveVocabulary {
    pub containment: Vocabulary,
    /// Existence and dependency facts for authored selections; no live DSL is reconstructed.
    pub dm: DmVocabulary,
    /// Authored extent vocabulary uses existing kernel resolution shapes.
    pub extent_shapes: BTreeMap<String, String>,
    /// Authoritative live roles; retained separately to detect foreign takeover.
    pub roles: BTreeMap<String, spellkave_schema::RoleDefinitionRow>,
    pub role_aggregations: Vec<spellkave_schema::RoleAggregationRow>,
    /// Authored actor key → (owning bundle, has the player-seat tag).
    pub role_subjects: BTreeMap<String, (String, bool)>,
    /// Definitions carrying the live player-seat tag, including definitions with no actor yet.
    pub playable_entity_definitions: BTreeSet<String>,
    pub capabilities: BTreeMap<String, String>,
    /// `economy_channel_definitions.id` → `bundle_id`, already-published.
    pub economy_channels: BTreeMap<String, String>,
    /// `modifier_template.modifier_template_id` → `bundle_id`, already-published.
    pub modifier_templates: BTreeMap<String, String>,
    /// `equipment_slot_definitions.slot_id` → `bundle_id`, already-published.
    /// Same existence-only shape as the two above: a bundle applied in a
    /// later `manifest apply` call may equip an actor into a slot its
    /// dependency published earlier (`srd/slot/main-hand` from `srd`, used
    /// by `elven_mini`) — without this the sk-9ll.3 `item_definitions.slot_id`
    /// / `actor_equipment_slots.slot_id` checks false-fatal on admission.
    pub equipment_slots: BTreeMap<String, String>,
    /// `roll_table.roll_table_id` → `bundle_id`, already-published.
    /// Existence-only (sk-3h5): a delta's `roll_table('<id>')` call must
    /// resolve a table its dependency already committed. Full-row
    /// reconstruction of headers/entries/tags lives on the apply path so
    /// dice-smuggling and result_ref checks see the living table, not just
    /// the delta's own (necessarily partial) entries.
    pub roll_tables: BTreeMap<String, String>,
    /// `action_definition.action_id` → `bundle_id`, already-published
    /// (sk-jay0.2.33.15.7, ADR-0142). Existence-only, same shape as the
    /// four maps above: `prompt_definitions.response_action_ids`
    /// (MF-VAL-101, `lib.rs`'s `validate_typed_manifest_rows`) otherwise
    /// false-fatals when a bundle's prompt answers point at a dependency's
    /// actions (e.g. `elven_mini`'s quest prompts naming `dm/dialog/
    /// choice/N` from `srd`) — the exact "already-published dependency"
    /// shape the other four maps exist to resolve. Deliberately not a
    /// full-row reconstruction: nothing here re-validates the referenced
    /// action's own content, only that it exists.
    pub actions: BTreeMap<String, String>,
    /// Fixed OpenSession.session_def_id → owning authored bundle.
    pub session_definitions: BTreeMap<String, String>,
    /// `mod_bundle.bundle_id`, already-published (sk-jay0.7.174.23). Widens
    /// `validate_typed_manifest_rows`' `bundles` closure (built from
    /// `manifest.bundles` only) so a delta that redeclares one `place_node`/
    /// `place_edge` without repeating its `[[bundles]]` header still
    /// resolves — the exact false-fatal `sk apply -k` hits, since it sends
    /// only the changed row. A bundle id absent from BOTH the delta and the
    /// live set stays fatal.
    pub bundles: BTreeSet<String>,
    /// `place_graph.place_graph_id` → `bundle_id`, already-published
    /// (sk-jay0.7.174.23). Same shape and rationale as `bundles` above, for
    /// `place_nodes.place_graph_id` / `place_edges.place_graph_id`. Only the
    /// bare pair is trusted — never enough to re-validate the referenced
    /// graph's own content (its `substrate_id`, in particular: full-row
    /// Uses the existing-graph index without reconstructing `PlaceGraphRow` values.
    pub place_graphs: BTreeMap<String, String>,
    /// Lossless spatial ancestry/provenance facts used only for seal admission.
    pub seal_graphs: BTreeMap<String, place_seals::SealGraph>,
    /// `affordance_template.template_id` → `bundle_id`, already-published
    /// (sk-jay0.7.174.26). Existence-only, same shape as `actions` above:
    /// `affordance_template_participants.template_id` (MF-VAL-336,
    /// `affordance_template_participants.rs`) otherwise false-fatals when a
    /// delta redeclares a participant without repeating its template.
    pub affordance_templates: BTreeMap<String, String>,
}

/// Narrow reference facts needed by DM selection admission.
#[derive(Clone, Debug, Default)]
pub struct DmVocabulary {
    pub bundle_dependencies: BTreeMap<String, BTreeSet<String>>,
    pub tags: BTreeMap<String, String>,
    pub conditions: BTreeMap<String, String>,
    pub items: BTreeMap<String, String>,
    pub party_tags: BTreeSet<String>,
    /// Canonical four-reference grant key → live owner; detect takeover before clear.
    pub condition_extent_owners: BTreeMap<String, String>,
}

impl LiveVocabulary {
    /// Existence-only index of an on-disk bundle (sk-vjh.2 `--against <path>`).
    /// Same maps the server admission path fills from live tables.
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut live = Self {
            containment: Vocabulary::from_manifest(manifest),
            dm: DmVocabulary {
                bundle_dependencies: manifest
                    .bundles
                    .iter()
                    .map(|r| (r.bundle_id.clone(), r.depends_on.iter().cloned().collect()))
                    .collect(),
                tags: manifest
                    .tag_value_definitions
                    .iter()
                    .map(|r| (r.tag.clone(), r.bundle_id.clone()))
                    .collect(),
                conditions: manifest
                    .condition_definitions
                    .iter()
                    .map(|r| (r.condition_definition_id.clone(), r.bundle_id.clone()))
                    .collect(),
                items: manifest
                    .entity_definitions
                    .iter()
                    .map(|r| (r.entity_def_id.clone(), r.bundle_id.clone()))
                    .collect(),
                party_tags: party_extents::party_extent_tags(manifest, None),
                condition_extent_owners: manifest
                    .condition_capability_extent_grants
                    .iter()
                    .map(|r| {
                        (
                            spellkave_schema::storage_key::storage_key_strs(&[
                                &r.condition_definition_id,
                                &r.capability_id,
                                &r.extent_kind_ref,
                                &r.extent_ref,
                            ]),
                            r.bundle_id.clone(),
                        )
                    })
                    .collect(),
            },
            roles: manifest
                .role_definitions
                .iter()
                .map(|r| (r.role_id.clone(), r.clone()))
                .collect(),
            role_aggregations: manifest.role_aggregations.clone(),
            role_subjects: role_bindings::authored_subjects(manifest),
            playable_entity_definitions: manifest
                .entity_definition_tag
                .iter()
                .filter(|row| row.tag == spellkave_schema::PLAYER_SEAT_TAG)
                .map(|row| row.entity_def_id.clone())
                .collect(),
            capabilities: manifest
                .capability_definitions
                .iter()
                .map(|r| (r.capability_id.clone(), r.bundle_id.clone()))
                .collect(),
            ..Self::default()
        };
        for row in &manifest.extent_kind_definitions {
            live.extent_shapes
                .insert(row.kind_id.clone(), row.resolution_shape.clone());
        }
        for row in &manifest.economy_channel_definitions {
            live.economy_channels
                .entry(row.id.clone())
                .or_insert_with(|| row.bundle_id.clone());
        }
        for row in &manifest.modifier_templates {
            live.modifier_templates
                .entry(row.modifier_template_id.clone())
                .or_insert_with(|| row.bundle_id.clone());
        }
        for row in &manifest.equipment_slot_definitions {
            live.equipment_slots
                .entry(row.slot_id.clone())
                .or_insert_with(|| row.bundle_id.clone());
        }
        for row in &manifest.roll_tables {
            live.roll_tables
                .entry(row.roll_table_id.clone())
                .or_insert_with(|| row.bundle_id.clone());
        }
        for row in &manifest.session_definitions {
            live.session_definitions
                .insert(row.session_def_id.clone(), row.bundle_id.clone());
        }
        for row in &manifest.action_definitions {
            live.actions
                .entry(row.action_id.clone())
                .or_insert_with(|| row.bundle_id.clone());
        }
        for row in &manifest.bundles {
            live.bundles.insert(row.bundle_id.clone());
        }
        for row in &manifest.place_graphs {
            live.seal_graphs.insert(
                row.place_graph_id.clone(),
                place_seals::SealGraph {
                    bundle_id: row.bundle_id.clone(),
                    parent_graph_id: row.parent_graph_id.clone(),
                    blueprint_id: row.blueprint_id.clone(),
                },
            );
            live.place_graphs
                .entry(row.place_graph_id.clone())
                .or_insert_with(|| row.bundle_id.clone());
        }
        for row in &manifest.affordance_templates {
            live.affordance_templates
                .entry(row.template_id.clone())
                .or_insert_with(|| row.bundle_id.clone());
        }
        live
    }
}

/// Run all cross-row reference checks against `manifest`.
///
/// `live` is `Some(..)` ONLY on the server's admission path
/// (`validate_manifest_with_world`) — see `resolve_frame_write_ref`'s doc
/// comment for why its presence (not its content) flips the Class-6
/// frame-write checks from sk-zqc's per-file two-tier trust to sk-pm7's
/// live-admission strictness.
pub fn validate_cross_row_refs(
    manifest: &Manifest,
    live: Option<&LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    validate_cross_row_refs_for_apply(manifest, manifest, live, diags);
}

/// Reference augmentation does not redeclare live definitions. Namespace ownership
/// is checked only for the incoming rows; reference checks use the augmented view.
pub fn validate_cross_row_refs_for_apply(
    manifest: &Manifest,
    applying_manifest: &Manifest,
    live: Option<&LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    let mut index = RefIndex::build(manifest);
    index.live_world = live.is_some();
    if let Some(live) = live {
        for (id, bundle_id) in &live.economy_channels {
            index
                .economy_channels
                .entry(id.clone())
                .or_insert_with(|| bundle_id.clone());
        }
        for (id, bundle_id) in &live.modifier_templates {
            index
                .modifier_templates
                .entry(id.clone())
                .or_insert_with(|| bundle_id.clone());
        }
        for (id, bundle_id) in &live.equipment_slots {
            index
                .equipment_slot_definitions
                .entry(id.clone())
                .or_insert_with(|| bundle_id.clone());
        }
        for (id, bundle_id) in &live.roll_tables {
            index
                .roll_tables
                .entry(id.clone())
                .or_insert_with(|| bundle_id.clone());
        }
        for (id, bundle_id) in &live.session_definitions {
            index
                .session_definitions
                .entry(id.clone())
                .or_insert_with(|| bundle_id.clone());
        }
        for (id, bundle_id) in &live.actions {
            index
                .action_definitions
                .entry(id.clone())
                .or_insert_with(|| bundle_id.clone());
        }
    }
    let macros = crate::dsl_macros::dsl_macro_compiler_definitions(manifest);

    validate_outcome_graph_refs(manifest, &macros, diags);
    validate_roll_table_entry_result_refs(manifest, &index, diags);
    validate_roll_table_dice_smuggling(manifest, diags);
    validate_dsl_string_refs_in_manifest(manifest, &index, &macros, diags);
    validate_condition_hook_grafted_refs(manifest, &index, &macros, diags);
    validate_manipulation_action_refs(manifest, &index, diags);
    validate_equipment_refs(manifest, &index, diags);
    validate_actor_equipment_slot_compatibility(manifest, diags);
    validate_modifier_target_refs(manifest, &index, diags);
    validate_frame_write_refs(manifest, &index, &macros, live.is_some(), diags);
    validate_frame_write_ordinal_uniqueness(manifest, diags);
    validate_self_reaction_resource_refs(manifest, &index, diags);
    validate_prompt_response_action_refs(manifest, &index, diags);
    validate_condition_implies_refs(manifest, &index, diags);
    validate_condition_capability_refs(manifest, &index, diags);
    condition_extents::validate(manifest, live, diags);
    validate_entity_tag_refs(manifest, diags);
    validate_affordance_binding_coverage(manifest, &macros, diags);
    validate_movement_default_binding_coverage(manifest, &macros, diags);
    validate_movement_mode_composition(manifest, diags);
    validate_affordance_knowledge_bindings(manifest, &macros, diags);
    validate_definition_seed_refs(manifest, &index, diags);
    validate_definition_seed_equipment_slot_dup(manifest, diags);
    validate_definition_seed_equipment_slot_compatibility(manifest, diags);
    validate_definition_seed_containment_cycle(manifest, diags);
    validate_required_capability_refs(manifest, &index, diags);
    privileged_namespace::validate(manifest, &index, diags);
    roles::validate(manifest, live, &index.bundle_scope, diags);
    role_bindings::validate(manifest, live, &index.bundle_scope, diags);
    validate_weapon_damage_slot_convention(manifest, &index, diags);
    validate_seed_resource_bounds(manifest, diags);
    validate_duplicate_state_seed(manifest, diags);
    validate_bundle_id_naming(manifest, diags);
    validate_duplicate_row_ids(manifest, diags);
    validate_id_bundle_namespace(applying_manifest, diags);
    validate_place_niche_attachment_coverage(manifest, diags);
}

/// `bundle_id` must be snake_case (`^[a-z0-9]+(_[a-z0-9]+)*$`): lowercase
/// alphanumeric segments joined by single underscores. Hyphens, uppercase, and
/// empty/leading/trailing/double underscores are flagged (MF-VAL-271, Warning).
fn validate_bundle_id_naming(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    // snake_case segments joined by single underscores, with `/` allowed as a
    // namespace separator (a bundle may be namespaced like a row id, e.g.
    // `tag_authoring/homebrew`). Flags hyphens, uppercase, empty segments.
    fn is_snake_case(id: &str) -> bool {
        !id.is_empty()
            && id.split('/').all(|part| {
                !part.is_empty()
                    && part.split('_').all(|seg| {
                        !seg.is_empty()
                            && seg
                                .bytes()
                                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                    })
            })
    }
    for row in &manifest.bundles {
        if !is_snake_case(&row.bundle_id) {
            diags.push(diag_warning(
                MF_VAL_BUNDLE_ID_NAMING,
                "bundles.bundle_id",
                format!(
                    "bundle_id '{}' is not snake_case (^[a-z0-9]+(_[a-z0-9]+)*$). bundle_id is the \
                     root segment of every '<bundle_id>/...' row id; keep one charset across the id \
                     space and align file-stem == bundle_id == metadata.profile.",
                    row.bundle_id
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Class 13 — srd.weapon tag ⇒ srd/damage DSL-slot co-occurrence (#1258 / sk-hml)
// ---------------------------------------------------------------------------

/// Warn when an `srd.weapon`-tagged `entity_definition` (in srd or an
/// srd-dependent bundle) carries no `entity_definition_item_dsl_source` row
/// with slot `srd/damage`. See `MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION`.
fn validate_weapon_damage_slot_convention(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    let mut slotted_defs: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.entity_definition_item_dsl_source {
        if row.slot == DAMAGE_SLOT {
            slotted_defs.insert(row.entity_def_id.as_str());
        }
    }

    // One warning per definition, even if it carries the identity tag twice.
    let mut warned: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.entity_definition_tag {
        if row.tag != WEAPON_TAG {
            continue;
        }
        let def = row.entity_def_id.as_str();
        // srd-scope: only definitions whose bundle is in the srd closure.
        if !index.bundle_scope.in_scope(&row.bundle_id, "srd") {
            continue;
        }
        if slotted_defs.contains(def) {
            continue;
        }
        if !warned.insert(def) {
            continue;
        }
        diags.push(diag_warning(
            MF_VAL_WEAPON_DAMAGE_SLOT_CONVENTION,
            "entity_definition_tag.tag",
            format!(
                "srd.weapon-tagged entity_definition '{def}' carries no entity_definition_item_dsl_source slot '{DAMAGE_SLOT}'; attack damage reads actor.wielded_item.source('{DAMAGE_SLOT}'), so wielding it into an attack fails closed at runtime (MissingItemDslSource, ADR-0035 / ADR-0045 D-5). Author a def-level item DSL-source row for slot '{DAMAGE_SLOT}' (#1258 / sk-hml). Convention warning, not a rejection."
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Class 12 — action_definition.required_capability + srd/spell catalog binding (#835)
// ---------------------------------------------------------------------------

fn validate_required_capability_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    // (a) Fatal — a non-empty `required_capability` must resolve to an authored
    // `capability_definition` inside the owning bundle's depends_on closure. An empty
    // value is a legitimately ungated action (no capability gate) and is skipped.
    let genesis_capabilities = privileged_namespace::bootstrap_capability_ids();
    for row in &manifest.action_definitions {
        let cap = row.required_capability.trim();
        if cap.is_empty() {
            continue;
        }
        // Kernel genesis capabilities are present before any authored bundle.
        // Recognize exact shipped ids; a misspelled frontier id still fails.
        if genesis_capabilities.iter().any(|id| id == cap) {
            continue;
        }
        match index.capability_definitions.get(cap) {
            None => diags.push(diag_fatal(
                MF_VAL_ACTION_REQUIRED_CAPABILITY_REF,
                "action_definitions.required_capability",
                format!(
                    "action_definition '{}' required_capability '{}' does not resolve to any authored capability_definition",
                    row.action_id, cap
                ),
            )),
            Some(ref_bundle) => {
                if !index.bundle_scope.in_scope(&row.bundle_id, ref_bundle) {
                    diags.push(diag_fatal(
                        MF_VAL_CROSS_BUNDLE_SCOPE,
                        "action_definitions.required_capability",
                        format!(
                            "action_definition '{}' required_capability '{}' resolves to bundle '{}' but owning bundle '{}' does not depend on it",
                            row.action_id, cap, ref_bundle, row.bundle_id
                        ),
                    ));
                }
            }
        }
    }

    // (a2) Fatal — `capability_id` is a global PK; a duplicate is rejected at apply
    // time by MF-APPLY-020 but was invisible to static validation (the RefIndex map
    // dedups it). Iterate the raw Vec so a cross-fragment collision (re-emitting an
    // already-authored capability, surfaced when `--bundle` merges the include closure)
    // is caught here instead of only at `just seed`.
    let mut seen_caps: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.capability_definitions {
        let cap = row.capability_id.as_str();
        if !seen_caps.insert(cap) {
            diags.push(diag_fatal(
                MF_VAL_CAPABILITY_DEFINITION_DUP,
                "capability_definitions.capability_id",
                format!(
                    "duplicate capability_definition '{cap}' — capability_id is a global primary key (the runtime rejects it with MF-APPLY-020 at apply time). Re-emitting a capability that already exists in another fragment is the usual cause; reuse the existing definition instead of redefining it."
                ),
            ));
        }
    }

    // (b) Warning (convention #835) — a `srd/spell/<slug>` capability should bind to the
    // catalog entity of the same id, so a spell is ONE identity. A capability that has
    // not adopted the catalog slug surfaces here as drift; it is never a rejection
    // (routing variants / synthetic caps may have no catalog entity).
    for row in &manifest.capability_definitions {
        let cap = row.capability_id.as_str();
        if !cap.starts_with("srd/spell/") {
            continue;
        }
        if !index.entity_definitions.contains_key(cap) {
            diags.push(diag_warning(
                MF_VAL_SPELL_CAPABILITY_CATALOG_CONVENTION,
                "capability_definitions.capability_id",
                format!(
                    "srd/spell capability '{cap}' has no matching entity_definition with the same id; a catalog spell should bind capability_id == entity_def_id (#835). Convention warning, not a rejection."
                ),
            ));
        }
    }
}

/// Reverse-index from id → bundle for every cross-referenceable row class.
struct RefIndex {
    bundle_scope: BundleScope,
    /// Bundles whose `[[bundles]]` row is present in this manifest — the
    /// sk-zqc CR-12 closure-completeness gate input (`resolve_scoped_ref`).
    present_bundles: BTreeSet<String>,
    /// `true` only on the server's admission path
    /// (`validate_cross_row_refs` with a `LiveVocabulary`) — see
    /// `check_frame_write_ref`'s doc comment for why that keeps an
    /// unresolved ref Fatal regardless of closure shape.
    live_world: bool,
    /// roll_table_id → bundle_id
    roll_tables: BTreeMap<String, String>,
    /// outcome_template_id → bundle_id
    outcome_templates: BTreeMap<String, String>,
    /// affordance_template.template_id → bundle_id
    affordance_templates: BTreeMap<String, String>,
    /// action_definition.action_id → bundle_id
    action_definitions: BTreeMap<String, String>,
    session_definitions: BTreeMap<String, String>,
    /// equipment_slot_definition.slot_id → bundle_id
    equipment_slot_definitions: BTreeMap<String, String>,
    /// item manifest key → bundle_id. Items are authored as `[[entities]]`
    /// rows with kind="item" (the `[[items]]` row kind was retired in #1968
    /// B-10).
    items: BTreeMap<String, String>,
    /// resource_definition.resource_definition_id → bundle_id
    resource_definitions: BTreeMap<String, String>,
    /// economy_channel_definitions.id → bundle_id (sk-ct8.4 / finding #86,
    /// backs `MF_VAL_FRAME_WRITE_CHANNEL_REF`).
    economy_channels: BTreeMap<String, String>,
    /// modifier_target_definition.target_id → bundle_id. Includes the 5
    /// seeded core targets (`ResolutionCheck`, `Damage`, `Dc`, `Duration`,
    /// `ResolutionMode`) under bundle `core` — they are NOT in the manifest
    /// but are always present at apply-time (`seed_core_modifier_targets`).
    modifier_target_definitions: BTreeMap<String, String>,
    /// condition_definition.condition_definition_id → bundle_id. Backs the
    /// Class 8 `implies` lint.
    condition_definitions: BTreeMap<String, String>,
    /// entity_definition.entity_def_id → bundle_id. Backs the Class 11
    /// definition-seed lint.
    entity_definitions: BTreeMap<String, String>,
    /// object_state_definition.state_id → bundle_id. Backs Class 11.
    object_state_definitions: BTreeMap<String, String>,
    /// capability_definition.capability_id → bundle_id. Backs Class 11.
    capability_definitions: BTreeMap<String, String>,
    /// modifier_template.modifier_template_id → bundle_id. Backs
    /// `entity_definition_equip_modifier.modifier_template_id`
    /// (MF-VAL-310, sk-ct8.3 / finding #37).
    modifier_templates: BTreeMap<String, String>,
    /// consequence_template.template_id → bundle_id. Backs
    /// `entity_definition.on_equip_consequence_id` /
    /// `on_unequip_consequence_id` (MF-VAL-311, sk-ct8.3 / finding #37).
    consequence_templates: BTreeMap<String, String>,
}

/// The 5 seeded core modifier_target_definition target_ids. Validator
/// admits these bootstrap targets without an explicit manifest row.
const CORE_MODIFIER_TARGET_IDS: &[&str] = &[
    "ResolutionCheck",
    "Damage",
    "Dc",
    "Duration",
    "ResolutionMode",
];

impl RefIndex {
    fn build(manifest: &Manifest) -> Self {
        let mut roll_tables = BTreeMap::new();
        for row in &manifest.roll_tables {
            roll_tables.insert(row.roll_table_id.clone(), row.bundle_id.clone());
        }
        let mut outcome_templates = BTreeMap::new();
        for row in &manifest.outcome_templates {
            outcome_templates.insert(row.outcome_template_id.clone(), row.bundle_id.clone());
        }
        let mut affordance_templates = BTreeMap::new();
        for row in &manifest.affordance_templates {
            affordance_templates.insert(row.template_id.clone(), row.bundle_id.clone());
        }
        let mut action_definitions = BTreeMap::new();
        for row in &manifest.action_definitions {
            action_definitions.insert(row.action_id.clone(), row.bundle_id.clone());
        }
        let session_definitions = manifest
            .session_definitions
            .iter()
            .map(|row| (row.session_def_id.clone(), row.bundle_id.clone()))
            .collect();
        let mut equipment_slot_definitions = BTreeMap::new();
        for row in &manifest.equipment_slot_definitions {
            equipment_slot_definitions.insert(row.slot_id.clone(), row.bundle_id.clone());
        }
        let mut items = BTreeMap::new();
        for row in &manifest.entities {
            if row.kind == EntityKindRow::Item {
                items.insert(row.entity_key.clone(), row.bundle_id.clone());
            }
        }
        let mut resource_definitions = BTreeMap::new();
        for row in &manifest.resource_definitions {
            resource_definitions.insert(row.resource_definition_id.clone(), row.bundle_id.clone());
        }
        let mut economy_channels = BTreeMap::new();
        for row in &manifest.economy_channel_definitions {
            economy_channels.insert(row.id.clone(), row.bundle_id.clone());
        }
        let mut modifier_target_definitions = BTreeMap::new();
        for id in CORE_MODIFIER_TARGET_IDS {
            modifier_target_definitions.insert((*id).to_string(), "core".to_string());
        }
        for row in &manifest.modifier_target_definitions {
            modifier_target_definitions.insert(row.target_id.clone(), row.bundle_id.clone());
        }
        let mut condition_definitions = BTreeMap::new();
        for row in &manifest.condition_definitions {
            condition_definitions
                .insert(row.condition_definition_id.clone(), row.bundle_id.clone());
        }
        let mut entity_definitions = BTreeMap::new();
        for row in &manifest.entity_definitions {
            entity_definitions.insert(row.entity_def_id.clone(), row.bundle_id.clone());
        }
        let mut object_state_definitions = BTreeMap::new();
        for row in &manifest.object_state_definitions {
            object_state_definitions.insert(row.state_id.clone(), row.bundle_id.clone());
        }
        let mut capability_definitions = BTreeMap::new();
        for row in &manifest.capability_definitions {
            capability_definitions.insert(row.capability_id.clone(), row.bundle_id.clone());
        }
        let mut modifier_templates = BTreeMap::new();
        for row in &manifest.modifier_templates {
            modifier_templates.insert(row.modifier_template_id.clone(), row.bundle_id.clone());
        }
        let mut consequence_templates = BTreeMap::new();
        for row in &manifest.consequence_templates {
            consequence_templates.insert(row.template_id.clone(), row.bundle_id.clone());
        }
        Self {
            bundle_scope: BundleScope::build(manifest),
            present_bundles: manifest
                .bundles
                .iter()
                .map(|b| b.bundle_id.clone())
                .collect(),
            live_world: false,
            roll_tables,
            outcome_templates,
            affordance_templates,
            action_definitions,
            session_definitions,
            equipment_slot_definitions,
            items,
            resource_definitions,
            economy_channels,
            modifier_target_definitions,
            condition_definitions,
            entity_definitions,
            object_state_definitions,
            capability_definitions,
            modifier_templates,
            consequence_templates,
        }
    }

    /// Union of template_id-shaped surfaces, returning the authoring bundle
    /// when resolved. Used by Class 3 / Class 4.
    fn resolve_template_id(&self, id: &str) -> Option<&str> {
        self.affordance_templates
            .get(id)
            .or_else(|| self.action_definitions.get(id))
            .or_else(|| self.outcome_templates.get(id))
            .map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Class 1 — roll_table_entry.result_ref
// ---------------------------------------------------------------------------

fn validate_roll_table_entry_result_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    let by_id: BTreeMap<&str, &spellkave_schema::RollTableRow> = manifest
        .roll_tables
        .iter()
        .map(|r| (r.roll_table_id.as_str(), r))
        .collect();

    for entry in &manifest.roll_table_entries {
        let Some(parent) = by_id.get(entry.roll_table_id.as_str()) else {
            // FK closure (MF-VAL-101 territory) — left to the existing
            // typed-row validator if/when it grows the check.
            continue;
        };
        match parent.result_kind.as_str() {
            "int" => {
                if entry.result_ref.parse::<i64>().is_err() {
                    diags.push(diag_fatal(
                        MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF,
                        "roll_table_entries.result_ref",
                        format!(
                            "roll_table_entry '{}' parent roll_table '{}' has result_kind='int' but result_ref '{}' is not parseable as i64",
                            entry.roll_table_entry_id, entry.roll_table_id, entry.result_ref
                        ),
                    ));
                }
            }
            "outcome_graph" => match index.outcome_templates.get(entry.result_ref.as_str()) {
                None => diags.push(diag_fatal(
                    MF_VAL_ROLL_TABLE_ENTRY_RESULT_REF,
                    "roll_table_entries.result_ref",
                    format!(
                        "roll_table_entry '{}' result_ref '{}' does not resolve to any \
                             outcome_template (parent roll_table '{}' result_kind='outcome_graph')",
                        entry.roll_table_entry_id, entry.result_ref, entry.roll_table_id
                    ),
                )),
                Some(ref_bundle) => {
                    if !index
                        .bundle_scope
                        .in_scope(&parent.bundle_id, ref_bundle.as_str())
                    {
                        diags.push(diag_fatal(
                                MF_VAL_CROSS_BUNDLE_SCOPE,
                                "roll_table_entries.result_ref",
                                format!(
                                    "roll_table_entry '{}' references outcome_template '{}' in bundle '{}', \
                                     but parent roll_table '{}' belongs to bundle '{}' which does not depend on it",
                                    entry.roll_table_entry_id,
                                    entry.result_ref,
                                    ref_bundle,
                                    entry.roll_table_id,
                                    parent.bundle_id
                                ),
                            ));
                    }
                }
            },
            // Reserved namespaces for future result_kinds. Do not flag —
            // F's brief: never false-positive on a future kind.
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Dice-smuggling — identity 1..N weight-1 int roll_tables (sk-s1e / #2184)
// ---------------------------------------------------------------------------

/// Reject an int `roll_table` whose entries are exactly result_ref `1..N`
/// (N = entry count) each with `weight = 1`. That shape is a die smuggled
/// through the roll-table mechanism (the retired damage-profile idiom).
fn validate_roll_table_dice_smuggling(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    // Intentional named-die tables (core/d20) carry tag `die`.
    let mut die_tagged: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.roll_table_tag {
        if row.tag == ROLL_TABLE_DIE_TAG {
            die_tagged.insert(row.roll_table_id.as_str());
        }
    }

    // table_id → [(result_ref parse, weight)]
    let mut by_table: BTreeMap<&str, Vec<(i64, u32)>> = BTreeMap::new();
    for entry in &manifest.roll_table_entries {
        let Ok(value) = entry.result_ref.parse::<i64>() else {
            // Non-int result_ref on an int table is already MF-VAL-210; skip.
            continue;
        };
        by_table
            .entry(entry.roll_table_id.as_str())
            .or_default()
            .push((value, entry.weight));
    }

    for table in &manifest.roll_tables {
        if table.result_kind != "int" {
            continue;
        }
        if die_tagged.contains(table.roll_table_id.as_str()) {
            continue;
        }
        let Some(entries) = by_table.get(table.roll_table_id.as_str()) else {
            continue;
        };
        if entries.is_empty() {
            continue;
        }
        // All weights must be exactly 1.
        if entries.iter().any(|(_, w)| *w != 1) {
            continue;
        }
        let mut values: Vec<i64> = entries.iter().map(|(v, _)| *v).collect();
        values.sort_unstable();
        // Duplicates break the identity map.
        if values.windows(2).any(|w| w[0] == w[1]) {
            continue;
        }
        let n = values.len() as i64;
        // A die has at least 2 faces. A single-entry {result_ref "1",
        // weight 1} table is a constant lookup (the archived flat-1 damage
        // profiles have exactly this shape), not a smuggled '1d1'.
        if n < 2 {
            continue;
        }
        // Exactly 1..=N in some order.
        let is_identity = values.iter().enumerate().all(|(i, v)| *v == (i as i64) + 1);
        if !is_identity {
            continue;
        }
        diags.push(diag_fatal(
            MF_VAL_ROLL_TABLE_DICE_SMUGGLING,
            "roll_tables.result_kind",
            format!(
                "roll_table '{}' is an identity-mapped uniform 1..{n} int table \
                 (every entry weight=1, result_ref 1..{n}) — that is a smuggled die. \
                 Author a dice expression (e.g. '1d{n}' / 'd20') or an \
                 entity_definition_item_dsl_source slot (sk-84i.2 .source) instead \
                 (sk-s1e / #2184). Intentional named-die tables for roll_table() \
                 consumers must carry tag 'die' (core/d20 pattern). Non-identity \
                 or non-uniform int tables remain legal.",
                table.roll_table_id
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// Classes 2 + 3 — DSL string refs inside every authored source field
// ---------------------------------------------------------------------------

fn validate_dsl_string_refs_in_manifest(
    manifest: &Manifest,
    index: &RefIndex,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    // Walk every authored DSL source against the owning row's bundle_id.
    for row in &manifest.modifier_templates {
        walk_dsl_source_pair(
            &row.predicate_source,
            &row.modifier_expression_source,
            &row.bundle_id,
            "modifier_template",
            &row.modifier_template_id,
            index,
            macros,
            diags,
        );
    }

    for row in &manifest.condition_definitions {
        walk_dsl_source(
            &row.gate_predicate_source,
            DslKind::Predicate,
            &row.bundle_id,
            "condition_definition.gate_predicate_source",
            &row.condition_definition_id,
            index,
            macros,
            diags,
        );
    }

    for row in &manifest.condition_remove_policy_definitions {
        walk_dsl_source(
            &row.remove_predicate_source,
            DslKind::Predicate,
            &row.bundle_id,
            "condition_remove_policy_definitions.remove_predicate_source",
            &row.policy_id,
            index,
            macros,
            diags,
        );
    }

    for row in &manifest.affordance_templates {
        walk_dsl_source(
            &row.predicate_source,
            DslKind::Predicate,
            &row.bundle_id,
            "affordance_template.predicate_source",
            &row.template_id,
            index,
            macros,
            diags,
        );
    }

    for row in &manifest.outcome_templates {
        walk_dsl_source(
            &row.outcome_expression_source,
            DslKind::Outcome,
            &row.bundle_id,
            "outcome_templates.outcome_expression_source",
            &row.outcome_template_id,
            index,
            macros,
            diags,
        );
    }

    for row in &manifest.outcome_graph_edges {
        let owning_bundle = manifest
            .outcome_graphs
            .iter()
            .find(|graph| graph.outcome_graph_id == row.outcome_graph_id)
            .map(|graph| graph.bundle_id.as_str())
            .unwrap_or("");
        walk_dsl_source(
            &row.guard_predicate_source,
            DslKind::Predicate,
            owning_bundle,
            "outcome_graph_edges.guard_predicate_source",
            &row.outcome_graph_edge_id,
            index,
            macros,
            diags,
        );
    }

    // Real session DSL owners; participants_pred and policy identifiers stay opaque.
    for row in &manifest.session_definitions {
        for (field, source) in [
            (
                "session_definitions.busy_allow_pred",
                row.busy_allow_pred.as_deref().unwrap_or(""),
            ),
            (
                "session_definitions.busy_break_pred",
                row.busy_break_pred.as_deref().unwrap_or(""),
            ),
            (
                "session_definitions.termination_pred",
                row.termination_pred.as_str(),
            ),
            (
                "session_definitions.winner_predicate",
                row.winner_predicate.as_str(),
            ),
        ] {
            walk_dsl_source(
                source,
                DslKind::Predicate,
                &row.bundle_id,
                field,
                &row.session_def_id,
                index,
                macros,
                diags,
            );
        }
    }

    for row in &manifest.entity_definition_item_dsl_source {
        walk_dsl_source(
            &row.source,
            DslKind::Int,
            &row.bundle_id,
            "entity_definition_item_dsl_source.source",
            &format!("{}:{}", row.entity_def_id, row.slot),
            index,
            macros,
            diags,
        );
    }
}

fn validate_outcome_graph_refs(
    manifest: &Manifest,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    let graph_ids: BTreeSet<&str> = manifest
        .outcome_graphs
        .iter()
        .map(|row| row.outcome_graph_id.as_str())
        .collect();
    let mut nodes_by_graph: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for row in &manifest.outcome_graph_nodes {
        if !graph_ids.contains(row.outcome_graph_id.as_str()) {
            diags.push(diag_fatal(
                MF_VAL_OUTCOME_GRAPH_NODE_REF,
                "outcome_graph_nodes.outcome_graph_id",
                format!(
                    "outcome_graph_node '{}' references missing outcome_graph '{}'",
                    row.outcome_graph_node_id, row.outcome_graph_id
                ),
            ));
        }
        nodes_by_graph
            .entry(row.outcome_graph_id.as_str())
            .or_default()
            .insert(row.node_id.as_str());
    }

    for graph in &manifest.outcome_graphs {
        if graph.max_depth == 0 || graph.max_nodes == 0 || graph.max_draws == 0 {
            diags.push(diag_fatal(
                MF_VAL_OUTCOME_GRAPH_BOUNDS,
                "outcome_graphs",
                format!(
                    "outcome_graph '{}' declares zero bound(s): max_depth={}, max_nodes={}, max_draws={}",
                    graph.outcome_graph_id, graph.max_depth, graph.max_nodes, graph.max_draws
                ),
            ));
        }
        let Some(nodes) = nodes_by_graph.get(graph.outcome_graph_id.as_str()) else {
            diags.push(diag_fatal(
                MF_VAL_OUTCOME_GRAPH_ROOT_REF,
                "outcome_graphs.root_node_id",
                format!(
                    "outcome_graph '{}' root_node_id '{}' has no authored nodes",
                    graph.outcome_graph_id, graph.root_node_id
                ),
            ));
            continue;
        };
        if !nodes.contains(graph.root_node_id.as_str()) {
            diags.push(diag_fatal(
                MF_VAL_OUTCOME_GRAPH_ROOT_REF,
                "outcome_graphs.root_node_id",
                format!(
                    "outcome_graph '{}' root_node_id '{}' is not authored in the graph",
                    graph.outcome_graph_id, graph.root_node_id
                ),
            ));
        }
        if (nodes.len() as u32) > graph.max_nodes {
            diags.push(diag_fatal(
                MF_VAL_OUTCOME_GRAPH_BOUNDS,
                "outcome_graphs.max_nodes",
                format!(
                    "outcome_graph '{}' has {} node(s), exceeding max_nodes={}",
                    graph.outcome_graph_id,
                    nodes.len(),
                    graph.max_nodes
                ),
            ));
        }
    }

    let graph_by_id: BTreeMap<&str, &spellkave_schema::OutcomeGraphRow> = manifest
        .outcome_graphs
        .iter()
        .map(|row| (row.outcome_graph_id.as_str(), row))
        .collect();
    let mut edges_by_graph: BTreeMap<&str, Vec<&spellkave_schema::OutcomeGraphEdgeRow>> =
        BTreeMap::new();
    for edge in &manifest.outcome_graph_edges {
        if !graph_ids.contains(edge.outcome_graph_id.as_str()) {
            diags.push(diag_fatal(
                MF_VAL_OUTCOME_GRAPH_EDGE_REF,
                "outcome_graph_edges.outcome_graph_id",
                format!(
                    "outcome_graph_edge '{}' references missing outcome_graph '{}'",
                    edge.outcome_graph_edge_id, edge.outcome_graph_id
                ),
            ));
        }
        let graph_nodes = nodes_by_graph.get(edge.outcome_graph_id.as_str());
        for (field, node_id) in [
            ("from_node_id", edge.from_node_id.as_str()),
            ("to_node_id", edge.to_node_id.as_str()),
        ] {
            if !graph_nodes
                .map(|nodes| nodes.contains(node_id))
                .unwrap_or(false)
            {
                diags.push(diag_fatal(
                    MF_VAL_OUTCOME_GRAPH_EDGE_REF,
                    &format!("outcome_graph_edges.{field}"),
                    format!(
                        "outcome_graph_edge '{}' references missing {} '{}' in graph '{}'",
                        edge.outcome_graph_edge_id, field, node_id, edge.outcome_graph_id
                    ),
                ));
            }
        }
        let source = edge.guard_predicate_source.trim();
        if !source.is_empty() {
            match crate::dsl_macros::parse_predicate(source, macros) {
                Ok(expr) => {
                    if let Some(graph) = graph_by_id.get(edge.outcome_graph_id.as_str()) {
                        let draws = count_stochastic_ops(&expr);
                        if draws > graph.max_draws {
                            diags.push(diag_fatal(
                                MF_VAL_OUTCOME_GRAPH_BOUNDS,
                                "outcome_graphs.max_draws",
                                format!(
                                    "outcome_graph '{}' edge '{}' guard uses {} stochastic op(s), exceeding max_draws={}",
                                    graph.outcome_graph_id,
                                    edge.outcome_graph_edge_id,
                                    draws,
                                    graph.max_draws
                                ),
                            ));
                        }
                    }
                }
                Err(err) => diags.push(diag_fatal(
                    MF_VAL_OUTCOME_GRAPH_GUARD_PARSE,
                    "outcome_graph_edges.guard_predicate_source",
                    format!(
                        "outcome_graph_edge '{}' guard_predicate_source failed to parse: {err:?}",
                        edge.outcome_graph_edge_id
                    ),
                )),
            }
        }
        edges_by_graph
            .entry(edge.outcome_graph_id.as_str())
            .or_default()
            .push(edge);
    }

    for (graph_id, edges) in edges_by_graph {
        let Some(graph) = graph_by_id.get(graph_id) else {
            continue;
        };
        let depth = longest_graph_depth(graph.root_node_id.as_str(), &edges, graph.max_nodes);
        if depth > graph.max_depth {
            diags.push(diag_fatal(
                MF_VAL_OUTCOME_GRAPH_BOUNDS,
                "outcome_graphs.max_depth",
                format!(
                    "outcome_graph '{}' has reachable depth {}, exceeding max_depth={}",
                    graph.outcome_graph_id, depth, graph.max_depth
                ),
            ));
        }
    }
}

fn count_stochastic_ops(expr: &Expression) -> u32 {
    expr.nodes
        .iter()
        .filter(|node| matches!(node.op, Op::Roll(_) | Op::RollTable(_)))
        .count() as u32
}

fn longest_graph_depth(
    root_node_id: &str,
    edges: &[&spellkave_schema::OutcomeGraphEdgeRow],
    max_nodes: u32,
) -> u32 {
    let mut by_from: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in edges {
        by_from
            .entry(edge.from_node_id.as_str())
            .or_default()
            .push(edge.to_node_id.as_str());
    }
    let mut best = 0;
    let mut stack = vec![(root_node_id, 0_u32, BTreeSet::new())];
    while let Some((node_id, depth, mut seen)) = stack.pop() {
        best = best.max(depth);
        if !seen.insert(node_id) || seen.len() as u32 > max_nodes.saturating_add(1) {
            continue;
        }
        if let Some(next) = by_from.get(node_id) {
            for to in next {
                stack.push((to, depth.saturating_add(1), seen.clone()));
            }
        }
    }
    best
}

#[derive(Clone, Copy)]
enum DslKind {
    Predicate,
    Outcome,
    /// Bare int expression (`entity_definition_item_dsl_source.source`).
    Int,
}

#[allow(clippy::too_many_arguments)] // Row provenance plus the shared macro/ref contexts are all diagnostic inputs.
fn walk_dsl_source_pair(
    predicate_source: &str,
    outcome_source: &str,
    bundle_id: &str,
    row_kind: &str,
    row_id: &str,
    index: &RefIndex,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    walk_dsl_source(
        predicate_source,
        DslKind::Predicate,
        bundle_id,
        &format!("{row_kind}.predicate_source"),
        row_id,
        index,
        macros,
        diags,
    );
    walk_dsl_source(
        outcome_source,
        DslKind::Outcome,
        bundle_id,
        &format!("{row_kind}.modifier_expression_source"),
        row_id,
        index,
        macros,
        diags,
    );
}

#[allow(clippy::too_many_arguments)] // Keep each diagnostic origin explicit at this validation boundary.
fn walk_dsl_source(
    source: &str,
    kind: DslKind,
    bundle_id: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    if source.is_empty() {
        return;
    }
    let parsed = match kind {
        DslKind::Predicate => compile_predicate_with_macros(source, macros).map_err(|failure| {
            spellkave_dsl::ParseFailure {
                diagnostics: failure.diagnostics,
            }
        }),
        DslKind::Outcome => crate::dsl_macros::parse_outcome(source, macros),
        DslKind::Int => crate::dsl_macros::parse_int_expression(source, macros),
    };
    let Ok(expr) = parsed else {
        // Parse failure handled by the predicate/outcome/item-DSL parse validators.
        return;
    };
    walk_expr_refs(&expr, bundle_id, path, row_id, index, diags, false);
    if matches!(kind, DslKind::Int) {
        check_formula_stat_refs(&expr, bundle_id, path, row_id, index, diags);
    }
}

/// Walk every node in `expr`, emitting one diagnostic per dangling ref.
///
/// `is_grafted` flips the diagnostic code to MF-VAL-213 (Class 4) and
/// changes the path prefix so condition-hook walks are distinguishable
/// from base-source walks.
fn walk_expr_refs(
    expr: &Expression,
    owning_bundle: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
    is_grafted: bool,
) {
    check_equipment_slot_refs(expr, owning_bundle, path, row_id, index, diags);
    for node in &expr.nodes {
        match &node.op {
            Op::HasCapability(op) => {
                privileged_namespace::reject_bridge(&op.capability_id, path, row_id, diags)
            }
            // Class 2 — roll_table(<id>) call. Option C (G3 / #1254): the table id
            // is a node. A `TextLiteral` keeps the FULL static `MF-VAL-211` check;
            // a runtime `ReadText` table id (e.g. a dynamic table pointer on the
            // wielded item) can't be resolved statically → a Warning. The
            // validator dispatches on AST node KIND, never on a content field id
            // (no ADR-0021 content leak); the runtime form fails closed at eval
            // (ADR-0035) if the table is missing or empty.
            Op::RollTable(rt) => match expr.node(rt.table_id_node).map(|n| &n.op) {
                Some(Op::TextLiteral(lit)) => check_roll_table_ref(
                    &lit.value,
                    owning_bundle,
                    path,
                    row_id,
                    index,
                    diags,
                    is_grafted,
                ),
                _ => diags.push(diag_warning(
                    MF_VAL_DSL_ROLL_TABLE_REF,
                    path,
                    format!(
                        "{row_id}: roll_table(...) resolves its id at runtime; the static \
                         roll_table check is skipped — it fails closed at eval (ADR-0035) if \
                         the table is missing or empty"
                    ),
                )),
            },
            Op::ImpliedByCount(op) => {
                if let Some(condition_definition_id) = op.condition_definition_id.as_deref() {
                    check_condition_definition_ref(
                        condition_definition_id,
                        owning_bundle,
                        path,
                        row_id,
                        index,
                        diags,
                    );
                }
            }
            // Class 3 — string-literal RHS of `Compare` whose other side
            // is `$event.detail.template_id`. We detect it by walking
            // every Compare and checking whether one operand is a
            // ReadText with that subject + field_id, and the other is a
            // TextLiteral.
            Op::Compare(cmp) => {
                if let Some(literal) = template_id_compare_literal(expr, cmp.left, cmp.right) {
                    check_template_id_literal(
                        literal,
                        owning_bundle,
                        path,
                        row_id,
                        index,
                        diags,
                        is_grafted,
                    );
                }
            }
            _ => {}
        }
    }
}

fn check_equipment_slot_refs(
    expr: &Expression,
    owning_bundle: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    let slots: BTreeSet<_> = expr
        .nodes
        .iter()
        .flat_map(|node| node.op.equipped_item_slots())
        .collect();
    for slot_id in slots {
        match index.equipment_slot_definitions.get(slot_id) {
            None => diags.push(diag_fatal(
                MF_VAL_DSL_EQUIPMENT_SLOT_REF,
                path,
                format!("row '{row_id}' references equipment slot '{slot_id}' which does not resolve to any authored equipment_slot_definition"),
            )),
            Some(ref_bundle) if !index.bundle_scope.in_scope(owning_bundle, ref_bundle) => {
                diags.push(diag_fatal(
                    MF_VAL_CROSS_BUNDLE_SCOPE,
                    path,
                    format!("row '{row_id}' in bundle '{owning_bundle}' references equipment slot '{slot_id}' in bundle '{ref_bundle}', but does not depend on it"),
                ));
            }
            Some(_) => {}
        }
    }
}

fn check_condition_definition_ref(
    condition_definition_id: &str,
    owning_bundle: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    match index.condition_definitions.get(condition_definition_id) {
        None => diags.push(diag_fatal(
            MF_VAL_CONDITION_IMPLIES_REF,
            path,
            format!(
                "row '{row_id}' references condition_definition '{condition_definition_id}' which does not resolve to any authored condition_definition"
            ),
        )),
        Some(ref_bundle) => {
            if !index
                .bundle_scope
                .in_scope(owning_bundle, ref_bundle.as_str())
            {
                diags.push(diag_fatal(
                    MF_VAL_CROSS_BUNDLE_SCOPE,
                    path,
                    format!(
                        "row '{row_id}' in bundle '{owning_bundle}' references condition_definition '{condition_definition_id}' in bundle '{ref_bundle}', but '{owning_bundle}' does not depend on '{ref_bundle}'"
                    ),
                ));
            }
        }
    }
}

/// If this Compare is `$event.detail.template_id == 'lit'` (in either
/// operand order), return the literal string. Otherwise `None`.
fn template_id_compare_literal(expr: &Expression, left_id: u32, right_id: u32) -> Option<&str> {
    let left = expr.node(left_id)?;
    let right = expr.node(right_id)?;
    if is_event_template_id_read(&left.op) {
        if let Op::TextLiteral(lit) = &right.op {
            return Some(lit.value.as_str());
        }
    }
    if is_event_template_id_read(&right.op) {
        if let Op::TextLiteral(lit) = &left.op {
            return Some(lit.value.as_str());
        }
    }
    None
}

fn is_event_template_id_read(op: &Op) -> bool {
    matches!(
        op,
        Op::ReadText(read)
            if matches!(&read.subject, SubjectRef::Event(spellkave_dsl::EventRef::Detail { field_id }) if field_id == "template_id")
                && read.field_id == "template_id"
    )
}

/// Kernel-projected Int facts that `stat`/`effective` may read without an
/// authored catalog row (the kernel pushes them; they are reserved ids).
const KERNEL_PROJECTED_INT_FIELDS: &[&str] = &[
    spellkave_dsl::WORLD_TICK_FIELD_ID,
    "core/distance_to_actor_mm",
    "core/distance_to_departed_mm",
    "core/distance_to_destination_mm",
    "core/dispatch_arg_quantity",
    "core/session_id",
    "core/entity_id",
    "core/is_exposed",
    "core/resolution_roll_modifier",
];

fn formula_stat_subject_is_catalogued(subject: &SubjectRef) -> bool {
    matches!(
        subject,
        SubjectRef::Actor
            | SubjectRef::Target
            | SubjectRef::ActorWieldedItem
            | SubjectRef::ActorEquippedItem { .. }
    )
}

fn check_formula_stat_refs(
    expr: &Expression,
    owning_bundle: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    for node in &expr.nodes {
        match &node.op {
            Op::ReadInt(rf) | Op::ReadEffectiveInt(rf) => {
                if !formula_stat_subject_is_catalogued(&rf.subject) {
                    continue;
                }
                check_formula_field_ref(
                    &rf.field_id,
                    false,
                    owning_bundle,
                    path,
                    row_id,
                    index,
                    diags,
                );
            }
            Op::ReadResourceMax(rf) | Op::ReadResourceMin(rf) => {
                if !formula_stat_subject_is_catalogued(&rf.subject) {
                    continue;
                }
                check_formula_field_ref(
                    &rf.field_id,
                    true,
                    owning_bundle,
                    path,
                    row_id,
                    index,
                    diags,
                );
            }
            _ => {}
        }
    }
}

fn check_formula_field_ref(
    field_id: &str,
    resource_only: bool,
    owning_bundle: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    let field_id = field_id.trim();
    if field_id.is_empty() || KERNEL_PROJECTED_INT_FIELDS.contains(&field_id) {
        return;
    }
    let resolved = if resource_only {
        index
            .resource_definitions
            .get(field_id)
            .map(|bundle| bundle.as_str())
    } else {
        index
            .resource_definitions
            .get(field_id)
            .or_else(|| index.object_state_definitions.get(field_id))
            .map(|bundle| bundle.as_str())
    };
    let kind = if resource_only {
        "resource_definitions"
    } else {
        "resource_definitions or object_state_definitions"
    };
    match resolved {
        None => {
            // sk-zqc CR-12 two-tier trust, mirrored from `check_frame_write_ref`
            // (MF-VAL-217): an unresolved stat is only a genuine miss when every
            // bundle in the owning bundle's `depends_on` closure is present in
            // this manifest, or when the server admits into a live world (its
            // vocabulary is authoritative). A per-file validate, a verb-runner
            // slice or a doc snippet that reads `srd/str` declared elsewhere
            // is correct as written — nothing is being admitted at that moment.
            let closure_visible = index
                .bundle_scope
                .closure_of(owning_bundle)
                .is_some_and(|reach| reach.iter().all(|b| index.present_bundles.contains(b)));
            if closure_visible || index.live_world {
                diags.push(diag_fatal(
                    MF_VAL_DSL_FORMULA_STAT_REF,
                    path,
                    format!(
                        "row '{row_id}' formula references '{field_id}' which does not resolve to any authored {kind} row"
                    ),
                ));
            } else {
                diags.push(Diagnostic {
                    code: MF_VAL_DSL_FORMULA_STAT_REF.to_owned(),
                    severity: Severity::Info,
                    message: format!(
                        "row '{row_id}' formula references '{field_id}' which is not declared in this manifest; trusted because '{owning_bundle}'s depends_on closure is not fully present in this validation pass (a per-file `just validate`) — nothing is being admitted here"
                    ),
                    path: path.to_owned(),
                    span: None,
                    hint: Some(
                        "if this is a typo, catch it statically with the full closure present \
                         (`just check-manifest-refs`, or `--bundle --dep`); a live-published world \
                         resolves this identically at admission"
                            .to_owned(),
                    ),
                    details: None,
                });
            }
        }
        Some(ref_bundle) => {
            if !index.bundle_scope.in_scope(owning_bundle, ref_bundle) {
                diags.push(diag_fatal(
                    MF_VAL_CROSS_BUNDLE_SCOPE,
                    path,
                    format!(
                        "row '{row_id}' in bundle '{owning_bundle}' formula references '{field_id}' in bundle '{ref_bundle}', but '{owning_bundle}' does not depend on '{ref_bundle}'"
                    ),
                ));
            }
        }
    }
}

fn check_roll_table_ref(
    table_id: &str,
    owning_bundle: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
    is_grafted: bool,
) {
    let code = if is_grafted {
        MF_VAL_HOOK_GRAFTED_REF
    } else {
        MF_VAL_DSL_ROLL_TABLE_REF
    };
    match index.roll_tables.get(table_id) {
        None => diags.push(diag_fatal(
            code,
            path,
            format!(
                "{row_id}: DSL roll_table('{table_id}') does not resolve to any authored roll_table row"
            ),
        )),
        Some(ref_bundle) => {
            if !index.bundle_scope.in_scope(owning_bundle, ref_bundle.as_str()) {
                diags.push(diag_fatal(
                    MF_VAL_CROSS_BUNDLE_SCOPE,
                    path,
                    format!(
                        "{row_id}: DSL roll_table('{table_id}') resolves to bundle '{ref_bundle}' but owning bundle '{owning_bundle}' does not depend on it"
                    ),
                ));
            }
        }
    }
}

fn check_template_id_literal(
    literal: &str,
    owning_bundle: &str,
    path: &str,
    row_id: &str,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
    is_grafted: bool,
) {
    let code = if is_grafted {
        MF_VAL_HOOK_GRAFTED_REF
    } else {
        MF_VAL_DSL_TEMPLATE_ID_LITERAL
    };
    match index.resolve_template_id(literal) {
        None => diags.push(diag_warning(
            code,
            path,
            format!(
                "{row_id}: DSL $event.detail.template_id == '{literal}' — literal does not resolve to any authored affordance_template.template_id, action_definition.action_id, or outcome_template.outcome_template_id. The literal may name a synthesized Layer-2 event payload (no manifest row to match) — this is informational, not a rejection."
            ),
        )),
        Some(ref_bundle) => {
            if !index.bundle_scope.in_scope(owning_bundle, ref_bundle) {
                diags.push(diag_fatal(
                    MF_VAL_CROSS_BUNDLE_SCOPE,
                    path,
                    format!(
                        "{row_id}: DSL template_id literal '{literal}' resolves to bundle '{ref_bundle}' but owning bundle '{owning_bundle}' does not depend on it"
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Class 4 — grafted condition-hook sub-expressions
// ---------------------------------------------------------------------------

fn validate_condition_hook_grafted_refs(
    manifest: &Manifest,
    index: &RefIndex,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    for row in &manifest.condition_definitions {
        for (hook_index, hook) in row.hooks.iter().enumerate() {
            let path_prefix = format!(
                "condition_definition.hooks[{hook_index}] (condition_definition '{}')",
                row.condition_definition_id
            );
            if let Some(source) = hook.detail_filter_source.as_ref() {
                if let Ok(expr) = crate::dsl_macros::parse_predicate(source, macros) {
                    walk_expr_refs(
                        &expr,
                        &row.bundle_id,
                        &format!("{path_prefix}.detail_filter_source"),
                        &row.condition_definition_id,
                        index,
                        diags,
                        true,
                    );
                }
            }
            if let Some(source) = hook.expression_source.as_ref() {
                if let Ok(expr) = crate::dsl_macros::parse_outcome(source, macros) {
                    walk_expr_refs(
                        &expr,
                        &row.bundle_id,
                        &format!("{path_prefix}.expression_source"),
                        &row.condition_definition_id,
                        index,
                        diags,
                        true,
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// manipulation_action tag/action linkage guards (issue #209 traps 1/2/3)
// ---------------------------------------------------------------------------

fn validate_manipulation_action_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    let outcomes_by_id: BTreeMap<&str, &spellkave_schema::OutcomeTemplateRow> = manifest
        .outcome_templates
        .iter()
        .map(|row| (row.outcome_template_id.as_str(), row))
        .collect();
    let actions_by_id: BTreeMap<&str, &spellkave_schema::ActionDefinitionRow> = manifest
        .action_definitions
        .iter()
        .map(|row| (row.action_id.as_str(), row))
        .collect();
    // Cross-bundle leniency (CR-12 — mirrors `action_economy::resolve_scoped_ref`).
    // `just seed packages/spellkave-mechanics/fixtures/mech_action_composition`
    // validates a PARTIAL manifest (its own `[[bundles]]` rows only); core content
    // arrives at apply-time from the already-seeded DB. A `manipulation_action_id`
    // absent from THIS manifest is genuinely missing only when the owning bundle's
    // WHOLE depends_on closure is present here (a full `--bundle --dep` merge —
    // complete information); then an absent action stays fatal, catching a
    // cross-bundle TYPO too. When the closure is NOT fully present (the partial
    // seed), trust the dependency — e.g. a bundle's reaction whose outcome maps to
    // a dependency's own generic action id (#1227); the runtime resolves it against
    // the merged world. (Closure-completeness, NOT an id-prefix-shape check — the
    // latter would mis-trust a typo'd sibling id when the dependency IS merged. The
    // runtime backstop: a typo'd manipulation_action_id is a silently-inert
    // affordance until that exact id is requested — `r_manipulate` selection — so
    // the static gate matters.) Tracking issue: the admission-contract change
    // (ADR-0023 single source, ADR-0024 three runtimes).
    let present_bundles: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|b| b.bundle_id.as_str())
        .collect();
    let closure_fully_present = |owning: &str| -> bool {
        index
            .bundle_scope
            .closure_of(owning)
            .is_some_and(|reach| reach.iter().all(|b| present_bundles.contains(b.as_str())))
    };
    let mut action_tags_by_id: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in &manifest.action_definition_tag {
        action_tags_by_id
            .entry(row.action_id.as_str())
            .or_default()
            .push(row.tag.as_str());
    }
    // Outcome-template tags now live only in the relational `[[outcome_template_tag]]`
    // section (the inline `outcome_templates.tags` CSV form was removed, CR-4). The
    // deprecated-descriptor check below reconstructs the CSV from these rows so the
    // MF-VAL-227/MF-VAL-220 diagnostics still fire on the relational form.
    let mut outcome_tags_by_id: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in &manifest.outcome_template_tag {
        outcome_tags_by_id
            .entry(row.outcome_template_id.as_str())
            .or_default()
            .push(row.tag.as_str());
    }
    let mut checked_non_empty_on_success: BTreeSet<String> = BTreeSet::new();
    let mut warned_missing_communication_descriptor: BTreeSet<String> = BTreeSet::new();
    let mut warned_missing_sense_descriptor: BTreeSet<String> = BTreeSet::new();

    // The authored `perception` tag identifies perception-scoped niches.
    let perception_niches: BTreeSet<&str> = manifest
        .niche_definition_tag
        .iter()
        .filter(|row| row.tag == "perception")
        .map(|row| row.niche_id.as_str())
        .collect();
    let perception_capabilities: BTreeSet<&str> = manifest
        .capability_definition_tag
        .iter()
        .filter(|row| row.tag == "perception")
        .map(|row| row.capability_id.as_str())
        .collect();

    // Brief 11 D3: `target_kind` removed from `affordance_template`. Wave B
    // moved behavior-driving action coupling from descriptor tags to the typed
    // `outcome_templates.manipulation_action_id` field. Legacy tag keys remain
    // parsed only so the validator can reject them with a targeted diagnostic.
    for affordance in &manifest.affordance_templates {
        let Some(outcome) = outcomes_by_id.get(affordance.outcome_template_id.as_str()) else {
            continue;
        };
        let outcome_tags_csv = outcome_tags_by_id
            .get(outcome.outcome_template_id.as_str())
            .map(|tags| tags.join(","))
            .unwrap_or_default();
        let parsed_tags = parse_action_tags(&outcome_tags_csv);
        for key in ["manipulation.action_id", "action_id"] {
            if parsed_tags.has_values_for(key) {
                diags.push(diag_fatal(
                    MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG,
                    "outcome_template_tag.tag",
                    format!(
                        "outcome_template '{}' uses deprecated descriptor tag '{}'; use outcome_templates.manipulation_action_id instead",
                        outcome.outcome_template_id, key
                    ),
                ));
            }
        }
        if parsed_tags.has_values_for("communication.mode") {
            diags.push(diag_fatal(
                MF_VAL_TYPED_DESCRIPTOR_LEGACY_TAG,
                "outcome_template_tag.tag",
                format!(
                    "outcome_template '{}' uses deprecated descriptor tag 'communication.mode'; use action_definitions.resolution_mode instead",
                    outcome.outcome_template_id
                ),
            ));
        }

        for key in &parsed_tags.typo_like_keys {
            diags.push(diag_warning(
                MF_VAL_MANIP_ACTION_TAG_TYPO,
                "outcome_template_tag.tag",
                format!(
                    "outcome_template '{}' has unknown tags key '{}' resembling a deprecated manipulation descriptor; use outcome_templates.manipulation_action_id instead",
                    outcome.outcome_template_id, key
                ),
            ));
        }

        if outcome.communication_speech_act_id.trim().is_empty()
            && is_communication_affordance(affordance)
            && warned_missing_communication_descriptor.insert(outcome.outcome_template_id.clone())
        {
            diags.push(diag_warning(
                MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR,
                "outcome_templates.communication_speech_act_id",
                format!(
                    "communication-scoped outcome_template '{}' selected by affordance_template '{}' lacks typed communication_speech_act_id",
                    outcome.outcome_template_id, affordance.template_id
                ),
            ));
        }

        // MF-VAL-229: the sense sibling of MF-VAL-228. A perception-scoped
        // affordance whose outcome omits the typed `sense_query_kind` validates +
        // applies but `r_sense` skips it (empty query id → `continue`), so the
        // sensing query is silently unperceivable. Warn (not Fatal): an author may
        // intentionally stage a not-yet-wired sense template.
        if outcome.sense_query_kind.trim().is_empty()
            && is_sense_affordance(affordance, &perception_niches, &perception_capabilities)
            && warned_missing_sense_descriptor.insert(outcome.outcome_template_id.clone())
        {
            diags.push(diag_warning(
                MF_VAL_SENSE_QUERY_KIND_DESCRIPTOR,
                "outcome_templates.sense_query_kind",
                format!(
                    "perception-scoped outcome_template '{}' selected by affordance_template '{}' lacks typed sense_query_kind; r_sense will skip it and the sensing query will be unperceivable",
                    outcome.outcome_template_id, affordance.template_id
                ),
            ));
        }

        let typed_action_id = outcome.manipulation_action_id.trim();
        let resolved_action_id = (!typed_action_id.is_empty()).then_some(typed_action_id);
        let action_path = "outcome_templates.manipulation_action_id";

        let Some(action_id) = resolved_action_id else {
            if !parsed_tags.typo_like_keys.is_empty() {
                diags.push(diag_fatal(
                    MF_VAL_MANIP_ACTION_TAG_SHAPE,
                    "outcome_template_tag.tag",
                    format!(
                        "outcome_template '{}' has action-like tag key(s) {:?} but no typed manipulation_action_id binding",
                        outcome.outcome_template_id, parsed_tags.typo_like_keys
                    ),
                ));
            }
            continue;
        };

        let action = actions_by_id.get(action_id);
        if action.is_none() && closure_fully_present(&affordance.bundle_id) {
            diags.push(diag_fatal(
                MF_VAL_MANIP_ACTION_TEMPLATE_ACTION_REF,
                "affordance_templates.template_id",
                format!(
                    "affordance_template '{}' resolves manipulation action_id '{}' but no matching action_definition.action_id row exists",
                    affordance.template_id, action_id
                ),
            ));
        }
        if let Some(action) = action {
            if outcome.communication_speech_act_id.trim().is_empty()
                && is_communication_action(action, action_tags_by_id.get(action_id))
                && warned_missing_communication_descriptor
                    .insert(outcome.outcome_template_id.clone())
            {
                diags.push(diag_warning(
                    MF_VAL_COMMUNICATION_SPEECH_ACT_DESCRIPTOR,
                    "outcome_templates.communication_speech_act_id",
                    format!(
                        "communication-scoped outcome_template '{}' selected by action_definition '{}' lacks typed communication_speech_act_id",
                        outcome.outcome_template_id, action_id
                    ),
                ));
            }
        }

        match index.action_definitions.get(action_id) {
            None if closure_fully_present(&outcome.bundle_id) => diags.push(diag_fatal(
                MF_VAL_MANIP_ACTION_ACTION_REF,
                action_path,
                format!(
                    "outcome_template '{}' resolves manipulation action_id '{}' but no action_definition.action_id row exists",
                    outcome.outcome_template_id, action_id
                ),
            )),
            // CR-12 lenient: action absent from a PARTIAL manifest (closure not fully
            // present) — trust the dependency bundle to provide it at apply-time.
            None => {}
            Some(ref_bundle) => {
                if !index.bundle_scope.in_scope(&affordance.bundle_id, ref_bundle) {
                    diags.push(diag_fatal(
                        MF_VAL_CROSS_BUNDLE_SCOPE,
                        action_path,
                        format!(
                            "outcome_template '{}' resolves action_id '{}' in bundle '{}', but owning bundle '{}' does not depend on it",
                            outcome.outcome_template_id, action_id, ref_bundle, affordance.bundle_id
                        ),
                    ));
                }
                if checked_non_empty_on_success.insert(action_id.to_owned()) {
                    if let Some(action) = actions_by_id.get(action_id) {
                        if action.on_success.trim().is_empty() {
                            diags.push(diag_fatal(
                                MF_VAL_MANIP_ACTION_ON_SUCCESS,
                                "action_definitions.on_success",
                                format!(
                                    "action_definition '{}' is dispatchable from manipulation_action affordances but has empty on_success; author a no-op consequence_template (empty frame_writes is valid) or wire an existing one",
                                    action_id
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
}

struct ParsedActionTags {
    manipulation_values: Vec<String>,
    action_values: Vec<String>,
    communication_mode_values: Vec<String>,
    typo_like_keys: Vec<String>,
}

fn is_communication_action(
    action: &spellkave_schema::ActionDefinitionRow,
    explicit_tags: Option<&Vec<&str>>,
) -> bool {
    if action.required_capability == "core/communicate" {
        return true;
    }
    // Action tags now live only in the relational `[[action_definition_tag]]`
    // section (`explicit_tags`); the inline `action_definitions.tags` CSV form
    // was removed (CR-4).
    explicit_tags
        .map(|tags| tags.iter().any(|tag| tag_is_communication(tag)))
        .unwrap_or(false)
}

fn is_communication_affordance(affordance: &spellkave_schema::AffordanceTemplateRow) -> bool {
    affordance.capability_id == "core/communicate"
        || affordance.niche_id == "core/communicate"
        || affordance.niche_id == "communication"
}

/// Tag-driven perception-scope detection (canon-warden ruling, C13): an affordance
/// is sense-scoped when its niche or capability carries the authored `perception`
/// tag — NOT by a hardcoded `core/sense` id. Keeps the MF-VAL-229 nudge
/// based on authored tags.
fn is_sense_affordance(
    affordance: &spellkave_schema::AffordanceTemplateRow,
    perception_niches: &BTreeSet<&str>,
    perception_capabilities: &BTreeSet<&str>,
) -> bool {
    perception_niches.contains(affordance.niche_id.as_str())
        || perception_capabilities.contains(affordance.capability_id.as_str())
}

fn tag_is_communication(tag: &str) -> bool {
    matches!(tag, "communication" | "communicate" | "core/communicate")
}

impl ParsedActionTags {
    fn has_values_for(&self, key: &str) -> bool {
        match key {
            "manipulation.action_id" => !self.manipulation_values.is_empty(),
            "action_id" => !self.action_values.is_empty(),
            "communication.mode" => !self.communication_mode_values.is_empty(),
            _ => false,
        }
    }
}

fn parse_action_tags(tags: &str) -> ParsedActionTags {
    let mut manipulation_values = Vec::new();
    let mut action_values = Vec::new();
    let mut communication_mode_values = Vec::new();
    let mut typo_like_keys = Vec::new();

    for entry in tags
        .split([',', ';'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().to_string();
        match key {
            "manipulation.action_id" => manipulation_values.push(value),
            "action_id" => action_values.push(value),
            "communication.mode" => communication_mode_values.push(value),
            _ if looks_like_action_id_typo(key) => typo_like_keys.push(key.to_owned()),
            _ => {}
        }
    }

    ParsedActionTags {
        manipulation_values,
        action_values,
        communication_mode_values,
        typo_like_keys,
    }
}

fn looks_like_action_id_typo(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.starts_with("manip")
        || lower.contains("action_id")
        || lower.contains("action-id")
        || lower.contains("actionid")
}

// ---------------------------------------------------------------------------
// Equipment refs — item_definition.slot_id and actor_equipment_slot.item_key
// ---------------------------------------------------------------------------

fn validate_equipment_refs(manifest: &Manifest, index: &RefIndex, diags: &mut Vec<Diagnostic>) {
    for row in &manifest.item_definitions {
        if row.slot_id.is_empty() {
            continue;
        }
        match index.equipment_slot_definitions.get(row.slot_id.as_str()) {
            None => diags.push(diag_fatal(
                MF_VAL_ITEM_DEF_SLOT_REF,
                "item_definitions.slot_id",
                format!(
                    "item_definition '{}' slot_id '{}' does not resolve to any equipment_slot_definition",
                    row.item_def_id, row.slot_id
                ),
            )),
            Some(ref_bundle) => {
                if !index.bundle_scope.in_scope(&row.bundle_id, ref_bundle.as_str()) {
                    diags.push(diag_fatal(
                        MF_VAL_CROSS_BUNDLE_SCOPE,
                        "item_definitions.slot_id",
                        format!(
                            "item_definition '{}' references slot_id '{}' in bundle '{}', but bundle '{}' does not depend on it",
                            row.item_def_id, row.slot_id, ref_bundle, row.bundle_id
                        ),
                    ));
                }
            }
        }
    }

    for row in &manifest.actor_equipment_slots {
        if row.item_key.is_empty() {
            continue;
        }
        if !index.items.contains_key(row.item_key.as_str()) {
            diags.push(diag_fatal(
                MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF,
                "actor_equipment_slots.item_key",
                format!(
                    "actor_equipment_slot actor '{}' slot '{}' item_key '{}' does not resolve to any item",
                    row.actor_entity_key, row.slot_id, row.item_key
                ),
            ));
        }
    }

    // sk-9ll.3: mirrors the `item_definitions.slot_id` check immediately
    // above — the instance-level `actor_equipment_slots.slot_id` deserves the
    // SAME existence + cross-bundle-scope treatment its definition-level
    // sibling already gets. Without this, an authored instance row naming a
    // never-defined slot_id passed static validation silently and only
    // surfaced (if ever) as an opaque runtime `SlotDefinitionNotFound`.
    for row in &manifest.actor_equipment_slots {
        if row.slot_id.is_empty() {
            continue;
        }
        match index.equipment_slot_definitions.get(row.slot_id.as_str()) {
            None => diags.push(diag_fatal(
                MF_VAL_ACTOR_EQUIP_SLOT_DEF_REF,
                "actor_equipment_slots.slot_id",
                format!(
                    "actor_equipment_slot actor '{}' item_key '{}' slot_id '{}' does not resolve to any equipment_slot_definition",
                    row.actor_entity_key, row.item_key, row.slot_id
                ),
            )),
            Some(ref_bundle) => {
                // `ActorEquipmentSlotRow` has no `bundle_id` (schema trap noted on
                // `add_actor_equipment_slot`) — scope against the OWNING item's
                // bundle instead, mirroring how `MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF`
                // already resolves `item_key` through the item's own row rather
                // than a bundle_id this row kind does not carry.
                if let Some(item_bundle) = index.items.get(row.item_key.as_str()) {
                    if !index.bundle_scope.in_scope(item_bundle, ref_bundle.as_str()) {
                        diags.push(diag_fatal(
                            MF_VAL_CROSS_BUNDLE_SCOPE,
                            "actor_equipment_slots.slot_id",
                            format!(
                                "actor_equipment_slot actor '{}' item_key '{}' references slot_id '{}' in bundle '{}', but bundle '{}' does not depend on it",
                                row.actor_entity_key, row.item_key, row.slot_id, ref_bundle, item_bundle
                            ),
                        ));
                    }
                }
            }
        }
    }

    let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (idx, row) in manifest.actor_equipment_slots.iter().enumerate() {
        let key = (row.actor_entity_key.clone(), row.slot_id.clone());
        if let Some(first_idx) = seen.get(&key) {
            diags.push(diag_fatal(
                MF_VAL_EQUIP_SLOT_DUP,
                "actor_equipment_slots.slot_id",
                format!(
                    "duplicate actor_equipment_slot pair (actor_entity_key='{}', slot_id='{}') at rows [{}, {}]",
                    row.actor_entity_key, row.slot_id, first_idx, idx
                ),
            ));
        } else {
            seen.insert(key, idx);
        }
    }
}

/// sk-ntz.4 — instance-level sibling of
/// `validate_definition_seed_equipment_slot_compatibility` (MF-VAL-315):
/// catch an `actor_equipment_slots` row whose item's `entity_definitions.
/// slot_compatibility_predicate` rejects the slot it is being authored into.
/// Before this check, only the runtime `EquipItem` apply path
/// (`equipment::check_slot_compatibility`) enforced the predicate for
/// instance-level authoring seeds — the same class of authoring-time gap
/// ADR-0028 D-7 closed for definition seeds via MF-VAL-315.
///
/// Mirrors the runtime's own "no resolvable definition -> compatible"
/// fallback: `equipment::check_slot_compatibility` takes `item_def: Option<&
/// EntityDefinition>` and short-circuits to `Ok(())` when `None` — an entity
/// with no `entity_def_id`, or one that does not resolve to any authored
/// `entity_definitions` row, has no predicate to evaluate, so this check
/// silently skips it too (other checks — the entity-definition-seed Class 11
/// family — are responsible for flagging a dangling `entity_def_id` itself,
/// if that is ever added for `[[entities]]` rows).
fn validate_actor_equipment_slot_compatibility(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let entity_def_id_by_key: BTreeMap<&str, &str> = manifest
        .entities
        .iter()
        .map(|e| (e.entity_key.as_str(), e.entity_def_id.as_str()))
        .collect();
    let predicate_by_def_id: BTreeMap<&str, &str> = manifest
        .entity_definitions
        .iter()
        .map(|d| {
            (
                d.entity_def_id.as_str(),
                d.slot_compatibility_predicate.as_str(),
            )
        })
        .collect();

    for row in &manifest.actor_equipment_slots {
        let Some(entity_def_id) = entity_def_id_by_key.get(row.item_key.as_str()) else {
            // Unknown item_key is already reported by MF_VAL_ACTOR_EQUIP_ITEM_DEF_REF.
            continue;
        };
        if entity_def_id.is_empty() {
            // No entity_def_id -> no entity_definitions row -> no predicate to
            // evaluate; the runtime treats this the same way (compatible).
            continue;
        }
        let Some(predicate) = predicate_by_def_id.get(*entity_def_id) else {
            continue; // a dangling entity_def_id is not this check's concern
        };
        if let Some(reason) =
            crate::equip_predicate::incompatibility_reason(predicate, &row.slot_id)
        {
            diags.push(diag_fatal(
                MF_VAL_ACTOR_EQUIP_SLOT_COMPATIBILITY,
                "actor_equipment_slots.slot_id",
                format!(
                    "actor_equipment_slot actor '{}' item_key '{}' cannot be equipped into slot '{}' — {reason}",
                    row.actor_entity_key, row.item_key, row.slot_id
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Class 9 — entity_tag.entity_key resolves to an authored entity instance
// ---------------------------------------------------------------------------

/// `[[entity_tag]]` rows carry an `entity_key` that the manifest apply resolves
/// against authored `[[entities]]` (→ `entity_ids`). A key resolving to nothing
/// would silently attach the tag to nothing — reject it at authoring time
/// (MF-VAL-242), mirroring the loud-failure discipline the knowledge-route
/// binding gap (V2/CR-18) also calls for.
fn validate_entity_tag_refs(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let mut known_keys: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.entities {
        known_keys.insert(row.entity_key.as_str());
    }
    for row in &manifest.entity_tag {
        if !known_keys.contains(row.entity_key.as_str()) {
            diags.push(diag_fatal(
                MF_VAL_ENTITY_TAG_ENTITY_REF,
                "entity_tag.entity_key",
                format!(
                    "entity_tag row tag '{}' references unknown entity_key '{}' — no authored entity in this manifest carries it",
                    row.tag, row.entity_key
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Class 10b — place_edge_affordance bindings_json covers predicate bindings
// ---------------------------------------------------------------------------

fn validate_affordance_binding_coverage(
    manifest: &Manifest,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    let template_predicate: BTreeMap<&str, &str> = manifest
        .affordance_templates
        .iter()
        .map(|t| (t.template_id.as_str(), t.predicate_source.as_str()))
        .collect();

    for row in &manifest.place_edge_affordance {
        let Some(source) = template_predicate.get(row.affordance_template_id.as_str()) else {
            continue; // an unknown template is reported by FK closure
        };
        if source.is_empty() {
            continue;
        }
        let Ok(expr) = crate::dsl_macros::parse_predicate(source, macros) else {
            continue; // a predicate parse failure is MF-VAL-201's job
        };
        let binding_names = predicate_binding_ref_names(&expr);
        if binding_names.is_empty() {
            continue;
        }
        let trimmed = row.bindings_json.trim();
        let bindings: BTreeMap<String, String> =
            serde_json::from_str(if trimmed.is_empty() { "{}" } else { trimmed })
                .unwrap_or_default();
        for name in binding_names {
            if !bindings.contains_key(name) {
                diags.push(diag_fatal(
                    MF_VAL_AFFORDANCE_BINDING_COVERAGE,
                    "place_edge_affordance.bindings_json",
                    format!(
                        "place_edge_affordance for edge '{}' references affordance_template '{}' whose predicate requires binding '{}' but bindings_json does not provide it",
                        row.edge_key, row.affordance_template_id, name
                    ),
                ));
            }
        }
    }
}

/// ADR-0101 D-1/D-5 sibling of [`validate_affordance_binding_coverage`]: the
/// same per-row binding-reference-closure check, run over
/// `[[movement_defaults]]` instead of `[[place_edge_affordance]]`.
fn validate_movement_default_binding_coverage(
    manifest: &Manifest,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    let template_predicate: BTreeMap<&str, &str> = manifest
        .affordance_templates
        .iter()
        .map(|t| (t.template_id.as_str(), t.predicate_source.as_str()))
        .collect();

    for row in &manifest.movement_defaults {
        let Some(source) = template_predicate.get(row.affordance_template_id.as_str()) else {
            continue; // an unknown template is reported by FK closure
        };
        if source.is_empty() {
            continue;
        }
        let Ok(expr) = crate::dsl_macros::parse_predicate(source, macros) else {
            continue; // a predicate parse failure is MF-VAL-201's job
        };
        let binding_names = predicate_binding_ref_names(&expr);
        if binding_names.is_empty() {
            continue;
        }
        let trimmed = row.bindings_json.trim();
        let bindings: BTreeMap<String, String> =
            serde_json::from_str(if trimmed.is_empty() { "{}" } else { trimmed })
                .unwrap_or_default();
        for name in binding_names {
            if !bindings.contains_key(name) {
                diags.push(diag_fatal(
                    MF_VAL_MOVEMENT_DEFAULT_BINDING_COVERAGE,
                    "movement_defaults.bindings_json",
                    format!(
                        "movement_default for bundle '{}' place_graph_id '{}' mode_id '{}' references affordance_template '{}' whose predicate requires binding '{}' but bindings_json does not provide it",
                        row.bundle_id, row.place_graph_id, row.mode_id, row.affordance_template_id, name
                    ),
                ));
            }
        }
    }
}

/// ADR-0101 D-1/D-5: the authored-rows-only in-mode conflict lint (a
/// commit-time-authoritative sibling exists server-side, see
/// [`MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT`]'s doc comment) plus the
/// same-template-two-modes authoring-slip warning
/// ([`MF_VAL_MOVEMENT_MODE_TEMPLATE_REPEATED`]).
fn validate_movement_mode_composition(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    // In-mode binding conflicts across `place_edge_affordance`, grouped by
    // (edge_key, mode_id). Owned `String` keys/values throughout: each
    // row's parsed `bindings_json` map is a per-iteration temporary, so the
    // cross-iteration accumulators cannot borrow from it.
    let mut edge_mode_bindings: BTreeMap<(String, String), BTreeMap<String, String>> =
        BTreeMap::new();
    // Same-template-in-two-modes, grouped by edge_key only.
    let mut edge_template_modes: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for row in &manifest.place_edge_affordance {
        let bindings = row_bindings_object(&row.bindings_json);
        let scope = (row.edge_key.clone(), row.mode_id.clone());
        let claimed = edge_mode_bindings.entry(scope).or_default();
        for key in bindings.keys() {
            if let Some(other_template) = claimed.get(key) {
                if other_template != &row.affordance_template_id {
                    diags.push(diag_fatal(
                        MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT,
                        "place_edge_affordance.bindings_json",
                        format!(
                            "edge '{}' mode '{}': binding '{key}' is declared by both '{other_template}' and '{}'",
                            row.edge_key, row.mode_id, row.affordance_template_id
                        ),
                    ));
                }
            }
        }
        for key in bindings.keys() {
            claimed.insert(key.clone(), row.affordance_template_id.clone());
        }

        let modes_for_template = edge_template_modes.entry(row.edge_key.clone()).or_default();
        if let Some(other_mode) = modes_for_template.get(&row.affordance_template_id) {
            if other_mode != &row.mode_id {
                diags.push(diag_warning(
                    MF_VAL_MOVEMENT_MODE_TEMPLATE_REPEATED,
                    "place_edge_affordance.mode_id",
                    format!(
                        "edge '{}' attaches affordance_template '{}' under both mode '{other_mode}' and mode '{}' — two distinct modes, or a mistyped mode_id?",
                        row.edge_key, row.affordance_template_id, row.mode_id
                    ),
                ));
            }
        }
        modes_for_template.insert(row.affordance_template_id.clone(), row.mode_id.clone());
    }

    // In-mode binding conflicts across `movement_defaults`, grouped by
    // (bundle_id, place_graph_id, mode_id).
    let mut default_scope_bindings: BTreeMap<(String, String, String), BTreeMap<String, String>> =
        BTreeMap::new();
    for row in &manifest.movement_defaults {
        let bindings = row_bindings_object(&row.bindings_json);
        let scope = (
            row.bundle_id.clone(),
            row.place_graph_id.clone(),
            row.mode_id.clone(),
        );
        let claimed = default_scope_bindings.entry(scope).or_default();
        for key in bindings.keys() {
            if let Some(other_template) = claimed.get(key) {
                if other_template != &row.affordance_template_id {
                    diags.push(diag_fatal(
                        MF_VAL_MOVEMENT_MODE_BINDING_CONFLICT,
                        "movement_defaults.bindings_json",
                        format!(
                            "bundle '{}' place_graph_id '{}' mode '{}': binding '{key}' is declared by both '{other_template}' and '{}'",
                            row.bundle_id, row.place_graph_id, row.mode_id, row.affordance_template_id
                        ),
                    ));
                }
            }
        }
        for key in bindings.keys() {
            claimed.insert(key.clone(), row.affordance_template_id.clone());
        }
    }
}

fn row_bindings_object(bindings_json: &str) -> BTreeMap<String, String> {
    let trimmed = bindings_json.trim();
    serde_json::from_str(if trimmed.is_empty() { "{}" } else { trimmed }).unwrap_or_default()
}

fn predicate_binding_ref_names(expr: &Expression) -> BTreeSet<&str> {
    expr.nodes
        .iter()
        .filter_map(|node| {
            node.op
                .entity_ref_binding()
                .or_else(|| match &node.op {
                    Op::HasItem(op) => op.item_def_id.strip_prefix("$bindings."),
                    _ => None,
                })
                .or_else(|| node.op.binding_subject_name())
        })
        .filter(|name| !name.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Class 10 — place_edge_affordance knows_entity binding resolves to an entity
// ---------------------------------------------------------------------------

/// A `knows_entity($bindings.x)` predicate compares numeric entity ids, so the
/// affordance's binding for `x` must name an authored `[[entities]]` key —
/// apply rewrites it to that entity's id. A key resolving to nothing
/// validates + applies clean but leaves the gate silently false forever (the
/// V2 wall, CR-18) — reject it at authoring time (MF-VAL-244), the offline
/// twin of the apply-time resolver. A literal numeric id is an explicit
/// authored choice and passes. The "which bindings are entity refs" decision
/// is the shared `Op::entity_ref_binding` classifier, so this finding and the
/// apply resolver can never disagree.
fn validate_affordance_knowledge_bindings(
    manifest: &Manifest,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    let mut known_keys: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.entities {
        known_keys.insert(row.entity_key.as_str());
    }

    let template_predicate: BTreeMap<&str, &str> = manifest
        .affordance_templates
        .iter()
        .map(|t| (t.template_id.as_str(), t.predicate_source.as_str()))
        .collect();

    for row in &manifest.place_edge_affordance {
        let Some(source) = template_predicate.get(row.affordance_template_id.as_str()) else {
            continue; // an unknown template is reported by the apply path
        };
        if source.is_empty() {
            continue;
        }
        let Ok(expr) = crate::dsl_macros::parse_predicate(source, macros) else {
            continue; // a predicate parse failure is MF-VAL-201's job
        };
        let entity_ref_names: BTreeSet<&str> = expr
            .nodes
            .iter()
            .filter_map(|node| node.op.entity_ref_binding())
            .collect();
        if entity_ref_names.is_empty() {
            continue;
        }
        let trimmed = row.bindings_json.trim();
        let bindings: BTreeMap<String, String> =
            serde_json::from_str(if trimmed.is_empty() { "{}" } else { trimmed })
                .unwrap_or_default();
        for name in entity_ref_names {
            let Some(value) = bindings.get(name) else {
                continue; // a missing binding is the apply-time binding check's job
            };
            if value.parse::<u64>().is_ok() {
                continue; // literal numeric entity id — an explicit authored choice
            }
            if !known_keys.contains(value.as_str()) {
                diags.push(diag_fatal(
                    MF_VAL_AFFORDANCE_KNOWS_ENTITY_BINDING_REF,
                    "place_edge_affordance.bindings_json",
                    format!(
                        "place_edge_affordance for edge '{}' binds '{}' = '{}' to a knows_entity predicate, but no authored entity or world_object carries the key '{}' — the knowledge gate would resolve to nothing and stay silently false forever (author the entity or fix the key)",
                        row.edge_key, name, value, value
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Class 7 — modifier_template.applies_to resolves to a modifier_target_definition
// ---------------------------------------------------------------------------

fn validate_modifier_target_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    for row in &manifest.modifier_templates {
        if row.applies_to.is_empty() {
            continue;
        }
        match index
            .modifier_target_definitions
            .get(row.applies_to.as_str())
        {
            None => diags.push(diag_fatal(
                MF_VAL_MODIFIER_TEMPLATE_TARGET_REF,
                "modifier_templates.applies_to",
                format!(
                    "modifier_template '{}' applies_to '{}' does not resolve to any modifier_target_definition (seeded core: {} | authored: 0+ rows)",
                    row.modifier_template_id,
                    row.applies_to,
                    CORE_MODIFIER_TARGET_IDS.join(", ")
                ),
            )),
            Some(ref_bundle) => {
                if !index
                    .bundle_scope
                    .in_scope(&row.bundle_id, ref_bundle.as_str())
                {
                    diags.push(diag_fatal(
                        MF_VAL_CROSS_BUNDLE_SCOPE,
                        "modifier_templates.applies_to",
                        format!(
                            "modifier_template '{}' references applies_to '{}' in bundle '{}', but bundle '{}' does not depend on it",
                            row.modifier_template_id, row.applies_to, ref_bundle, row.bundle_id
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Classes 6 + 7 — consequence_frame_write JSON walker
// ---------------------------------------------------------------------------

fn validate_frame_write_refs(
    manifest: &Manifest,
    index: &RefIndex,
    macros: &[DslMacroDefinition],
    live_world: bool,
    diags: &mut Vec<Diagnostic>,
) {
    // Build owning bundle per template_id (from consequence_templates).
    let template_bundle: BTreeMap<&str, &str> = manifest
        .consequence_templates
        .iter()
        .map(|t| (t.template_id.as_str(), t.bundle_id.as_str()))
        .collect();
    // sk-zqc CR-12 closure-completeness gate (mirrors
    // `validate_manipulation_action_refs`'s `present_bundles`, one bundle-id
    // set built once per validate pass, not per row).
    let present: BTreeSet<&str> = manifest
        .bundles
        .iter()
        .map(|b| b.bundle_id.as_str())
        .collect();

    for row in &manifest.consequence_frame_writes {
        let owning_bundle = template_bundle
            .get(row.template_id.as_str())
            .copied()
            .unwrap_or("");
        let path = format!(
            "consequence_frame_writes.frame_write (template '{}', ordinal {})",
            row.template_id, row.ordinal
        );
        walk_frame_write_json(
            &row.frame_write,
            owning_bundle,
            &path,
            index,
            macros,
            &present,
            live_world,
            diags,
        );
    }
}

/// `consequence_frame_writes`
/// storage key is `(template_id, ordinal)`, so two blocks of one template
/// sharing an `ordinal` would collide at commit instead of failing cleanly at
/// authoring time. Same duplicate-pair shape as
/// `validate_actor_equipment_slot_compatibility`'s `MF_VAL_EQUIP_SLOT_DUP`
/// check.
fn validate_frame_write_ordinal_uniqueness(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let mut seen: BTreeMap<(&str, u32), usize> = BTreeMap::new();
    for (idx, row) in manifest.consequence_frame_writes.iter().enumerate() {
        let key = (row.template_id.as_str(), row.ordinal);
        if let Some(first_idx) = seen.get(&key) {
            diags.push(diag_fatal(
                MF_VAL_FRAME_WRITE_ORDINAL_DUP,
                "consequence_frame_writes.ordinal",
                format!(
                    "duplicate consequence_frame_writes ordinal {} for template '{}' at rows [{}, {}]",
                    row.ordinal, row.template_id, first_idx, idx
                ),
            ));
        } else {
            seen.insert(key, idx);
        }
    }
}

fn validate_self_reaction_resource_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    let writes_by_template: BTreeMap<&str, Vec<&serde_json::Value>> = manifest
        .consequence_frame_writes
        .iter()
        .fold(BTreeMap::new(), |mut acc, row| {
            acc.entry(row.template_id.as_str())
                .or_default()
                .push(&row.frame_write);
            acc
        });

    for action in &manifest.action_definitions {
        if !action.self_eligible {
            continue;
        }
        // A self-eligible reaction can trigger itself and needs a finite budget
        // (ADR-0050). Ordinary self-targeted actions are exempt.
        if action.reaction_eligibility.is_none() && action.reaction_trigger.is_none() {
            continue;
        }
        // `spends_channel` is the canonical action-economy budget gate. Its
        // existence and scope are validated by `action_economy`; only legacy
        // actions without a channel need the ModifyResource fallback below.
        if action
            .spends_channel
            .as_deref()
            .is_some_and(|channel| !channel.is_empty())
        {
            continue;
        }
        let resources: BTreeSet<String> = writes_by_template
            .get(action.on_success.as_str())
            .into_iter()
            .flat_map(|writes| writes.iter().copied())
            .flat_map(frame_write_modify_resources)
            .collect();
        if resources.is_empty() {
            diags.push(diag_fatal(
                MF_VAL_SELF_REACTION_RESOURCE_REF,
                "action_definitions.self_eligible",
                format!(
                    "self_eligible action_definition '{}' on_success '{}' has no ModifyResource budget resource",
                    action.action_id, action.on_success
                ),
            ));
            continue;
        }
        for resource in resources {
            match index.resource_definitions.get(resource.as_str()) {
                None => diags.push(diag_fatal(
                        MF_VAL_SELF_REACTION_RESOURCE_REF,
                        "action_definitions.self_eligible",
                        format!(
                            "self_eligible action_definition '{}' budget resource '{}' does not resolve to any resource_definition",
                            action.action_id, resource
                        ),
                    )),
                Some(ref_bundle) => {
                    if !index
                        .bundle_scope
                        .in_scope(action.bundle_id.as_str(), ref_bundle.as_str())
                    {
                        diags.push(diag_fatal(
                            MF_VAL_SELF_REACTION_RESOURCE_REF,
                            "action_definitions.self_eligible",
                            format!(
                                "self_eligible action_definition '{}' budget resource '{}' resolves to bundle '{}' outside owning bundle '{}'",
                                action.action_id, resource, ref_bundle, action.bundle_id
                            ),
                        ));
                    }
                }
            }
        }
    }
}

/// ADR-0142 D-1, sk-jay0.2.33.15.7 (canon-warden A-4): `prompt_definitions.
/// response_action_ids` names existing action_definition rows, resolved
/// through the SAME existence-plus-bundle-scope path the sibling live maps
/// (`economy_channels`/`modifier_templates`/`equipment_slot_definitions`/
/// `roll_tables`) already use — `index.action_definitions` carries both the
/// manifest's own rows AND (on the live path) `LiveVocabulary.actions`,
/// merged with bundle_id in `validate_cross_row_refs` above. A prompt may
/// only name actions in its OWN bundle's `depends_on` closure, never an
/// arbitrary already-published action anywhere in the world — the same
/// "in scope" guarantee `MF_VAL_CROSS_BUNDLE_SCOPE` already enforces for
/// every other cross-bundle reference kind.
fn validate_prompt_response_action_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    for row in &manifest.prompt_definitions {
        for action_id in &row.response_action_ids {
            match index.action_definitions.get(action_id.as_str()) {
                None => diags.push(diag_fatal(
                    MF_VAL_FK_CLOSURE,
                    "prompt_definitions.response_action_ids",
                    format!(
                        "prompt_definition '{}' response_action_ids names unknown action_definition '{}'",
                        row.prompt_def_id, action_id
                    ),
                )),
                Some(ref_bundle) => {
                    if !index.bundle_scope.in_scope(row.bundle_id.as_str(), ref_bundle.as_str()) {
                        diags.push(diag_fatal(
                            MF_VAL_CROSS_BUNDLE_SCOPE,
                            "prompt_definitions.response_action_ids",
                            format!(
                                "prompt_definition '{}' response_action_ids '{}' resolves to bundle '{}' outside owning bundle '{}'",
                                row.prompt_def_id, action_id, ref_bundle, row.bundle_id
                            ),
                        ));
                    }
                }
            }
        }
    }
}

fn frame_write_modify_resources(value: &serde_json::Value) -> Vec<String> {
    // `ForEachActorInRadius.body` is a `Vec<FrameWriteOp>` → a JSON array. Recurse
    // element-wise so a `ModifyResource` nested inside an AoE body counts toward the
    // MF-VAL-263 self-budget check (the `walk_frame_write_json` MF-VAL-217 walker
    // already does this for the same shape). Without this, a self-anchored AoE whose
    // only resource write lives in its body — e.g. `srd/cast_acid_splash` — was
    // wrongly reported as having no ModifyResource budget resource (#1216).
    if let serde_json::Value::Array(items) = value {
        return items
            .iter()
            .flat_map(frame_write_modify_resources)
            .collect();
    }
    let serde_json::Value::Object(map) = value else {
        return Vec::new();
    };
    let Some((variant, inner)) = map.iter().next() else {
        return Vec::new();
    };
    let serde_json::Value::Object(fields) = inner else {
        return Vec::new();
    };
    match variant.as_str() {
        "ModifyResource" => fields
            .get("resource")
            .and_then(serde_json::Value::as_str)
            .map(|resource| vec![resource.to_string()])
            .unwrap_or_default(),
        "ForEachActorInRadius" => fields
            .get("body")
            .map(frame_write_modify_resources)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Resolve one Class-6 frame-write ref (`ModifyResource.resource` /
/// `ModifyChannel.channel`) through the shared `resolve_scoped_ref` two-tier
/// trust (sk-zqc), then push the matching diagnostic.
///
/// `live_world` distinguishes two admission shapes that otherwise look
/// IDENTICAL to `resolve_scoped_ref` (a `depends_on` closure with `token`
/// absent from `decl_bundle`): `false` — every offline caller (CLI validate,
/// wasm bridge, per-file `just validate`) — trusts an undeclared token when
/// the closure isn't fully visible (CR-12): the file IS correct, nothing is
/// being admitted anywhere at that moment. `true` — the server's admission
/// path (`validate_manifest_with_world`) — has authoritative live-world
/// knowledge (sk-pm7's existence-only index is already merged into
/// `decl_bundle` by `validate_cross_row_refs`): an absent token there is a
/// genuine miss, not a visibility gap, so it stays Fatal regardless of
/// closure shape. This remains true when the supplied live world is empty.
#[allow(clippy::too_many_arguments)]
fn check_frame_write_ref(
    using_bundle: &str,
    token: &str,
    decl_bundle: &BTreeMap<&str, &str>,
    scope: &BundleScope,
    present: &BTreeSet<&str>,
    live_world: bool,
    code: &str,
    field: &str,
    target_kind: &str,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let dangling = || {
        diag_fatal(
            code,
            path,
            format!("{field} '{token}' does not resolve to any authored {target_kind} row"),
        )
    };
    match resolve_scoped_ref(using_bundle, token, decl_bundle, scope, present) {
        RefResolution::InScope => {}
        RefResolution::OutOfScope { decl_bundle } => diags.push(diag_fatal(
            MF_VAL_CROSS_BUNDLE_SCOPE,
            path,
            format!(
                "{field} '{token}' resolves to bundle '{decl_bundle}' but owning bundle '{using_bundle}' does not depend on it"
            ),
        )),
        RefResolution::UndeclaredButVisible => diags.push(dangling()),
        RefResolution::Trusted if live_world => diags.push(dangling()),
        RefResolution::Trusted => diags.push(Diagnostic {
            code: code.to_owned(),
            severity: Severity::Info,
            message: format!(
                "{field} '{token}' is not declared in this manifest; trusted because '{using_bundle}'s depends_on closure is not fully present in this validation pass (a per-file `just validate`) — nothing is being admitted here"
            ),
            path: path.to_owned(),
            span: None,
            hint: Some(
                "if this is a typo, catch it statically with the full closure present \
                 (`just check-manifest-refs`, or `--bundle --dep`); a live-published world \
                 resolves this identically at admission"
                    .to_owned(),
            ),
            details: None,
        }),
    }
}

/// Walk authored frame-write references and DSL expressions, including the fixed
/// OpenSession session definition. Nested radius bodies use the same dependency
/// closure and live-world admission policy.
#[allow(clippy::too_many_arguments)]
fn walk_frame_write_json(
    value: &serde_json::Value,
    owning_bundle: &str,
    path: &str,
    index: &RefIndex,
    macros: &[DslMacroDefinition],
    present: &BTreeSet<&str>,
    live_world: bool,
    diags: &mut Vec<Diagnostic>,
) {
    let serde_json::Value::Object(map) = value else {
        return;
    };
    // External serde-tagged enum: exactly one key naming the variant.
    let Some((variant, inner)) = map.iter().next() else {
        return;
    };
    let serde_json::Value::Object(fields) = inner else {
        return;
    };
    match variant.as_str() {
        "OpenSession" => {
            let definition = fields
                .get("session_def_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if definition.trim().is_empty() {
                diags.push(diag_fatal(
                    MF_VAL_FK_CLOSURE,
                    path,
                    "OpenSession.session_def_id must name an authored session definition".into(),
                ));
                return;
            }
            let decl_bundle = index
                .session_definitions
                .iter()
                .map(|(id, bundle)| (id.as_str(), bundle.as_str()))
                .collect();
            check_frame_write_ref(
                owning_bundle,
                definition,
                &decl_bundle,
                &index.bundle_scope,
                present,
                live_world,
                MF_VAL_FK_CLOSURE,
                "OpenSession.session_def_id",
                "session_definitions",
                path,
                diags,
            );
        }
        "ModifyResource" => {
            // ADR-0045 COND-4: `delta_source` is an authored Int DSL expression.
            if let Some(serde_json::Value::String(delta_source)) = fields.get("delta_source") {
                check_frame_write_dsl_expr(
                    delta_source,
                    ValueType::Int,
                    path,
                    owning_bundle,
                    index,
                    macros,
                    diags,
                );
            }
            if let Some(serde_json::Value::String(resource)) = fields.get("resource") {
                let decl_bundle: BTreeMap<&str, &str> = index
                    .resource_definitions
                    .iter()
                    .map(|(id, bundle)| (id.as_str(), bundle.as_str()))
                    .collect();
                check_frame_write_ref(
                    owning_bundle,
                    resource,
                    &decl_bundle,
                    &index.bundle_scope,
                    present,
                    live_world,
                    MF_VAL_FRAME_WRITE_RESOURCE_REF,
                    "ModifyResource.resource",
                    "resource_definitions",
                    path,
                    diags,
                );
            }
        }
        "ModifyChannel" => {
            // sk-ct8.4 / finding #86: `delta_source` empty/omitted is the
            // authored default (+1 full pool) — only a non-empty override is
            // an executable DSL Int expression (mirrors ModifyResource above,
            // whose delta_source is never optional).
            if let Some(serde_json::Value::String(delta_source)) = fields.get("delta_source") {
                if !delta_source.trim().is_empty() {
                    check_frame_write_dsl_expr(
                        delta_source,
                        ValueType::Int,
                        path,
                        owning_bundle,
                        index,
                        macros,
                        diags,
                    );
                }
            }
            if let Some(serde_json::Value::String(channel)) = fields.get("channel") {
                let decl_bundle: BTreeMap<&str, &str> = index
                    .economy_channels
                    .iter()
                    .map(|(id, bundle)| (id.as_str(), bundle.as_str()))
                    .collect();
                check_frame_write_ref(
                    owning_bundle,
                    channel,
                    &decl_bundle,
                    &index.bundle_scope,
                    present,
                    live_world,
                    MF_VAL_FRAME_WRITE_CHANNEL_REF,
                    "ModifyChannel.channel",
                    "economy_channel_definitions",
                    path,
                    diags,
                );
            }
        }
        "ForEachActorInRadius" => {
            // ADR-0042 D-2 retired the `center_id` sentinel (MF-VAL-218) with the
            // field: the authored shape is now `center_ref`/`shape_id`, and an
            // unresolvable center fails loudly at apply time. The live work here is
            // the recursive body walk (nested frame-writes still need ref-checking).
            // ADR-0045 COND-4: a non-empty `predicate_source` is a per-actor Bool.
            if let Some(serde_json::Value::String(predicate_source)) =
                fields.get("predicate_source")
            {
                if !predicate_source.trim().is_empty() {
                    check_frame_write_dsl_expr(
                        predicate_source,
                        ValueType::Bool,
                        path,
                        owning_bundle,
                        index,
                        macros,
                        diags,
                    );
                }
            }
            if let Some(serde_json::Value::Array(body)) = fields.get("body") {
                for inner_op in body {
                    walk_frame_write_json(
                        inner_op,
                        owning_bundle,
                        path,
                        index,
                        macros,
                        present,
                        live_world,
                        diags,
                    );
                }
            }
        }
        _ => {}
    }
}

/// ADR-0045 COND-4: parse + type-check an executable `frame_write` DSL string at
/// admission. `expected` is `Int` for `ModifyResource.delta_source` (parsed as a
/// bare int expression) and `Bool` for `ForEachActorInRadius.predicate_source`.
/// A parse failure is `MF-VAL-252`; a wrong root value-type is `MF-VAL-253`.
/// Both are Fatal — a malformed expression never reaches the runtime as a silent
/// `Err`.
fn check_frame_write_dsl_expr(
    source: &str,
    expected: ValueType,
    path: &str,
    owning_bundle: &str,
    index: &RefIndex,
    macros: &[DslMacroDefinition],
    diags: &mut Vec<Diagnostic>,
) {
    let parsed = match expected {
        ValueType::Int => crate::dsl_macros::parse_int_expression(source, macros),
        _ => crate::dsl_macros::parse_predicate(source, macros),
    };
    let expr = match parsed {
        Ok(expr) => expr,
        Err(failure) => {
            let detail = failure
                .diagnostics
                .first()
                .map(|d| format!("[{}] {}", d.code, d.message))
                .unwrap_or_else(|| "parse error".to_string());
            diags.push(diag_fatal(
                MF_VAL_FRAME_WRITE_DSL_PARSE,
                path,
                format!("frame_write DSL expression '{source}' failed to parse: {detail}"),
            ));
            return;
        }
    };
    // The parser roots a bare atom at its own value-type, so `parse_int_expression
    // ("true")` succeeds with a Bool root — the type check below is what rejects
    // a Bool-where-Int (and vice versa), not the parse.
    if root_value_type(&expr) != Some(expected) {
        diags.push(diag_fatal(
            MF_VAL_FRAME_WRITE_DSL_TYPE,
            path,
            format!(
                "frame_write DSL expression '{source}' has the wrong value-type (expected {expected:?})"
            ),
        ));
        return;
    }
    check_equipment_slot_refs(&expr, owning_bundle, path, path, index, diags);
    if expected == ValueType::Int {
        check_formula_stat_refs(&expr, owning_bundle, path, path, index, diags);
    }
}

/// The value-type of an expression's root node.
fn root_value_type(expr: &Expression) -> Option<ValueType> {
    expr.nodes
        .iter()
        .find(|node| node.node_id == expr.root_node_id)
        .map(|node| node.value_type)
}

// ---------------------------------------------------------------------------
// Class 8 — condition_definition.implies
// ---------------------------------------------------------------------------

fn validate_condition_implies_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    for row in &manifest.condition_definitions {
        for (idx, implied_id) in row.implies.iter().enumerate() {
            let path = format!(
                "condition_definitions.implies[{idx}] (condition_definition '{}')",
                row.condition_definition_id
            );
            match index.condition_definitions.get(implied_id.as_str()) {
                None => diags.push(diag_fatal(
                    MF_VAL_CONDITION_IMPLIES_REF,
                    &path,
                    format!(
                        "condition_definition '{}' implies '{}' which does not resolve to any authored condition_definition",
                        row.condition_definition_id, implied_id
                    ),
                )),
                Some(ref_bundle) => {
                    if !index
                        .bundle_scope
                        .in_scope(&row.bundle_id, ref_bundle.as_str())
                    {
                        diags.push(diag_fatal(
                            MF_VAL_CROSS_BUNDLE_SCOPE,
                            &path,
                            format!(
                                "condition_definition '{}' in bundle '{}' implies '{}' in bundle '{}', but '{}' does not depend on '{}'",
                                row.condition_definition_id,
                                row.bundle_id,
                                implied_id,
                                ref_bundle,
                                row.bundle_id,
                                ref_bundle
                            ),
                        ));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Class 14 — condition_definition capability grants / suppressions (sk-syo)
// ---------------------------------------------------------------------------

fn validate_condition_capability_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    for row in &manifest.condition_definitions {
        for (field, ids) in [
            (
                "grants_capability_while_active",
                &row.grants_capability_while_active,
            ),
            (
                "suppresses_capability_while_active",
                &row.suppresses_capability_while_active,
            ),
        ] {
            for (idx, capability_id) in ids.iter().enumerate() {
                let path = format!(
                    "condition_definitions.{field}[{idx}] (condition_definition '{}')",
                    row.condition_definition_id
                );
                match index.capability_definitions.get(capability_id.as_str()) {
                    None => diags.push(diag_fatal(
                        MF_VAL_CONDITION_CAPABILITY_REF,
                        &path,
                        format!(
                            "condition_definition '{}' {field} names '{}' which does not resolve to any authored capability_definition",
                            row.condition_definition_id, capability_id
                        ),
                    )),
                    Some(ref_bundle) => {
                        if !index
                            .bundle_scope
                            .in_scope(&row.bundle_id, ref_bundle.as_str())
                        {
                            diags.push(diag_fatal(
                                MF_VAL_CROSS_BUNDLE_SCOPE,
                                &path,
                                format!(
                                    "condition_definition '{}' in bundle '{}' {field} names '{}' in bundle '{}', but '{}' does not depend on '{}'",
                                    row.condition_definition_id,
                                    row.bundle_id,
                                    capability_id,
                                    ref_bundle,
                                    row.bundle_id,
                                    ref_bundle
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Class 11 — definition-level instance seeds (#728 / #729 + condition fossil)
// ---------------------------------------------------------------------------

fn validate_definition_seed_refs(
    manifest: &Manifest,
    index: &RefIndex,
    diags: &mut Vec<Diagnostic>,
) {
    // Resolve one seed ref: existence (against the first of `id_maps` that
    // has it) then Class-5 bundle scope. `kind` names the referenced
    // definition kind(s) for the message. `id_maps` is a union lookup —
    // `state_id` resolves against `object_state_definitions` OR
    // `resource_definitions` (battle-test finding #10 / MF-VAL-253 widening:
    // both catalogs are equally real at runtime, `apply_definition_seeds` is
    // catalog-agnostic) — every other ref in this function passes exactly one
    // map, unchanged.
    let check = |owning_bundle: &str,
                 ref_id: &str,
                 id_maps: &[&BTreeMap<String, String>],
                 code: &str,
                 path: &str,
                 kind: &str,
                 diags: &mut Vec<Diagnostic>| {
        match id_maps.iter().find_map(|id_map| id_map.get(ref_id)) {
            None => diags.push(diag_fatal(
                code,
                path,
                format!(
                    "definition-level seed references {kind} '{ref_id}', which no manifest row provides"
                ),
            )),
            Some(ref_bundle) => {
                if !index.bundle_scope.in_scope(owning_bundle, ref_bundle.as_str()) {
                    diags.push(diag_fatal(
                        MF_VAL_CROSS_BUNDLE_SCOPE,
                        path,
                        format!(
                            "definition-level seed references {kind} '{ref_id}' in bundle '{ref_bundle}', but owning bundle '{owning_bundle}' does not depend on it"
                        ),
                    ));
                }
            }
        }
    };

    for row in &manifest.entity_definition_initial_state {
        check(
            &row.bundle_id,
            &row.entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_ENTITY_REF,
            "entity_definition_initial_state.entity_def_id",
            "entity_definition",
            diags,
        );
        check(
            &row.bundle_id,
            &row.state_id,
            &[&index.object_state_definitions, &index.resource_definitions],
            MF_VAL_DEF_SEED_STATE_REF,
            "entity_definition_initial_state.state_id",
            "object_state_definition or resource_definition",
            diags,
        );
    }

    for row in &manifest.entity_definition_initial_capability {
        check(
            &row.bundle_id,
            &row.entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_ENTITY_REF,
            "entity_definition_initial_capability.entity_def_id",
            "entity_definition",
            diags,
        );
        check(
            &row.bundle_id,
            &row.capability_id,
            &[&index.capability_definitions],
            MF_VAL_DEF_SEED_CAPABILITY_REF,
            "entity_definition_initial_capability.capability_id",
            "capability_definition",
            diags,
        );
    }

    // sk-bwd: the IDENTITY→capability bridge rides the same
    // `MF_VAL_DEF_SEED_CAPABILITY_REF` check as the seed row above — an
    // unresolvable `capability_id` grants nothing at all, silently, which is
    // exactly the failure mode this class exists to make loud. Its `tag` leg
    // uses advisory `tag_value_definition` vocabulary and accepts any nonempty tag.
    for row in &manifest.tag_capability_grant {
        check(
            &row.bundle_id,
            &row.capability_id,
            &[&index.capability_definitions],
            MF_VAL_DEF_SEED_CAPABILITY_REF,
            "tag_capability_grant.capability_id",
            "capability_definition",
            diags,
        );
    }

    for row in &manifest.entity_definition_initial_condition {
        check(
            &row.bundle_id,
            &row.entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_ENTITY_REF,
            "entity_definition_initial_condition.entity_def_id",
            "entity_definition",
            diags,
        );
        check(
            &row.bundle_id,
            &row.condition_definition_id,
            &[&index.condition_definitions],
            MF_VAL_DEF_SEED_CONDITION_REF,
            "entity_definition_initial_condition.condition_definition_id",
            "condition_definition",
            diags,
        );
    }

    // sk-ct8.3 / finding #37 (BL-19), canon-warden fix #3: the equip-modifier
    // link row is NOT a spawn-time seed like the three siblings above (it is
    // read live, gated by CURRENT `entity_equipment_slot` membership — see
    // `entity_definition_equip_modifier.rs`'s own module doc), but it shares
    // the exact same "entity_def_id + one other FK + bundle_id" shape, so it
    // rides this same Class-11 check rather than earning a parallel one.
    for row in &manifest.entity_definition_equip_modifier {
        check(
            &row.bundle_id,
            &row.entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_ENTITY_REF,
            "entity_definition_equip_modifier.entity_def_id",
            "entity_definition",
            diags,
        );
        check(
            &row.bundle_id,
            &row.modifier_template_id,
            &[&index.modifier_templates],
            MF_VAL_DEF_SEED_MODIFIER_TEMPLATE_REF,
            "entity_definition_equip_modifier.modifier_template_id",
            "modifier_template",
            diags,
        );
    }

    // sk-ct8.3 / finding #37 (BL-19), canon-warden fix #3: the two
    // wear/equip-gating hook columns on `entity_definition` itself (Part A)
    // — same optional-FK shape as `on_consume_consequence_id`/
    // `on_destroy_consequence_id` (never validated before this slice; those
    // two remain out of scope here, unchanged behavior). Empty string means
    // "no hook" (the runtime's own convention,
    // `fire_equip_lifecycle_hook`) — only a non-empty value is checked.
    for row in &manifest.entity_definitions {
        if !row.on_equip_consequence_id.is_empty() {
            check(
                &row.bundle_id,
                &row.on_equip_consequence_id,
                &[&index.consequence_templates],
                MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF,
                "entity_definitions.on_equip_consequence_id",
                "consequence_template",
                diags,
            );
        }
        if !row.on_unequip_consequence_id.is_empty() {
            check(
                &row.bundle_id,
                &row.on_unequip_consequence_id,
                &[&index.consequence_templates],
                MF_VAL_ENTITY_DEFINITION_EQUIP_HOOK_REF,
                "entity_definitions.on_unequip_consequence_id",
                "consequence_template",
                diags,
            );
        }
    }

    // sk-84i.3 (parent sk-84i): the two new definition-seed satellites.
    for row in &manifest.entity_definition_initial_containment {
        check(
            &row.bundle_id,
            &row.entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_ENTITY_REF,
            "entity_definition_initial_containment.entity_def_id",
            "entity_definition",
            diags,
        );
        check(
            &row.bundle_id,
            &row.contained_entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_CONTAINMENT_REF,
            "entity_definition_initial_containment.contained_entity_def_id",
            "entity_definition",
            diags,
        );
    }

    for row in &manifest.entity_definition_initial_equipment {
        check(
            &row.bundle_id,
            &row.entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_ENTITY_REF,
            "entity_definition_initial_equipment.entity_def_id",
            "entity_definition",
            diags,
        );
        check(
            &row.bundle_id,
            &row.equipped_entity_def_id,
            &[&index.entity_definitions],
            MF_VAL_DEF_SEED_EQUIPMENT_ITEM_REF,
            "entity_definition_initial_equipment.equipped_entity_def_id",
            "entity_definition",
            diags,
        );
        check(
            &row.bundle_id,
            &row.slot_id,
            &[&index.equipment_slot_definitions],
            MF_VAL_DEF_SEED_EQUIPMENT_SLOT_REF,
            "entity_definition_initial_equipment.slot_id",
            "equipment_slot_definition",
            diags,
        );
    }
}

// ---------------------------------------------------------------------------
// Class 11b (sk-84i.3, parent sk-84i) — def-level containment/equipment
// duplicate-slot guard and containment-graph cycle check
// ---------------------------------------------------------------------------

/// At most one `entity_definition_initial_equipment` row per
/// `(entity_def_id, slot_id)`: two seed rows targeting the same slot on the
/// same definition would race for that slot at spawn time (the second insert
/// fails the runtime slot-exclusivity guard in
/// `definition_seed::apply_definition_seeds`) — catch it at authoring time
/// instead of at first spawn.
fn validate_definition_seed_equipment_slot_dup(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for row in &manifest.entity_definition_initial_equipment {
        let key = (row.entity_def_id.as_str(), row.slot_id.as_str());
        if !seen.insert(key) {
            diags.push(diag_fatal(
                MF_VAL_DEF_SEED_EQUIPMENT_SLOT_DUP,
                "entity_definition_initial_equipment.slot_id",
                format!(
                    "entity_definition '{}' authors more than one \
                     entity_definition_initial_equipment row for slot '{}' — at most one \
                     item may be seeded per slot",
                    row.entity_def_id, row.slot_id
                ),
            ));
        }
    }
}

/// Returns the incompatibility reason when the predicate refuses this slot.
pub fn equipment_seed_slot_incompatibility_reason(
    predicate: &str,
    slot_id: &str,
) -> Option<String> {
    crate::equip_predicate::incompatibility_reason(predicate, slot_id)
}

/// At manifest-commit time, catch a definition-level equipment seed whose
/// target item definition's `slot_compatibility_predicate` would reject the
/// seed's own `slot_id` — this seed would fail closed at spawn time
/// (`definition_seed::apply_definition_seeds`'s `check_slot_compatibility`
/// call, canon-warden finding A-5) with no manifest-commit-time signal
/// otherwise. Companion to `validate_definition_seed_equipment_slot_dup`.
fn validate_definition_seed_equipment_slot_compatibility(
    manifest: &Manifest,
    diags: &mut Vec<Diagnostic>,
) {
    let predicates: BTreeMap<&str, &str> = manifest
        .entity_definitions
        .iter()
        .map(|d| {
            (
                d.entity_def_id.as_str(),
                d.slot_compatibility_predicate.as_str(),
            )
        })
        .collect();
    for row in &manifest.entity_definition_initial_equipment {
        let Some(predicate) = predicates.get(row.equipped_entity_def_id.as_str()) else {
            // Missing-ref is already reported by MF-VAL-311; don't double-report.
            continue;
        };
        if let Some(reason) = equipment_seed_slot_incompatibility_reason(predicate, &row.slot_id) {
            diags.push(diag_fatal(
                MF_VAL_DEF_SEED_EQUIPMENT_SLOT_COMPATIBILITY,
                "entity_definition_initial_equipment.slot_id",
                format!(
                    "entity_definition_initial_equipment '{}': equipped_entity_def_id '{}' \
                     cannot be equipped into slot '{}' — {reason}",
                    row.entity_def_id, row.equipped_entity_def_id, row.slot_id
                ),
            ));
        }
    }
}

/// Rejects cycles across the combined definition-level containment and
/// equipment graph. Both kinds of seed can create child instances.
fn validate_definition_seed_containment_cycle(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let mut graph: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for row in &manifest.entity_definition_initial_containment {
        graph
            .entry(row.entity_def_id.as_str())
            .or_default()
            .push(row.contained_entity_def_id.as_str());
        graph
            .entry(row.contained_entity_def_id.as_str())
            .or_default();
    }
    for row in &manifest.entity_definition_initial_equipment {
        graph
            .entry(row.entity_def_id.as_str())
            .or_default()
            .push(row.equipped_entity_def_id.as_str());
        graph
            .entry(row.equipped_entity_def_id.as_str())
            .or_default();
    }
    let mut temp: BTreeSet<&str> = BTreeSet::new();
    let mut perm: BTreeSet<&str> = BTreeSet::new();
    for node in graph.keys().copied().collect::<Vec<_>>() {
        if let Some(cycle_path) = containment_cycle_from(node, &graph, &mut temp, &mut perm) {
            diags.push(diag_fatal(
                MF_VAL_DEF_SEED_CONTAINMENT_CYCLE,
                "entity_definition_initial_containment",
                format!(
                    "entity_definition_initial_containment/_equipment graph contains a \
                     cycle: {}",
                    cycle_path.join(" -> ")
                ),
            ));
            return;
        }
    }
}

/// Three-color DFS cycle detection (mirrors `visit_cycle` in `lib.rs`, which
/// is private to that module — this is a local copy over `&str` node ids
/// rather than reaching across modules for a five-line helper). Returns the
/// cycle path (node names, in traversal order, cycle-closing node repeated
/// at the end) when a cycle is found reachable from `node`.
fn containment_cycle_from<'a>(
    node: &'a str,
    graph: &BTreeMap<&'a str, Vec<&'a str>>,
    temp: &mut BTreeSet<&'a str>,
    perm: &mut BTreeSet<&'a str>,
) -> Option<Vec<&'a str>> {
    if perm.contains(node) {
        return None;
    }
    if !temp.insert(node) {
        return Some(vec![node]);
    }
    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if let Some(mut path) = containment_cycle_from(neighbor, graph, temp, perm) {
                path.insert(0, node);
                return Some(path);
            }
        }
    }
    temp.remove(node);
    perm.insert(node);
    None
}

// ---------------------------------------------------------------------------
// Catalog C6 — a seeded state value outside its resource_definition bounds
// ---------------------------------------------------------------------------

/// A `resource_definition` clamps its `object_state` to `[min_value, max_value]`
/// at runtime. A seed row (`entity_definition_initial_state` or instance
/// `object_state`) whose `state_id` names such a resource but whose `value_int`
/// is out of band is SILENTLY CLAMPED on commit — the authored number is not the
/// number that lands. Surface it (MF-VAL-269, Warning) so the author widens the
/// resource or fixes the seed instead of shipping a value that quietly vanishes.
fn validate_seed_resource_bounds(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let bounds: BTreeMap<&str, (i64, i64)> = manifest
        .resource_definitions
        .iter()
        .map(|r| {
            (
                r.resource_definition_id.as_str(),
                (r.min_value, r.max_value),
            )
        })
        .collect();

    let check =
        |state_id: &str, value: i64, owner: &str, path: &str, diags: &mut Vec<Diagnostic>| {
            if let Some(&(min, max)) = bounds.get(state_id) {
                if value < min || value > max {
                    diags.push(diag_fatal(
                        MF_VAL_SEED_RESOURCE_BOUNDS,
                        path,
                        format!(
                            "seeded value {value} for state '{state_id}' on '{owner}' is outside \
                         resource_definition '{state_id}' bounds [{min}, {max}] — the runtime \
                         silently clamps it, so the seeded value will not land. Author within \
                         bounds or widen the resource_definition."
                        ),
                    ));
                }
            }
        };

    for row in &manifest.entity_definition_initial_state {
        if let Some(v) = row.value_int {
            check(
                &row.state_id,
                v,
                &row.entity_def_id,
                "entity_definition_initial_state.value_int",
                diags,
            );
        }
    }
    for row in &manifest.object_state {
        if let Some(v) = row.value_int {
            check(
                &row.state_id,
                v,
                &row.entity_key,
                "object_state.value_int",
                diags,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Catalog R6 — a duplicate (entity, state) seed silently last-write-wins
// ---------------------------------------------------------------------------

/// Two seed rows for the same `(entity, state)` collapse at commit: the later
/// row wins and the earlier value is lost with no diagnostic. Detect both
/// carriers — `entity_definition_initial_state` keyed `(entity_def_id, state_id)`
/// and instance `object_state` keyed `(entity_key, state_id)` — bundle-agnostic
/// (a cross-include re-seed is exactly the footgun). MF-VAL-270, Warning: the
/// write-loss is silent but a redundant identical re-seed is harmless, so this
/// surfaces rather than rejects (ratchet to Fatal once the corpus is clean).
fn validate_duplicate_state_seed(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    let mut seen_def: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (idx, row) in manifest.entity_definition_initial_state.iter().enumerate() {
        let key = (row.entity_def_id.as_str(), row.state_id.as_str());
        if let Some(&first) = seen_def.get(&key) {
            diags.push(diag_fatal(
                MF_VAL_DUPLICATE_STATE_SEED,
                "entity_definition_initial_state",
                format!(
                    "duplicate entity_definition_initial_state seed (entity_def_id='{}', \
                     state_id='{}') at rows [{}, {}] — the later row silently last-write-wins; \
                     the earlier value is lost. Author one seed per (definition, state).",
                    row.entity_def_id, row.state_id, first, idx
                ),
            ));
        } else {
            seen_def.insert(key, idx);
        }
    }

    let mut seen_obj: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (idx, row) in manifest.object_state.iter().enumerate() {
        let key = (row.entity_key.as_str(), row.state_id.as_str());
        if let Some(&first) = seen_obj.get(&key) {
            diags.push(diag_fatal(
                MF_VAL_DUPLICATE_STATE_SEED,
                "object_state",
                format!(
                    "duplicate object_state seed (entity_key='{}', state_id='{}') at rows \
                     [{}, {}] — the later row silently last-write-wins; the earlier value is \
                     lost. Author one seed per (entity, state).",
                    row.entity_key, row.state_id, first, idx
                ),
            ));
        } else {
            seen_obj.insert(key, idx);
        }
    }
}

// ---------------------------------------------------------------------------
// Catalog R3 — cross-file duplicate id (generated vs. hand-authored siblings)
// ---------------------------------------------------------------------------

/// Flag a duplicate id within one row vector's id namespace. Scans the raw
/// `Vec` (not a dedup-by-construction `BTreeMap` index like `RefIndex`), the
/// same technique `validate_required_capability_refs` already uses for
/// `capability_id` (MF-VAL-266) — generalised to every other globally-unique
/// definitional id column. See `MF_VAL_DUPLICATE_ROW_ID`.
fn scan_duplicate_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    row_kind: &str,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            diags.push(diag_fatal(
                MF_VAL_DUPLICATE_ROW_ID,
                path,
                format!(
                    "duplicate {row_kind} id '{id}' — a generator driver and a hand-authored \
                     sibling (or two generator passes) both emit this id; the runtime rejects it \
                     as a duplicate unique column at apply time (MF-APPLY-020). Fix: set \
                     `emitCapability:false` (or the generator's equivalent id-skip flag) on the \
                     driver entry for every id a hand-authored fixture already owns, or rename one of the two \
                     emitters so the ids no longer collide."
                ),
            ));
        }
    }
}

fn validate_duplicate_row_ids(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    scan_duplicate_ids(
        manifest
            .entity_definitions
            .iter()
            .map(|r| r.entity_def_id.as_str()),
        "entity_definition",
        "entity_definitions.entity_def_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .action_definitions
            .iter()
            .map(|r| r.action_id.as_str()),
        "action_definition",
        "action_definitions.action_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .roll_tables
            .iter()
            .map(|r| r.roll_table_id.as_str()),
        "roll_table",
        "roll_tables.roll_table_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .outcome_templates
            .iter()
            .map(|r| r.outcome_template_id.as_str()),
        "outcome_template",
        "outcome_templates.outcome_template_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .resource_definitions
            .iter()
            .map(|r| r.resource_definition_id.as_str()),
        "resource_definition",
        "resource_definitions.resource_definition_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .condition_definitions
            .iter()
            .map(|r| r.condition_definition_id.as_str()),
        "condition_definition",
        "condition_definitions.condition_definition_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .stacking_discipline_definitions
            .iter()
            .map(|r| r.discipline_id.as_str()),
        "stacking_discipline_definition",
        "stacking_discipline_definitions.discipline_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .condition_remove_policy_definitions
            .iter()
            .map(|r| r.policy_id.as_str()),
        "condition_remove_policy_definition",
        "condition_remove_policy_definitions.policy_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .object_state_definitions
            .iter()
            .map(|r| r.state_id.as_str()),
        "object_state_definition",
        "object_state_definitions.state_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .equipment_slot_definitions
            .iter()
            .map(|r| r.slot_id.as_str()),
        "equipment_slot_definition",
        "equipment_slot_definitions.slot_id",
        diags,
    );
    scan_duplicate_ids(
        manifest
            .affordance_templates
            .iter()
            .map(|r| r.template_id.as_str()),
        "affordance_template",
        "affordance_templates.template_id",
        diags,
    );
    scan_duplicate_ids(
        manifest.entities.iter().map(|r| r.entity_key.as_str()),
        "entity",
        "entities.entity_key",
        diags,
    );
}

/// True when `id` lives under `bundle_id`'s namespace: an exact match, or
/// `id` starts with `"<bundle_id>/"`. `bundle_id` may itself carry a `/`
/// namespace separator (e.g. `tag_authoring/homebrew`, C2/MF-VAL-271), which
/// this handles for free since it compares the literal prefix rather than
/// just the first path segment.
fn id_in_bundle_namespace(id: &str, bundle_id: &str) -> bool {
    id == bundle_id || id.starts_with(&format!("{bundle_id}/"))
}

/// Scan one definitional id column for foreign-namespace defines. Skips rows
/// with an empty `id` or `bundle_id` — an empty value means some other,
/// more specific lint (a required-field check, or MF-VAL-101 unknown-bundle)
/// already owns flagging that row; this rule only judges a *populated*
/// id/bundle_id pair.
fn scan_id_namespace(
    rows: impl Iterator<Item = (String, String)>,
    row_kind: &str,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    for (id, bundle_id) in rows {
        if id.is_empty() || bundle_id.is_empty() {
            continue;
        }
        if !id_in_bundle_namespace(&id, &bundle_id) {
            diags.push(diag_warning(
                MF_VAL_ID_BUNDLE_NAMESPACE,
                path,
                format!(
                    "{row_kind} id '{id}' is defined in bundle '{bundle_id}' but does not live \
                     under that bundle's namespace (expected '{bundle_id}' or \
                     '{bundle_id}/...'). A bundle should only DEFINE ids in its own namespace — \
                     consuming another bundle's id (a reference) is fine and unrelated to this \
                     rule, but a foreign-namespace *definition* can silently collide the moment \
                     two bundles are loaded together (the #1510 `core/poisoned`+`core/cursed`, \
                     `demos/*`, and cross-demo `place_graph`/`entity_key` collisions). Rename the \
                     id into '{bundle_id}/...'."
                ),
            ));
        }
    }
}

/// Class 14 / catalog row **S3** (generalised) — every definitional id
/// column authors generate content for (the same universe MF-VAL-272 tracks
/// for cross-bundle duplicate detection, plus `place_graphs.place_graph_id`,
/// which #1510's sibling PR renames for exactly this reason) must live under
/// its own row's `bundle_id` namespace. See `MF_VAL_ID_BUNDLE_NAMESPACE`.
fn validate_id_bundle_namespace(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    scan_id_namespace(
        manifest
            .role_definitions
            .iter()
            .filter(|r| !roles::STANDARD_ROLE_IDS.contains(&r.role_id.as_str()))
            .map(|r| (r.role_id.clone(), r.bundle_id.clone())),
        "role_definition",
        "role_definitions.role_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .entity_definitions
            .iter()
            .map(|r| (r.entity_def_id.clone(), r.bundle_id.clone())),
        "entity_definition",
        "entity_definitions.entity_def_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .action_definitions
            .iter()
            .map(|r| (r.action_id.clone(), r.bundle_id.clone())),
        "action_definition",
        "action_definitions.action_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .roll_tables
            .iter()
            .map(|r| (r.roll_table_id.clone(), r.bundle_id.clone())),
        "roll_table",
        "roll_tables.roll_table_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .outcome_templates
            .iter()
            .map(|r| (r.outcome_template_id.clone(), r.bundle_id.clone())),
        "outcome_template",
        "outcome_templates.outcome_template_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .resource_definitions
            .iter()
            .map(|r| (r.resource_definition_id.clone(), r.bundle_id.clone())),
        "resource_definition",
        "resource_definitions.resource_definition_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .condition_definitions
            .iter()
            .map(|r| (r.condition_definition_id.clone(), r.bundle_id.clone())),
        "condition_definition",
        "condition_definitions.condition_definition_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .stacking_discipline_definitions
            .iter()
            .map(|r| (r.discipline_id.clone(), r.bundle_id.clone())),
        "stacking_discipline_definition",
        "stacking_discipline_definitions.discipline_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .condition_remove_policy_definitions
            .iter()
            .map(|r| (r.policy_id.clone(), r.bundle_id.clone())),
        "condition_remove_policy_definition",
        "condition_remove_policy_definitions.policy_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .object_state_definitions
            .iter()
            .map(|r| (r.state_id.clone(), r.bundle_id.clone())),
        "object_state_definition",
        "object_state_definitions.state_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .equipment_slot_definitions
            .iter()
            .map(|r| (r.slot_id.clone(), r.bundle_id.clone())),
        "equipment_slot_definition",
        "equipment_slot_definitions.slot_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .affordance_templates
            .iter()
            .map(|r| (r.template_id.clone(), r.bundle_id.clone())),
        "affordance_template",
        "affordance_templates.template_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .capability_definitions
            .iter()
            .map(|r| (r.capability_id.clone(), r.bundle_id.clone())),
        "capability_definition",
        "capability_definitions.capability_id",
        diags,
    );
    scan_id_namespace(
        manifest
            .entities
            .iter()
            .map(|r| (r.entity_key.clone(), r.bundle_id.clone())),
        "entity",
        "entities.entity_key",
        diags,
    );
    scan_id_namespace(
        manifest
            .place_graphs
            .iter()
            .map(|r| (r.place_graph_id.clone(), r.bundle_id.clone())),
        "place_graph",
        "place_graphs.place_graph_id",
        diags,
    );
}

// ---------------------------------------------------------------------------
// sk-2wg / MF-VAL-328 — place-provided niche ⇒ spatial_niche_attachments
// ---------------------------------------------------------------------------

/// Warn when a location-dependent (place-provided) niche used by a
/// manipulate affordance has no `spatial_niche_attachments` row in the
/// assembled Manifest. See [`MF_VAL_PLACE_NICHE_ATTACHMENT`].
fn validate_place_niche_attachment_coverage(manifest: &Manifest, diags: &mut Vec<Diagnostic>) {
    // Reusable / definitions-only bundles ship niche-gated actions expecting
    // a downstream scene to author the attachment. No place_nodes ⇒ no scene.
    if manifest.place_nodes.is_empty() {
        return;
    }

    let attached: BTreeSet<&str> = manifest
        .spatial_niche_attachments
        .iter()
        .map(|row| row.niche_id.as_str())
        .collect();
    let provider_by_niche: BTreeMap<&str, &str> = manifest
        .niche_definitions
        .iter()
        .map(|row| (row.niche_id.as_str(), row.provider.as_str()))
        .collect();
    let outcomes_by_id: BTreeMap<&str, &spellkave_schema::OutcomeTemplateRow> = manifest
        .outcome_templates
        .iter()
        .map(|row| (row.outcome_template_id.as_str(), row))
        .collect();
    let mut traversal_templates: BTreeSet<&str> = BTreeSet::new();
    for row in &manifest.place_edge_affordance {
        traversal_templates.insert(row.affordance_template_id.as_str());
    }
    for row in &manifest.movement_defaults {
        traversal_templates.insert(row.affordance_template_id.as_str());
    }

    let is_traversal = |aff: &spellkave_schema::AffordanceTemplateRow| -> bool {
        if traversal_templates.contains(aff.template_id.as_str()) {
            return true;
        }
        outcomes_by_id
            .get(aff.outcome_template_id.as_str())
            .is_some_and(|outcome| !outcome.movement_descriptor.trim().is_empty())
    };

    let requires_spatial_attachment = |niche_id: &str| -> bool {
        match provider_by_niche.get(niche_id) {
            Some(provider) => {
                let provider = provider.trim();
                provider.is_empty() || provider == "place"
            }
            // Fail-closed place default — the same fallback Gate B uses when
            // no niche_definitions row exists (sk-84i.1 strikeable-without-def).
            None => true,
        }
    };

    let mut warned: BTreeSet<&str> = BTreeSet::new();
    for aff in &manifest.affordance_templates {
        let niche = aff.niche_id.trim();
        if niche.is_empty() || is_traversal(aff) {
            continue;
        }
        if !requires_spatial_attachment(niche) || attached.contains(niche) {
            continue;
        }
        if !warned.insert(niche) {
            continue;
        }
        diags.push(Diagnostic {
            code: MF_VAL_PLACE_NICHE_ATTACHMENT.to_owned(),
            severity: Severity::Warning,
            message: format!(
                "place-provided niche '{niche}' is used by affordance_template '{}' but the assembled manifest has no spatial_niche_attachments row for it; r_manipulate fails closed at attachment_presence before resolution (sk-84i.1). Author an attachment on a place_node, or set niche_definitions.provider to actor/entity if this verb should not be node-scoped.",
                aff.template_id
            ),
            path: "affordance_templates.niche_id".to_owned(),
            span: None,
            hint: Some(
                "a reusable bundle with no place_nodes is skipped; this warning fires on the composed scene (or an inline applyManifestSource test) that forgot the attachment"
                    .to_owned(),
            ),
            details: Some(serde_json::json!({
                "niche_id": niche,
                "template_id": aff.template_id,
            })),
        });
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn diag_fatal(code: &str, path: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Fatal,
        message,
        path: path.to_owned(),
        span: None,
        hint: None,
        details: None,
    }
}

fn diag_warning(code: &str, path: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity: Severity::Warning,
        message,
        path: path.to_owned(),
        span: None,
        hint: None,
        details: None,
    }
}

#[cfg(test)]
mod outcome_graph_tests {
    use super::*;
    use spellkave_schema::{
        ActionDefinitionRow, ActorEquipmentSlotRow, BundleRow, ConsequenceFrameWriteRow,
        EntityDefinitionInitialContainmentRow, EntityDefinitionInitialEquipmentRow,
        EntityDefinitionInitialStateRow, EntityDefinitionRow, EntityKindRow, EntityRow,
        EquipmentSlotDefinitionRow, Manifest, ObjectStateDefinitionRow, ObjectStateRow,
        OutcomeGraphEdgeRow, OutcomeGraphNodeRow, OutcomeGraphRow, PlaceGraphRow,
        ResourceDefinitionRow,
    };

    fn graph_manifest() -> Manifest {
        let mut manifest = Manifest::default();
        manifest.outcome_graphs.push(OutcomeGraphRow {
            outcome_graph_id: "test/graph".into(),
            bundle_id: "test".into(),
            display_name: "Graph".into(),
            root_node_id: "root".into(),
            max_depth: 8,
            max_nodes: 8,
            max_draws: 4,
            description: String::new(),
        });
        manifest.outcome_graph_nodes.push(OutcomeGraphNodeRow {
            outcome_graph_node_id: "test/graph:root".into(),
            outcome_graph_id: "test/graph".into(),
            node_id: "root".into(),
            outcome_template_id: String::new(),
            terminal: true,
            authored_order: 0,
        });
        manifest
    }

    fn codes(manifest: &Manifest) -> Vec<String> {
        let mut diags = Vec::new();
        validate_cross_row_refs(manifest, None, &mut diags);
        diags.into_iter().map(|diag| diag.code).collect()
    }

    fn diags_for(manifest: &Manifest) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        validate_cross_row_refs(manifest, None, &mut diags);
        diags
    }

    fn bounded_resource(id: &str, min: i64, max: i64) -> ResourceDefinitionRow {
        ResourceDefinitionRow {
            resource_definition_id: id.into(),
            bundle_id: "test".into(),
            display_name: id.into(),
            value_kind: "int".into(),
            min_value: min,
            max_value: max,
            recovery_rule: "none".into(),
            initial_value: None,
            description: String::new(),
        }
    }

    fn def_seed(
        entity_def_id: &str,
        state_id: &str,
        value: i64,
    ) -> EntityDefinitionInitialStateRow {
        EntityDefinitionInitialStateRow {
            entity_def_id: entity_def_id.into(),
            state_id: state_id.into(),
            value_bool: true,
            value_int: Some(value),
            value_text: None,
            bundle_id: "test".into(),
        }
    }

    fn object_state_def(id: &str) -> ObjectStateDefinitionRow {
        ObjectStateDefinitionRow {
            state_id: id.into(),
            bundle_id: "test".into(),
            target_kind: "entity".into(),
            display_name: id.into(),
            default_visibility: "public".into(),
            description: String::new(),
        }
    }

    fn obj_state(entity_key: &str, state_id: &str, value: i64) -> ObjectStateRow {
        ObjectStateRow {
            entity_key: entity_key.into(),
            state_id: state_id.into(),
            visibility: Some("public".into()),
            value_bool: true,
            value_int: Some(value),
            value_text: None,
            source_template_id: String::new(),
            bundle_id: "test".into(),
            expires_at_tick: 0,
        }
    }

    // --- C6 / MF-VAL-269: seed outside resource bounds ----------------------

    /// Regression fixture for catalog row **C6**: ratcheted Warning → Fatal
    /// (#1359) once the corpus swept clean.
    #[test]
    fn seed_above_resource_max_is_fatal_269() {
        let mut m = Manifest::default();
        m.resource_definitions
            .push(bounded_resource("srd/attitude", -1, 1));
        m.entity_definition_initial_state
            .push(def_seed("test/npc", "srd/attitude", 5)); // 5 > max 1
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_SEED_RESOURCE_BOUNDS)
            .expect("MF-VAL-269 must fire for an out-of-bounds seed");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn seed_within_resource_bounds_clean_269() {
        let mut m = Manifest::default();
        m.resource_definitions
            .push(bounded_resource("srd/attitude", -1, 1));
        m.entity_definition_initial_state
            .push(def_seed("test/npc", "srd/attitude", 1)); // 1 == max, in band
        m.object_state
            .push(obj_state("test/npc", "srd/attitude", -1)); // == min
        assert!(!codes(&m).contains(&MF_VAL_SEED_RESOURCE_BOUNDS.to_string()));
    }

    #[test]
    fn seed_for_unbounded_state_is_ignored_269() {
        // No resource_definition for the state → nothing clamps → no warning.
        let mut m = Manifest::default();
        m.entity_definition_initial_state
            .push(def_seed("test/npc", "test/freeform", 9999));
        assert!(!codes(&m).contains(&MF_VAL_SEED_RESOURCE_BOUNDS.to_string()));
    }

    // --- Class 11 / MF-VAL-253: definition seed state_id catalog union ------
    // A definition-level seed’s `state_id` must resolve against
    // `object_state_definitions` OR `resource_definitions` — both are real,
    // authored catalogs the runtime materializer accepts identically.

    #[test]
    fn definition_seed_resolves_against_resource_definition_253() {
        let mut m = Manifest::default();
        m.resource_definitions
            .push(bounded_resource("srd/str", 1, 20));
        m.entity_definition_initial_state
            .push(def_seed("srd/goblin", "srd/str", 8));
        assert!(!codes(&m).contains(&MF_VAL_DEF_SEED_STATE_REF.to_string()));
    }

    #[test]
    fn definition_seed_resolves_against_object_state_definition_253() {
        let mut m = Manifest::default();
        m.object_state_definitions.push(object_state_def("srd/hp"));
        m.entity_definition_initial_state
            .push(def_seed("srd/bat", "srd/hp", 5));
        assert!(!codes(&m).contains(&MF_VAL_DEF_SEED_STATE_REF.to_string()));
    }

    #[test]
    fn definition_seed_missing_from_both_catalogs_is_fatal_253() {
        let mut m = Manifest::default();
        m.entity_definition_initial_state
            .push(def_seed("srd/goblin", "srd/nonexistent", 8));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_DEF_SEED_STATE_REF)
            .expect("MF-VAL-253 must still fire when state_id resolves against neither catalog");
        assert_eq!(hit.severity, Severity::Fatal);
        assert!(
            hit.message
                .contains("object_state_definition or resource_definition"),
            "message should name both catalogs, got: {}",
            hit.message
        );
    }

    #[test]
    fn definition_seed_resource_out_of_bounds_trips_269_not_253() {
        // `validate_seed_resource_bounds` (269) was never gated by 253 — both
        // are called unconditionally and independently from
        // `validate_cross_row_refs`, so before this fix a resource-typed
        // definition seed got BOTH diagnostics and was rejected on 253 alone
        // regardless of 269's verdict. This proves the actual effect of the
        // widening: 253 no longer fires, so 269 is now the sole SURVIVING
        // diagnostic — the row is admitted rather than unconditionally
        // rejected, and its bounds violation (if any) is what decides its
        // fate.
        let mut m = Manifest::default();
        m.resource_definitions
            .push(bounded_resource("srd/str", 1, 20));
        m.entity_definition_initial_state
            .push(def_seed("srd/goblin", "srd/str", 99)); // 99 > max 20
        let diags = diags_for(&m);
        assert!(
            !diags.iter().any(|d| d.code == MF_VAL_DEF_SEED_STATE_REF),
            "253 must not fire — the id resolves via resource_definitions"
        );
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_SEED_RESOURCE_BOUNDS)
            .expect("MF-VAL-269 must fire for an out-of-bounds resource-typed definition seed");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn definition_seed_state_id_in_both_catalogs_same_bundle_resolves_clean_253() {
        // Order-dependence note (canon-warden review, 2026-07-27):
        // `check`'s union resolves via `id_maps.iter().find_map(...)`, which
        // returns the FIRST catalog hit — `object_state_definitions` before
        // `resource_definitions`. So an id authored in BOTH catalogs across
        // DIFFERENT bundles would have its MF-VAL-214 cross-bundle-scope
        // verdict decided by whichever bundle holds the
        // `object_state_definitions` row, not necessarily whichever bundle
        // is in scope. This case covers an id defined in both catalogs by one bundle.
        let mut m = Manifest::default();
        m.bundles.push(BundleRow {
            bundle_id: "owner".into(),
            display_name: "owner".into(),
            author: "test".into(),
            version: "1.0.0".into(),
            depends_on: vec!["vocab".into()],
            description: String::new(),
            tag_prefixes: Vec::new(),
            allow_flat_tags: false,
            default_extent_shape_id: String::new(),
            default_extent_params_json: String::new(),
        });
        let mut os_def = object_state_def("srd/str");
        os_def.bundle_id = "vocab".into();
        m.object_state_definitions.push(os_def);
        let mut res_def = bounded_resource("srd/str", 1, 20);
        res_def.bundle_id = "vocab".into();
        m.resource_definitions.push(res_def);
        let mut seed = def_seed("owner/npc", "srd/str", 10);
        seed.bundle_id = "owner".into();
        m.entity_definition_initial_state.push(seed);

        let diags = diags_for(&m);
        assert!(
            !diags.iter().any(|d| d.code == MF_VAL_CROSS_BUNDLE_SCOPE),
            "the referenced bundle 'vocab' is in scope via depends_on, regardless of which \
             catalog resolves first: {diags:?}"
        );
        assert!(!diags.iter().any(|d| d.code == MF_VAL_DEF_SEED_STATE_REF));
    }

    // --- R6 / MF-VAL-270: duplicate (entity, state) seed --------------------

    /// Regression fixture for catalog row **R6**: the shipped Bat HP `5` →
    /// `30` clobber.
    /// Ratcheted Warning → Fatal (#1359) once the corpus swept clean.
    #[test]
    fn duplicate_definition_seed_is_fatal_270() {
        let mut m = Manifest::default();
        m.entity_definition_initial_state
            .push(def_seed("srd/bat", "srd/hp", 5));
        m.entity_definition_initial_state
            .push(def_seed("srd/bat", "srd/hp", 30)); // clobbers the 5
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_DUPLICATE_STATE_SEED)
            .expect("MF-VAL-270 must fire for a duplicate definition-seed");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn duplicate_object_state_seed_is_fatal_270() {
        let mut m = Manifest::default();
        m.object_state
            .push(obj_state("phandelver/klarg", "srd/hp", 7));
        m.object_state
            .push(obj_state("phandelver/klarg", "srd/hp", 99));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_DUPLICATE_STATE_SEED)
            .expect("MF-VAL-270 must fire for a duplicate object_state seed");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn distinct_seeds_clean_270() {
        let mut m = Manifest::default();
        m.entity_definition_initial_state
            .push(def_seed("srd/bat", "srd/hp", 5));
        m.entity_definition_initial_state
            .push(def_seed("srd/bat", "srd/ac", 12)); // different state
        m.object_state.push(obj_state("a", "srd/hp", 1));
        m.object_state.push(obj_state("b", "srd/hp", 1)); // different entity
        assert!(!codes(&m).contains(&MF_VAL_DUPLICATE_STATE_SEED.to_string()));
    }

    // --- C2 / MF-VAL-271: bundle_id snake_case ------------------------------

    fn bundle(id: &str) -> BundleRow {
        BundleRow {
            bundle_id: id.into(),
            display_name: id.into(),
            author: "test".into(),
            version: "1.0.0".into(),
            depends_on: vec![],
            description: String::new(),
            tag_prefixes: Vec::new(),
            allow_flat_tags: false,
            default_extent_shape_id: String::new(),
            default_extent_params_json: String::new(),
        }
    }

    #[test]
    fn open_session_definition_obeys_dependency_closure_and_strict_live_admission() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("test"));
        m.bundles.push(bundle("foreign"));
        m.session_definitions.push(
            serde_json::from_value(serde_json::json!({
                "session_def_id": "foreign/session", "bundle_id": "foreign"
            }))
            .unwrap(),
        );
        m.consequence_templates.push(
            serde_json::from_value(serde_json::json!({
                "template_id": "test/open", "bundle_id": "test"
            }))
            .unwrap(),
        );
        m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
            template_id: "test/open".into(),
            ordinal: 0,
            frame_write: serde_json::json!({"OpenSession":{"session_def_id":"foreign/session"}}),
        });
        assert!(codes(&m).contains(&MF_VAL_CROSS_BUNDLE_SCOPE.to_string()));
        m.bundles[0].depends_on.push("foreign".into());
        assert!(!codes(&m).contains(&MF_VAL_CROSS_BUNDLE_SCOPE.to_string()));
        m.session_definitions.clear();
        let mut diags = Vec::new();
        validate_cross_row_refs(&m, Some(&LiveVocabulary::default()), &mut diags);
        assert!(diags
            .iter()
            .any(|d| d.code == MF_VAL_FK_CLOSURE && d.severity == Severity::Fatal));
        for malformed in [
            serde_json::json!({}),
            serde_json::json!({"session_def_id":""}),
            serde_json::json!({"session_def_id":7}),
        ] {
            m.consequence_frame_writes[0].frame_write =
                serde_json::json!({"OpenSession":malformed});
            assert!(diags_for(&m)
                .iter()
                .any(|d| d.code == MF_VAL_FK_CLOSURE && d.severity == Severity::Fatal));
        }
    }

    #[test]
    fn hyphenated_bundle_id_warns_271() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("green-herald"));
        assert!(codes(&m).contains(&MF_VAL_BUNDLE_ID_NAMING.to_string()));
    }

    #[test]
    fn snake_case_bundle_id_clean_271() {
        let mut m = Manifest::default();
        m.bundles.push(bundle("green_herald"));
        m.bundles.push(bundle("deferred_external_resolution"));
        m.bundles.push(bundle("srd"));
        m.bundles.push(bundle("tag_authoring/homebrew")); // namespaced — allowed
        assert!(!codes(&m).contains(&MF_VAL_BUNDLE_ID_NAMING.to_string()));
    }

    // --- R3 / MF-VAL-272: cross-file duplicate id ---------------------------

    fn entity_definition(entity_def_id: &str) -> EntityDefinitionRow {
        EntityDefinitionRow {
            entity_def_id: entity_def_id.into(),
            bundle_id: "test".into(),
            display_name: entity_def_id.into(),
            lineage_id: String::new(),
            affordance_seed_id: String::new(),
            on_consume_consequence_id: String::new(),
            on_destroy_consequence_id: String::new(),
            on_equip_consequence_id: String::new(),
            on_unequip_consequence_id: String::new(),
            grants_capability_while_equipped: String::new(),
            grants_capability_while_present_in_location: String::new(),
            slot_compatibility_predicate: String::new(),
            stackable: false,
            description: String::new(),
        }
    }

    fn entity(entity_key: &str) -> EntityRow {
        EntityRow {
            entity_key: entity_key.into(),
            bundle_id: "test".into(),
            entity_def_id: String::new(),
            name: entity_key.into(),
            kind: EntityKindRow::Npc,
            place_node_id: String::new(),
            quantity: 1,
            owned_by_entity_key: String::new(),
            description: String::new(),
        }
    }

    /// Regression fixture for catalog row **R3**: a generator driver and a
    /// hand-authored sibling emit the same `entity_def_id` — file-clean on
    /// their own, only a duplicate once `--bundle --dep` merges both into one
    /// Manifest (the acid_splash pattern's failure mode, generalised beyond
    /// `capability_id`/MF-VAL-266).
    #[test]
    fn duplicate_entity_definition_id_is_fatal_272() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("srd/goblin"));
        m.entity_definitions.push(entity_definition("srd/goblin")); // generator re-emits the hand-authored id
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_DUPLICATE_ROW_ID)
            .expect("MF-VAL-272 must fire for a duplicate entity_def_id");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    /// Same failure mode on an authored `[[entities]]` instance row (the more
    /// common generator/hand-authored collision than a definition id).
    #[test]
    fn duplicate_entity_key_is_fatal_272() {
        let mut m = Manifest::default();
        m.entities.push(entity("phandelver/klarg"));
        m.entities.push(entity("phandelver/klarg"));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_DUPLICATE_ROW_ID)
            .expect("MF-VAL-272 must fire for a duplicate entity_key");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn duplicate_action_id_is_fatal_272() {
        let mut m = Manifest::default();
        let action = |on_success: &str| ActionDefinitionRow {
            action_id: "srd/attack/melee".into(),
            bundle_id: "test".into(),
            required_capability: String::new(),
            on_success: on_success.into(),
            on_failure: String::new(),
            resolution_mode: Default::default(),
            resolution_predicate_source: String::new(),
            resolution_scope: String::new(),
            non_steering: false,
            display_name: "Melee Attack".into(),
            cost_time_ms: 0,
            session_def_id: String::new(),
            resumable: false,
            reaction_eligibility: None,
            reaction_window_ms: None,
            reaction_trigger: None,
            self_eligible: false,
            self_only: false,
            description: String::new(),
            spends_channel: None,
        };
        m.action_definitions.push(action("outcome/a"));
        m.action_definitions.push(action("outcome/b"));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_DUPLICATE_ROW_ID)
            .expect("MF-VAL-272 must fire for a duplicate action_id");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn distinct_ids_clean_272() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("srd/goblin"));
        m.entity_definitions.push(entity_definition("srd/bat"));
        m.entities.push(entity("phandelver/klarg"));
        m.entities.push(entity("phandelver/yeemik"));
        assert!(!codes(&m).contains(&MF_VAL_DUPLICATE_ROW_ID.to_string()));
    }

    // --- MF-VAL-263: a self-eligible reaction needs a depleting budget
    // --- resource (ADR-0050 self-reaction-resolution-flip) -------------

    fn self_eligible_reaction(on_success: &str) -> ActionDefinitionRow {
        ActionDefinitionRow {
            action_id: "srd/legendary_resistance".into(),
            bundle_id: "test".into(),
            required_capability: String::new(),
            on_success: on_success.into(),
            on_failure: String::new(),
            resolution_mode: Default::default(),
            resolution_predicate_source: String::new(),
            resolution_scope: String::new(),
            non_steering: false,
            display_name: "Legendary Resistance".into(),
            cost_time_ms: 0,
            session_def_id: String::new(),
            resumable: false,
            reaction_eligibility: Some("core/reaction/on_failed_save".into()),
            reaction_window_ms: None,
            reaction_trigger: None,
            self_eligible: true,
            self_only: false,
            description: String::new(),
            spends_channel: None,
        }
    }

    #[test]
    fn self_eligible_reaction_without_budget_resource_is_fatal_263() {
        let mut m = Manifest::default();
        m.action_definitions
            .push(self_eligible_reaction("outcome/legendary_resistance"));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_SELF_REACTION_RESOURCE_REF)
            .expect("MF-VAL-263 must fire for a self-eligible reaction with no budget resource");
        assert_eq!(hit.severity, Severity::Fatal);
    }

    #[test]
    fn self_eligible_reaction_with_resolvable_budget_resource_is_clean_263() {
        let mut m = Manifest::default();
        m.resource_definitions
            .push(bounded_resource("test/uses", 0, 3));
        m.action_definitions
            .push(self_eligible_reaction("outcome/legendary_resistance"));
        m.consequence_frame_writes.push(ConsequenceFrameWriteRow {
            template_id: "outcome/legendary_resistance".into(),
            ordinal: 0,
            frame_write: serde_json::json!({ "ModifyResource": { "resource": "test/uses" } }),
        });
        let diags = diags_for(&m);
        assert!(
            !diags.iter().any(|d| d.code == MF_VAL_SELF_REACTION_RESOURCE_REF),
            "a self-eligible reaction with a resolvable budget resource must not fire MF-VAL-263: {diags:?}"
        );
    }

    // --- S3 (generalised) / MF-VAL-291: id defined outside its own bundle's
    // --- namespace ------------------------------------------------------

    fn place_graph(place_graph_id: &str, bundle_id: &str) -> PlaceGraphRow {
        PlaceGraphRow {
            place_graph_id: place_graph_id.into(),
            bundle_id: bundle_id.into(),
            substrate_id: "test_substrate".into(),
            display_name: place_graph_id.into(),
            origin_cell_res10: 0,
            origin_cell_res7: 0,
            origin_cell_res4: 0,
            description: String::new(),
            bearing: 0.0,
            metres_per_unit: 1.0,
            blueprint_id: String::new(),
            parent_graph_id: String::new(),
            anchor_node_id: String::new(),
            is_ephemeral_instance: false,
        }
    }

    /// Regression fixture mirroring the real #1510 corpus finding: a
    /// definitional row whose id squats a wholly foreign namespace
    /// (`srd/hp` defined by a bundle that is not `srd`).
    #[test]
    fn entity_definition_outside_own_namespace_warns_291() {
        let mut m = Manifest::default();
        m.entity_definitions.push(EntityDefinitionRow {
            entity_def_id: "srd/hp".into(),
            bundle_id: "build_fixture".into(),
            display_name: "Squatter".into(),
            lineage_id: String::new(),
            affordance_seed_id: String::new(),
            on_consume_consequence_id: String::new(),
            on_destroy_consequence_id: String::new(),
            on_equip_consequence_id: String::new(),
            on_unequip_consequence_id: String::new(),
            grants_capability_while_equipped: String::new(),
            grants_capability_while_present_in_location: String::new(),
            slot_compatibility_predicate: String::new(),
            stackable: false,
            description: String::new(),
        });
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_ID_BUNDLE_NAMESPACE)
            .expect("MF-VAL-291 must fire for a foreign-namespace entity_def_id");
        assert_eq!(hit.severity, Severity::Warning);
    }

    #[test]
    fn entity_definition_in_own_namespace_clean_291() {
        let mut m = Manifest::default();
        m.entity_definitions.push(EntityDefinitionRow {
            entity_def_id: "build_fixture/hp_track".into(),
            bundle_id: "build_fixture".into(),
            display_name: "Owned".into(),
            lineage_id: String::new(),
            affordance_seed_id: String::new(),
            on_consume_consequence_id: String::new(),
            on_destroy_consequence_id: String::new(),
            on_equip_consequence_id: String::new(),
            on_unequip_consequence_id: String::new(),
            grants_capability_while_equipped: String::new(),
            grants_capability_while_present_in_location: String::new(),
            slot_compatibility_predicate: String::new(),
            stackable: false,
            description: String::new(),
        });
        assert!(!codes(&m).contains(&MF_VAL_ID_BUNDLE_NAMESPACE.to_string()));
    }

    /// A namespace warning is expected because `entity_key = "actor"` carries no namespace at all, so it
    /// trivially fails to live under any real bundle's namespace.
    #[test]
    fn entity_key_outside_own_namespace_warns_291() {
        let mut m = Manifest::default();
        m.entities.push(entity_in("actor", "affordances_tour"));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_ID_BUNDLE_NAMESPACE)
            .expect("MF-VAL-291 must fire for a non-namespaced entity_key");
        assert_eq!(hit.severity, Severity::Warning);
    }

    #[test]
    fn entity_key_in_own_namespace_clean_291() {
        let mut m = Manifest::default();
        m.entities
            .push(entity_in("affordances_tour/actor", "affordances_tour"));
        assert!(!codes(&m).contains(&MF_VAL_ID_BUNDLE_NAMESPACE.to_string()));
    }

    fn entity_in(entity_key: &str, bundle_id: &str) -> EntityRow {
        EntityRow {
            entity_key: entity_key.into(),
            bundle_id: bundle_id.into(),
            entity_def_id: String::new(),
            name: entity_key.into(),
            kind: EntityKindRow::Npc,
            place_node_id: String::new(),
            quantity: 1,
            owned_by_entity_key: String::new(),
            description: String::new(),
        }
    }

    /// A namespace warning is expected because `place_graph_id = "fixture"` repeated across four demo
    /// bundles, each squatting no namespace at all.
    #[test]
    fn place_graph_outside_own_namespace_warns_291() {
        let mut m = Manifest::default();
        m.place_graphs
            .push(place_graph("fixture", "machine_fixture"));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_ID_BUNDLE_NAMESPACE)
            .expect("MF-VAL-291 must fire for a non-namespaced place_graph_id");
        assert_eq!(hit.severity, Severity::Warning);
    }

    #[test]
    fn place_graph_in_own_namespace_clean_291() {
        let mut m = Manifest::default();
        m.place_graphs
            .push(place_graph("machine_fixture/fixture", "machine_fixture"));
        m.place_graphs
            .push(place_graph("machine_fixture", "machine_fixture")); // exact match allowed
        assert!(!codes(&m).contains(&MF_VAL_ID_BUNDLE_NAMESPACE.to_string()));
    }

    /// An empty `bundle_id` (unauthored / defaulted row) must not crash or
    /// false-positive — a different lint (or admission itself) owns
    /// flagging an unknown/missing bundle.
    #[test]
    fn empty_bundle_id_is_ignored_291() {
        let mut m = Manifest::default();
        m.entity_definitions.push(EntityDefinitionRow {
            entity_def_id: "srd/hp".into(),
            bundle_id: String::new(),
            display_name: "No bundle".into(),
            lineage_id: String::new(),
            affordance_seed_id: String::new(),
            on_consume_consequence_id: String::new(),
            on_destroy_consequence_id: String::new(),
            on_equip_consequence_id: String::new(),
            on_unequip_consequence_id: String::new(),
            grants_capability_while_equipped: String::new(),
            grants_capability_while_present_in_location: String::new(),
            slot_compatibility_predicate: String::new(),
            stackable: false,
            description: String::new(),
        });
        assert!(!codes(&m).contains(&MF_VAL_ID_BUNDLE_NAMESPACE.to_string()));
    }

    #[test]
    fn outcome_graph_node_missing_graph_is_fatal() {
        let mut manifest = Manifest::default();
        manifest.outcome_graph_nodes.push(OutcomeGraphNodeRow {
            outcome_graph_node_id: "test/missing:root".into(),
            outcome_graph_id: "test/missing".into(),
            node_id: "root".into(),
            outcome_template_id: String::new(),
            terminal: true,
            authored_order: 0,
        });

        assert!(codes(&manifest).contains(&MF_VAL_OUTCOME_GRAPH_NODE_REF.to_string()));
    }

    #[test]
    fn outcome_graph_missing_root_is_fatal() {
        let mut manifest = graph_manifest();
        manifest.outcome_graphs[0].root_node_id = "missing".into();

        assert!(codes(&manifest).contains(&MF_VAL_OUTCOME_GRAPH_ROOT_REF.to_string()));
    }

    #[test]
    fn outcome_graph_edge_missing_endpoint_is_fatal() {
        let mut manifest = graph_manifest();
        manifest.outcome_graph_edges.push(OutcomeGraphEdgeRow {
            outcome_graph_edge_id: "test/edge".into(),
            outcome_graph_id: "test/graph".into(),
            from_node_id: "root".into(),
            to_node_id: "missing".into(),
            guard_predicate_source: String::new(),
            authored_order: 0,
        });

        assert!(codes(&manifest).contains(&MF_VAL_OUTCOME_GRAPH_EDGE_REF.to_string()));
    }

    #[test]
    fn outcome_graph_edge_guard_parse_error_is_fatal() {
        let mut manifest = graph_manifest();
        manifest.outcome_graph_edges.push(OutcomeGraphEdgeRow {
            outcome_graph_edge_id: "test/edge".into(),
            outcome_graph_id: "test/graph".into(),
            from_node_id: "root".into(),
            to_node_id: "root".into(),
            guard_predicate_source: "not(".into(),
            authored_order: 0,
        });

        assert!(codes(&manifest).contains(&MF_VAL_OUTCOME_GRAPH_GUARD_PARSE.to_string()));
    }

    #[test]
    fn outcome_graph_declared_bounds_are_fatal() {
        let mut manifest = graph_manifest();
        manifest.outcome_graphs[0].max_nodes = 0;

        assert!(codes(&manifest).contains(&MF_VAL_OUTCOME_GRAPH_BOUNDS.to_string()));
    }

    // --- sk-84i.3 (parent sk-84i): entity_definition_initial_containment /
    // _equipment ref checks, equipment-slot-dup guard, containment-graph
    // cycle check ---------------------------------------------------------

    fn containment_seed(
        entity_def_id: &str,
        contained_entity_def_id: &str,
        quantity: u64,
    ) -> EntityDefinitionInitialContainmentRow {
        EntityDefinitionInitialContainmentRow {
            entity_def_id: entity_def_id.into(),
            contained_entity_def_id: contained_entity_def_id.into(),
            quantity,
            bundle_id: "test".into(),
        }
    }

    fn equipment_seed(
        entity_def_id: &str,
        equipped_entity_def_id: &str,
        slot_id: &str,
    ) -> EntityDefinitionInitialEquipmentRow {
        EntityDefinitionInitialEquipmentRow {
            entity_def_id: entity_def_id.into(),
            equipped_entity_def_id: equipped_entity_def_id.into(),
            slot_id: slot_id.into(),
            bundle_id: "test".into(),
        }
    }

    fn equipment_slot(slot_id: &str) -> EquipmentSlotDefinitionRow {
        EquipmentSlotDefinitionRow {
            slot_id: slot_id.into(),
            bundle_id: "test".into(),
            label: slot_id.into(),
            default_compatibility_predicate: "true".into(),
            description: String::new(),
        }
    }

    #[test]
    fn dsl_equipment_slot_graph_guard_uses_graph_bundle() {
        let mut manifest = graph_manifest();
        manifest.bundles.push(bundle("test"));
        manifest.bundles.push(bundle("other"));
        manifest.outcome_graph_edges.push(OutcomeGraphEdgeRow {
            outcome_graph_edge_id: "test/edge".into(),
            outcome_graph_id: "test/graph".into(),
            from_node_id: "root".into(),
            to_node_id: "root".into(),
            guard_predicate_source: "actor.equipped_item('other/grip').has_tag('test/tool')".into(),
            authored_order: 0,
        });
        assert!(codes(&manifest).contains(&MF_VAL_DSL_EQUIPMENT_SLOT_REF.to_string()));
        let mut slot = equipment_slot("other/grip");
        slot.bundle_id = "other".into();
        manifest.equipment_slot_definitions.push(slot);
        assert!(codes(&manifest).contains(&MF_VAL_CROSS_BUNDLE_SCOPE.to_string()));
        manifest.bundles[0].depends_on.push("other".into());
        let diagnostics = codes(&manifest);
        assert!(!diagnostics.contains(&MF_VAL_CROSS_BUNDLE_SCOPE.to_string()));
        assert!(!diagnostics.contains(&MF_VAL_DSL_EQUIPMENT_SLOT_REF.to_string()));
    }

    #[test]
    fn dsl_equipment_slot_requires_authored_reference_and_bundle_scope() {
        let mut manifest = Manifest::default();
        manifest.bundles.push(bundle("test"));
        manifest.bundles.push(bundle("other"));
        for source in [
            "actor.equipped_item('test/grip').has_tag('test/tool')",
            "actor.equipped_item('test/grip').stat('test/value') > 0",
            "roll_table(actor.equipped_item('test/grip').text('test/profile')) > 0",
        ] {
            let expression = spellkave_dsl::parse_predicate(source).unwrap();
            let check = |manifest: &Manifest, owner| {
                let mut diagnostics = Vec::new();
                check_equipment_slot_refs(
                    &expression,
                    owner,
                    "test/source",
                    "test/row",
                    &RefIndex::build(manifest),
                    &mut diagnostics,
                );
                diagnostics
            };
            manifest.equipment_slot_definitions.clear();
            let missing = check(&manifest, "test");
            assert_eq!(missing.len(), 1);
            assert_eq!(missing[0].code, MF_VAL_DSL_EQUIPMENT_SLOT_REF);
            assert_eq!(missing[0].severity, Severity::Fatal);
            manifest
                .equipment_slot_definitions
                .push(equipment_slot("test/grip"));
            assert!(check(&manifest, "test").is_empty());
            manifest.bundles[1].depends_on.clear();
            assert_eq!(check(&manifest, "other")[0].code, MF_VAL_CROSS_BUNDLE_SCOPE);
            manifest.bundles[1].depends_on.push("test".into());
            assert!(check(&manifest, "other").is_empty());
        }
    }

    #[test]
    fn containment_missing_entity_def_id_is_fatal_252() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/quiver"));
        m.entity_definition_initial_containment
            .push(containment_seed("test/missing", "test/quiver", 1));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_ENTITY_REF.to_string()));
    }

    #[test]
    fn containment_missing_contained_def_is_fatal_310() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/bandit"));
        m.entity_definition_initial_containment
            .push(containment_seed("test/bandit", "test/missing_quiver", 1));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_CONTAINMENT_REF.to_string()));
    }

    #[test]
    fn containment_valid_refs_are_clean() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/bandit"));
        m.entity_definitions.push(entity_definition("test/quiver"));
        m.entity_definition_initial_containment
            .push(containment_seed("test/bandit", "test/quiver", 1));
        let diags = diags_for(&m);
        assert!(diags.is_empty(), "valid containment seed: {diags:?}");
    }

    #[test]
    fn equipment_missing_item_def_is_fatal_311() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/missing_scimitar",
            "test/main_hand",
        ));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_EQUIPMENT_ITEM_REF.to_string()));
    }

    #[test]
    fn equipment_missing_slot_def_is_fatal_312() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        m.entity_definitions
            .push(entity_definition("test/scimitar"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/scimitar",
            "test/missing_slot",
        ));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_EQUIPMENT_SLOT_REF.to_string()));
    }

    #[test]
    fn equipment_valid_refs_are_clean() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        m.entity_definitions
            .push(entity_definition("test/scimitar"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/scimitar",
            "test/main_hand",
        ));
        let diags = diags_for(&m);
        assert!(diags.is_empty(), "valid equipment seed: {diags:?}");
    }

    #[test]
    fn equipment_duplicate_slot_on_same_def_is_fatal_313() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        m.entity_definitions
            .push(entity_definition("test/scimitar"));
        m.entity_definitions.push(entity_definition("test/dagger"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/scimitar",
            "test/main_hand",
        ));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/dagger",
            "test/main_hand",
        ));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_EQUIPMENT_SLOT_DUP.to_string()));
    }

    #[test]
    fn equipment_same_slot_on_different_defs_is_clean() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        m.entity_definitions.push(entity_definition("test/orc"));
        m.entity_definitions
            .push(entity_definition("test/scimitar"));
        m.entity_definitions.push(entity_definition("test/axe"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/scimitar",
            "test/main_hand",
        ));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/orc",
            "test/axe",
            "test/main_hand",
        ));
        let diags = diags_for(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_DEF_SEED_EQUIPMENT_SLOT_DUP),
            "two different defs sharing a slot id must not collide: {diags:?}"
        );
    }

    // --- A-5 (canon-warden, PR #2154): MF-VAL-315 slot-compatibility twin ---

    #[test]
    fn equipment_seed_slot_incompatible_predicate_is_fatal_315() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        let mut helmet = entity_definition("test/helmet");
        helmet.slot_compatibility_predicate = r#"slot_id == "test/head""#.into();
        m.entity_definitions.push(helmet);
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/helmet",
            "test/main_hand",
        ));
        assert!(
            codes(&m).contains(&MF_VAL_DEF_SEED_EQUIPMENT_SLOT_COMPATIBILITY.to_string()),
            "seeding a head-only helmet into main_hand must be caught at commit"
        );
    }

    #[test]
    fn equipment_seed_false_predicate_is_fatal_315() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        let mut cursed = entity_definition("test/cursed_item");
        cursed.slot_compatibility_predicate = "false".into();
        m.entity_definitions.push(cursed);
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/cursed_item",
            "test/main_hand",
        ));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_EQUIPMENT_SLOT_COMPATIBILITY.to_string()));
    }

    #[test]
    fn equipment_seed_matching_predicate_is_clean_315() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        let mut scimitar = entity_definition("test/scimitar");
        scimitar.slot_compatibility_predicate = r#"slot_id == "test/main_hand""#.into();
        m.entity_definitions.push(scimitar);
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/scimitar",
            "test/main_hand",
        ));
        let diags = diags_for(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_DEF_SEED_EQUIPMENT_SLOT_COMPATIBILITY),
            "matching slot_id predicate must not be flagged: {diags:?}"
        );
    }

    #[test]
    fn equipment_seed_empty_predicate_is_clean_315() {
        // Empty predicate is the equipment_valid_refs_are_clean fixture's
        // shape (default entity_definition()) — reconfirm the compatibility
        // check specifically stays silent on it too.
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        m.entity_definitions
            .push(entity_definition("test/scimitar"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/scimitar",
            "test/main_hand",
        ));
        let diags = diags_for(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_DEF_SEED_EQUIPMENT_SLOT_COMPATIBILITY),
            "empty predicate means compatible with any slot: {diags:?}"
        );
    }

    // --- sk-ntz.4: MF-VAL-326 instance-level sibling of MF-VAL-315 ---------

    fn actor_equip_seed(
        actor_entity_key: &str,
        item_key: &str,
        slot_id: &str,
    ) -> ActorEquipmentSlotRow {
        ActorEquipmentSlotRow {
            actor_entity_key: actor_entity_key.into(),
            slot_id: slot_id.into(),
            item_key: item_key.into(),
        }
    }

    fn item_entity(entity_key: &str, entity_def_id: &str) -> EntityRow {
        let mut row = entity(entity_key);
        row.kind = EntityKindRow::Item;
        row.entity_def_id = entity_def_id.into();
        row
    }

    #[test]
    fn actor_equip_slot_incompatible_predicate_is_fatal_326() {
        let mut m = Manifest::default();
        let mut helmet = entity_definition("test/helmet");
        helmet.slot_compatibility_predicate = r#"slot_id == "test/head""#.into();
        m.entity_definitions.push(helmet);
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entities
            .push(item_entity("test/regdar_helmet", "test/helmet"));
        m.actor_equipment_slots.push(actor_equip_seed(
            "test/regdar",
            "test/regdar_helmet",
            "test/main_hand",
        ));
        assert!(
            codes(&m).contains(&MF_VAL_ACTOR_EQUIP_SLOT_COMPATIBILITY.to_string()),
            "authoring a head-only helmet into main_hand must be caught at manifest commit"
        );
    }

    #[test]
    fn actor_equip_slot_false_predicate_is_fatal_326() {
        let mut m = Manifest::default();
        let mut cursed = entity_definition("test/cursed_item");
        cursed.slot_compatibility_predicate = "false".into();
        m.entity_definitions.push(cursed);
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entities
            .push(item_entity("test/regdar_cursed", "test/cursed_item"));
        m.actor_equipment_slots.push(actor_equip_seed(
            "test/regdar",
            "test/regdar_cursed",
            "test/main_hand",
        ));
        assert!(codes(&m).contains(&MF_VAL_ACTOR_EQUIP_SLOT_COMPATIBILITY.to_string()));
    }

    #[test]
    fn actor_equip_slot_matching_predicate_is_clean_326() {
        let mut m = Manifest::default();
        let mut scimitar = entity_definition("test/scimitar");
        scimitar.slot_compatibility_predicate = r#"slot_id == "test/main_hand""#.into();
        m.entity_definitions.push(scimitar);
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entities
            .push(item_entity("test/regdar_scimitar", "test/scimitar"));
        m.actor_equipment_slots.push(actor_equip_seed(
            "test/regdar",
            "test/regdar_scimitar",
            "test/main_hand",
        ));
        let diags = diags_for(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_ACTOR_EQUIP_SLOT_COMPATIBILITY),
            "matching slot_id predicate must not be flagged: {diags:?}"
        );
    }

    #[test]
    fn actor_equip_slot_no_entity_def_id_is_clean_326() {
        // An item entity with no entity_def_id has no entity_definitions row
        // to evaluate a predicate from — the runtime's own
        // `check_slot_compatibility` short-circuits `item_def: None` to
        // `Ok(())`, so this check must mirror that, not false-flag it.
        let mut m = Manifest::default();
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entities.push(entity("test/regdar_mystery_item"));
        m.actor_equipment_slots.push(actor_equip_seed(
            "test/regdar",
            "test/regdar_mystery_item",
            "test/main_hand",
        ));
        let diags = diags_for(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_ACTOR_EQUIP_SLOT_COMPATIBILITY),
            "no entity_def_id means no predicate to evaluate: {diags:?}"
        );
    }

    #[test]
    fn containment_direct_self_cycle_is_fatal_314() {
        let mut m = Manifest::default();
        m.entity_definitions
            .push(entity_definition("test/ouroboros"));
        m.entity_definition_initial_containment
            .push(containment_seed("test/ouroboros", "test/ouroboros", 1));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_CONTAINMENT_CYCLE.to_string()));
    }

    #[test]
    fn containment_transitive_cycle_is_fatal_314() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/a"));
        m.entity_definitions.push(entity_definition("test/b"));
        m.entity_definition_initial_containment
            .push(containment_seed("test/a", "test/b", 1));
        m.entity_definition_initial_containment
            .push(containment_seed("test/b", "test/a", 1));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_CONTAINMENT_CYCLE.to_string()));
    }

    #[test]
    fn containment_chain_without_cycle_is_clean() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/bandit"));
        m.entity_definitions.push(entity_definition("test/quiver"));
        m.entity_definitions.push(entity_definition("test/bolt"));
        m.entity_definition_initial_containment
            .push(containment_seed("test/bandit", "test/quiver", 1));
        m.entity_definition_initial_containment
            .push(containment_seed("test/quiver", "test/bolt", 20));
        let diags = diags_for(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_DEF_SEED_CONTAINMENT_CYCLE),
            "a -> b -> c chain is not a cycle: {diags:?}"
        );
    }

    // --- D-4 (canon-warden, PR #2154): the MF-VAL-314 graph must union
    // containment AND equipment edges, not just containment ------------------

    #[test]
    fn equipment_direct_cycle_is_fatal_314() {
        // A equips B, B equips A — a transitive cycle reachable ONLY through
        // the equipment table. Before D-4 the graph never saw these edges.
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/a"));
        m.entity_definitions.push(entity_definition("test/b"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/a",
            "test/b",
            "test/main_hand",
        ));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/b",
            "test/a",
            "test/main_hand",
        ));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_CONTAINMENT_CYCLE.to_string()));
    }

    #[test]
    fn mixed_containment_equipment_cycle_is_fatal_314() {
        // A contains B (containment table), B equips A (equipment table) —
        // a cycle that crosses both tables. Neither table's graph alone
        // contains it; only the union does.
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/a"));
        m.entity_definitions.push(entity_definition("test/b"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_containment
            .push(containment_seed("test/a", "test/b", 1));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/b",
            "test/a",
            "test/main_hand",
        ));
        assert!(codes(&m).contains(&MF_VAL_DEF_SEED_CONTAINMENT_CYCLE.to_string()));
    }

    #[test]
    fn equipment_chain_without_cycle_is_clean() {
        let mut m = Manifest::default();
        m.entity_definitions.push(entity_definition("test/goblin"));
        m.entity_definitions
            .push(entity_definition("test/scimitar"));
        m.equipment_slot_definitions
            .push(equipment_slot("test/main_hand"));
        m.entity_definition_initial_equipment.push(equipment_seed(
            "test/goblin",
            "test/scimitar",
            "test/main_hand",
        ));
        let diags = diags_for(&m);
        assert!(
            !diags
                .iter()
                .any(|d| d.code == MF_VAL_DEF_SEED_CONTAINMENT_CYCLE),
            "a single equip edge is not a cycle: {diags:?}"
        );
    }
}

#[cfg(test)]
mod place_niche_attachment_tests {
    use super::*;
    use spellkave_schema::{
        AffordanceTemplateRow, Manifest, NicheDefinitionRow, OutcomeTemplateRow, PlaceNodeRow,
        SpatialNicheAttachmentRow, VisibilityClass,
    };

    fn place_node(node_id: &str) -> PlaceNodeRow {
        PlaceNodeRow {
            node_id: node_id.into(),
            bundle_id: "scene".into(),
            place_graph_id: "scene/graph".into(),
            local_id: "anchor".into(),
            substrate_id: "indoor".into(),
            display_name: "Anchor".into(),
            description: String::new(),
            cell_res10: 0,
            cell_res7: 0,
            cell_res4: 0,
            pose_elevation: 0.0,
            pose_elevation_present: false,
            pos_x: 0,
            pos_y: 0,
            pos_present: false,
            extent_shape_id: String::new(),
            extent_params_json: String::new(),
            is_ephemeral: false,
        }
    }

    fn niche(id: &str, provider: &str) -> NicheDefinitionRow {
        NicheDefinitionRow {
            niche_id: id.into(),
            bundle_id: "srd".into(),
            display_name: id.into(),
            description: String::new(),
            predicate_source: String::new(),
            provider: provider.into(),
        }
    }

    fn affordance(template_id: &str, niche_id: &str, outcome_id: &str) -> AffordanceTemplateRow {
        AffordanceTemplateRow {
            template_id: template_id.into(),
            bundle_id: "srd".into(),
            niche_id: niche_id.into(),
            capability_id: "srd/attack".into(),
            display_name: template_id.into(),
            predicate_bool: true,
            predicate_source: String::new(),
            accepts_global_storm: false,
            locality_class: String::new(),
            reads_occupancy: false,
            description: String::new(),
            stratification_ok: true,
            outcome_template_id: outcome_id.into(),
            range_m: 0.0,
            spends_channel: None,
            recharges_on: None,
            composes: None,
            movement_cost_expression_source: String::new(),
        }
    }

    fn outcome(id: &str, movement_descriptor: &str) -> OutcomeTemplateRow {
        OutcomeTemplateRow {
            outcome_template_id: id.into(),
            bundle_id: "srd".into(),
            display_name: id.into(),
            outcome_bool: true,
            outcome_expression_source: String::new(),
            manipulation_action_id: String::new(),
            communication_speech_act_id: String::new(),
            movement_descriptor: movement_descriptor.into(),
            sense_query_kind: String::new(),
            sense_visibility: VisibilityClass::Local,
            consequence_template_id: String::new(),
            discovery_validated: false,
            description: String::new(),
        }
    }

    fn attachment(niche_id: &str, node_id: &str) -> SpatialNicheAttachmentRow {
        SpatialNicheAttachmentRow {
            bundle_id: "scene".into(),
            place_node_id: node_id.into(),
            place_edge_id: None,
            niche_id: niche_id.into(),
            visibility: "Public".into(),
            condition_bool: true,
            ecological_invariant: false,
            on_entry_action_id: String::new(),
            on_entry_target_entity_key: String::new(),
        }
    }

    fn codes(manifest: &Manifest) -> Vec<String> {
        let mut diags = Vec::new();
        validate_cross_row_refs(manifest, None, &mut diags);
        diags.into_iter().map(|d| d.code).collect()
    }

    fn diags_for(manifest: &Manifest) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        validate_cross_row_refs(manifest, None, &mut diags);
        diags
    }

    /// sk-84i.1 / sk-2wg RED: a scene with a place-provided niche-gated
    /// affordance and no attachment used to admit, then die at
    /// attachment_presence.
    #[test]
    fn place_provided_niche_without_attachment_warns_328() {
        let mut m = Manifest::default();
        m.place_nodes.push(place_node("scene/graph/anchor"));
        m.niche_definitions
            .push(niche("srd/niche/strikeable", "place"));
        m.outcome_templates
            .push(outcome("srd/attack/melee/outcome", ""));
        m.affordance_templates.push(affordance(
            "srd/attack/melee/affordance",
            "srd/niche/strikeable",
            "srd/attack/melee/outcome",
        ));
        let diags = diags_for(&m);
        let hit = diags
            .iter()
            .find(|d| d.code == MF_VAL_PLACE_NICHE_ATTACHMENT)
            .expect("MF-VAL-328 must fire when a place-provided niche has no attachment");
        assert_eq!(hit.severity, Severity::Warning);
        assert!(hit.message.contains("srd/niche/strikeable"));
        assert!(hit.message.contains("srd/attack/melee/affordance"));
    }

    #[test]
    fn place_provided_niche_with_attachment_is_clean_328() {
        let mut m = Manifest::default();
        m.place_nodes.push(place_node("scene/graph/anchor"));
        m.niche_definitions
            .push(niche("srd/niche/strikeable", "place"));
        m.outcome_templates
            .push(outcome("srd/attack/melee/outcome", ""));
        m.affordance_templates.push(affordance(
            "srd/attack/melee/affordance",
            "srd/niche/strikeable",
            "srd/attack/melee/outcome",
        ));
        m.spatial_niche_attachments
            .push(attachment("srd/niche/strikeable", "scene/graph/anchor"));
        assert!(!codes(&m).contains(&MF_VAL_PLACE_NICHE_ATTACHMENT.to_string()));
    }

    #[test]
    fn definitions_only_bundle_without_place_nodes_is_quiet_328() {
        let mut m = Manifest::default();
        m.niche_definitions
            .push(niche("srd/niche/strikeable", "place"));
        m.outcome_templates
            .push(outcome("srd/attack/melee/outcome", ""));
        m.affordance_templates.push(affordance(
            "srd/attack/melee/affordance",
            "srd/niche/strikeable",
            "srd/attack/melee/outcome",
        ));
        assert!(!codes(&m).contains(&MF_VAL_PLACE_NICHE_ATTACHMENT.to_string()));
    }

    #[test]
    fn actor_provided_niche_does_not_need_attachment_328() {
        let mut m = Manifest::default();
        m.place_nodes.push(place_node("scene/graph/anchor"));
        m.niche_definitions
            .push(niche("srd/niche/attack-verb", "actor"));
        m.outcome_templates
            .push(outcome("srd/attack/melee/outcome", ""));
        m.affordance_templates.push(affordance(
            "srd/attack/melee/affordance",
            "srd/niche/attack-verb",
            "srd/attack/melee/outcome",
        ));
        assert!(!codes(&m).contains(&MF_VAL_PLACE_NICHE_ATTACHMENT.to_string()));
    }

    #[test]
    fn entity_provided_niche_does_not_need_attachment_328() {
        let mut m = Manifest::default();
        m.place_nodes.push(place_node("scene/graph/anchor"));
        m.niche_definitions.push(niche("srd.creature", "entity"));
        m.outcome_templates
            .push(outcome("srd/persuade/outcome", ""));
        m.affordance_templates.push(affordance(
            "srd/persuade/affordance",
            "srd.creature",
            "srd/persuade/outcome",
        ));
        assert!(!codes(&m).contains(&MF_VAL_PLACE_NICHE_ATTACHMENT.to_string()));
    }

    /// Missing niche_definitions row fail-closes to place (Gate B default),
    /// which is the proof-scene strikeable shape that died at runtime.
    #[test]
    fn missing_niche_definition_without_attachment_warns_328() {
        let mut m = Manifest::default();
        m.place_nodes.push(place_node("scene/graph/anchor"));
        m.outcome_templates
            .push(outcome("srd/attack/melee/outcome", ""));
        m.affordance_templates.push(affordance(
            "srd/attack/melee/affordance",
            "srd/niche/strikeable",
            "srd/attack/melee/outcome",
        ));
        assert!(codes(&m).contains(&MF_VAL_PLACE_NICHE_ATTACHMENT.to_string()));
    }

    #[test]
    fn traversal_affordance_does_not_need_spatial_attachment_328() {
        let mut m = Manifest::default();
        m.place_nodes.push(place_node("scene/graph/anchor"));
        m.niche_definitions
            .push(niche("core/passable_portal", "place"));
        m.outcome_templates
            .push(outcome("core/walk/outcome", "traverse_edge"));
        m.affordance_templates.push(affordance(
            "core/walk_edge",
            "core/passable_portal",
            "core/walk/outcome",
        ));
        assert!(!codes(&m).contains(&MF_VAL_PLACE_NICHE_ATTACHMENT.to_string()));
    }
}
