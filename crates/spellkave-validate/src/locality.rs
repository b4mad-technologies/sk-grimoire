//! CEL-AST (DSL) locality classifier for affordance template predicates.
//!
//! Per ADR-0027 D-1: walks the DSL `Expression` AST and assigns one of four
//! locality classes. Unrecognised patterns fall back to `Global` (G-4).

use spellkave_dsl::{Expression, Op, SubjectRef};

/// Locality class assigned by static predicate analysis (ADR-0027 D-1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalityClass {
    /// Predicate references only the subject actor's own fields / traces.
    Local,
    /// Predicate references rows at a bounded `place_node` (actor's current node).
    Place,
    /// Predicate references a tagged subset (capability / alignment / faction).
    Family,
    /// Conservative fallback: cannot prove bounded scope.
    Global,
}

impl LocalityClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            LocalityClass::Local => "Local",
            LocalityClass::Place => "Place",
            LocalityClass::Family => "Family",
            LocalityClass::Global => "Global",
        }
    }
}

/// Classify the locality of a parsed DSL predicate expression.
///
/// Rules (applied per-node; worst-case wins):
/// - `BoolLiteral`, `IntLiteral`, `TextLiteral` → `Local` (constant)
/// - `ReadField(Actor, _)`, `HasCapability(Actor, _)`, `HasItem(Actor, _)`,
///   `HasNiche(Actor, _)`,
///   `HasStatus(Actor, _)`, `TracePresent(Actor, _)` → `Local`
/// - `ReadField(Location, _)`, `TracePresent(Location, _)` → `Place`
///   (location subject reads from the actor's current place_node)
/// - `ReadField(Target, _)`, `HasNiche(Target, _)`, `HasStatus(Target, _)`,
///   `TracePresent(Target, _)` → `Place` (target is a co-located entity)
/// - `HasCapability(Actor, cap)` alone → `Local`; when used as the subject
///   of an outer iteration (`Binding`) → `Family`
/// - `ReadField(World, _)` → `Global` (accesses global state)
/// - `Binding(_)` subject → `Global` (lambda/iteration variable; G-4)
/// - `And`, `Or`, `Not`, `Compare`, `Arithmetic`, `Roll*` → recurse; take worst
///
/// The four classes form a total order: Local < Place < Family < Global.
pub fn classify(expr: &Expression) -> LocalityClass {
    classify_with(expr, false)
}

/// As [`classify`], but for the reaction-TRIGGER surface (§8.4 WHAT-provokes,
/// slice F-impl). A `reaction_trigger` reads the fixed triggering-INTENT facts
/// via `$event.*`; the producer evaluates it ONCE per event and never scans, so
/// an `Event` subject is a bounded per-event read (`Local`), NOT the G-4 `Global`
/// fallback the eligibility/affordance surface uses (where no event is in scope).
/// `World`/`Session` reads and outcome ops stay `Global` — a trigger may read the
/// event, not unbounded world state (ADR-0027 D-3). The bound is otherwise
/// identical, so a trigger reading `world.*`/`session.*` is still rejected.
pub fn classify_reaction_trigger(expr: &Expression) -> LocalityClass {
    classify_with(expr, true)
}

fn classify_with(expr: &Expression, event_bounded: bool) -> LocalityClass {
    let mut worst = LocalityClass::Local;
    for node in &expr.nodes {
        let class = classify_op(&node.op, event_bounded);
        worst = worst_case(worst, class);
        if worst == LocalityClass::Global {
            break;
        }
    }
    worst
}

fn worst_case(a: LocalityClass, b: LocalityClass) -> LocalityClass {
    use LocalityClass::*;
    match (a, b) {
        (Global, _) | (_, Global) => Global,
        (Family, _) | (_, Family) => Family,
        (Place, _) | (_, Place) => Place,
        (Local, Local) => Local,
    }
}

fn classify_subject(subject: &SubjectRef, event_bounded: bool) -> LocalityClass {
    match subject {
        // The wielded item is reached through the actor's own equipment/inventory
        // (no world scan) → locally bounded, same class as the actor itself.
        SubjectRef::Actor | SubjectRef::ActorWieldedItem | SubjectRef::ActorEquippedItem { .. } => {
            LocalityClass::Local
        }
        SubjectRef::Target | SubjectRef::Location => LocalityClass::Place,
        SubjectRef::Session | SubjectRef::World => LocalityClass::Global,
        // The trigger surface populates `$event.*` with the fixed per-event
        // intent facts (a bounded read, no scan); the eligibility/affordance
        // surface has no event in scope → G-4 conservative Global.
        SubjectRef::Event(_) if event_bounded => LocalityClass::Local,
        SubjectRef::Binding(_) | SubjectRef::Event(_) => LocalityClass::Global,
    }
}

fn classify_op(op: &Op, event_bounded: bool) -> LocalityClass {
    match op {
        Op::BoolLiteral(_) | Op::IntLiteral(_) | Op::TextLiteral(_) => LocalityClass::Local,

        Op::ReadInt(rf)
        | Op::ReadEffectiveInt(rf)
        | Op::ReadText(rf)
        | Op::ReadBool(rf)
        // #30 / sk-ct8.2: `resource_max`/`resource_min` read a per-subject
        // fact exactly like `stat`/`effective` (no world scan) — same
        // subject-based classification.
        | Op::ReadResourceMax(rf)
        | Op::ReadResourceMin(rf) => classify_subject(&rf.subject, event_bounded),

        Op::HasCapability(hc) => classify_subject(&hc.subject, event_bounded),
        Op::HasItem(hi) => classify_subject(&hi.subject, event_bounded),
        Op::HasNiche(hn) => classify_subject(&hn.subject, event_bounded),
        Op::HasTag(ht) => classify_subject(&ht.subject, event_bounded),
        // `shares_tag` reads only the two named subjects' own tag pools (no world
        // scan, no storm — ADR-0027 D-1); worst-case over the two subjects. With
        // Actor + Target that is Place.
        Op::SharesTag(st) => worst_case(
            classify_subject(&st.subject_a, event_bounded),
            classify_subject(&st.subject_b, event_bounded),
        ),
        Op::HasStatus(hs) => classify_subject(&hs.subject, event_bounded),
        Op::ImpliedByCount(_) => LocalityClass::Local,
        Op::ParticipantTagGroupCount(_) => LocalityClass::Global,
        // K-A Fact 2: `peers_near_target` is
        // anchored at Target's `place_node` — same bound as any other
        // Target-subject read (`ReadField(Target, _)` etc. above), not the
        // unbounded `Global` `ParticipantTagGroupCount` needs (session-wide,
        // no spatial bound at all).
        Op::PeersNearTarget(_) => LocalityClass::Place,
        // ADR-0134 D-3: `occupants_at` reads the occupant set of ONE place node
        // (the edge's incident node on the traversal path, Target's node on the
        // manipulate path) — bounded by the same `place_node` the predicate is
        // already anchored to, so `Place`, never `Global`.
        Op::OccupantsAt(_) => LocalityClass::Place,
        // `channel_pool` reads one authored `economy_channel_definitions` row by
        // its literal id — a bounded O(1) constant lookup, no subject, no world
        // scan (same reasoning as `ImpliedByCount`, sk-ct8.4 / finding #86).
        Op::ChannelPool(_) => LocalityClass::Local,
        Op::TracePresent(tp) => classify_subject(&tp.subject, event_bounded),
        Op::KnowsEntity(ke) => classify_subject(&ke.subject, event_bounded),

        // Logical / arithmetic combinators: child nodes are visited separately
        // in `classify` via the flat node list; return Local here (no subject).
        Op::And(_) | Op::Or(_) | Op::Not(_) | Op::Let(_) => LocalityClass::Local,
        Op::Compare(_) | Op::Arithmetic(_) | Op::Cond(_) => LocalityClass::Local,

        // Stochastic nodes: no table reads, bounded to actor.
        Op::Roll(_) | Op::RollUnder(_) | Op::RollTable(_) => LocalityClass::Local,

        // sk-84i.2 (widened G-1): `<subject>.source(slot)` evaluates a
        // DATA-DEFINED sub-expression (an authored source text) this static
        // walk cannot see into — unlike `RollTable`'s bounded weighted-row
        // lookup, the item's own source could read ANY subject (including
        // `world`/`session`), regardless of what THIS node's own `subject`
        // is. Cannot prove bounded scope → G-4 conservative fallback, same
        // as the outcome-only ops below.
        Op::DslSource(_) => LocalityClass::Global,

        // Outcome nodes should not appear in predicates; treat conservatively.
        Op::OutcomeSequence(_)
        | Op::InvokeOutcomeGraph(_)
        | Op::ModifyResource(_)
        | Op::ApplyStatus(_)
        | Op::EmitTrace(_)
        | Op::Arbitrate(_) => LocalityClass::Global,
    }
}

/// Static predicate **state-input dependencies** (incremental-affordance
/// reconciliation, #886). Returns the sorted/deduped set of `state_id`s
/// (`field_id`s) the predicate reads via `ReadInt`/`ReadText`/`ReadBool` — the
/// direct field-reading ops. Called item-source expressions are separate ASTs
/// and are not covered by this list. Under current offer evaluation semantics,
/// source-bearing comparisons are bypassed as stochastic, so their indirect
/// state reads do not determine offer eligibility.
///
/// Computed at validate time alongside [`classify`] (ADR-0027 D-3: no AST
/// re-walk at frame time). The runtime materialiser intersects this set with a
/// committed frame's dirty `state_id`s to decide which templates a
/// `ModifyResource` could have affected, instead of rebuilding all of them.
///
/// Subject-agnostic by design: a read of `actor.state(x)`, `target.state(x)`,
/// `location.state(x)`, or `world.state(x)` all contribute `x`. This
/// over-covers on the subject axis (a perceiver may be re-derived when only a
/// target-bound read truly matters) — which is conservative (correct), never
/// under-covering on that axis. The flat-node walk visits nested direct reads,
/// not the contents of called item-source expressions.
pub fn predicate_state_inputs(expr: &Expression) -> Vec<String> {
    let mut inputs: Vec<String> = expr
        .nodes
        .iter()
        .filter_map(|node| match &node.op {
            Op::ReadInt(rf) | Op::ReadEffectiveInt(rf) | Op::ReadText(rf) | Op::ReadBool(rf) => {
                let field = rf.field_id.trim();
                (!field.is_empty()).then(|| field.to_string())
            }
            _ => None,
        })
        .collect();
    inputs.sort();
    inputs.dedup();
    inputs
}

/// The `condition_definition_id`s a predicate reads via `Op::HasStatus`.
///
/// The kernel-driven sibling of [`predicate_state_inputs`] (issue #1105): it
/// lets condition-scoped reconcile re-derive a template iff a just-applied
/// condition matches one its predicate reads — derived purely by static
/// analysis, with NO authored declaration (ADR-0021: the kernel classifies, the
/// author does not; ADR-0027 D-3: no frame-time AST re-walk). An author writing
/// a new buff/debuff spell gets correct reconcile scoping for free and cannot
/// reintroduce a `Full`-fallback perf cliff by forgetting to declare anything.
///
/// Subject-agnostic by design (`actor.status(x)` and `target.status(x)` both
/// contribute `x`) — conservative over-cover, never under-cover, matching
/// `predicate_state_inputs`.
pub fn predicate_condition_inputs(expr: &Expression) -> Vec<String> {
    let mut inputs: Vec<String> = expr
        .nodes
        .iter()
        .filter_map(|node| match &node.op {
            Op::HasStatus(hs) => {
                let id = hs.condition_definition_id.trim();
                (!id.is_empty()).then(|| id.to_string())
            }
            _ => None,
        })
        .collect();
    inputs.sort();
    inputs.dedup();
    inputs
}

/// D9 (actor-carried-verbs, canon-warden amendment): true iff `expr` reads
/// any `target.*` fact. Used to decide whether an actor-provided manipulation
/// template's untargeted ("swing at thin air") place-sentinel is safe to
/// materialise — its resolution predicate must read NO target facts, or
/// execution against `target_entity_id = 0` would technical-fail.
///
/// Deliberately narrower than [`classify`]: a `Location` read classifies
/// `Place` too (same locality bound as `Target`) but does NOT block the
/// untargeted sentinel — only an actual `target.*` read does. Exhaustive
/// match (no wildcard arm): a new `Op` variant forces a decision here, same
/// discipline as [`predicate_reads_occupancy`].
pub fn reads_target_subject(expr: &Expression) -> bool {
    expr.nodes
        .iter()
        .any(|node| op_reads_target_subject(&node.op))
}

fn subject_is_target(subject: &SubjectRef) -> bool {
    matches!(subject, SubjectRef::Target)
}

fn op_reads_target_subject(op: &Op) -> bool {
    match op {
        Op::ReadInt(rf)
        | Op::ReadEffectiveInt(rf)
        | Op::ReadText(rf)
        | Op::ReadBool(rf)
        | Op::ReadResourceMax(rf)
        | Op::ReadResourceMin(rf) => subject_is_target(&rf.subject),
        Op::HasCapability(hc) => subject_is_target(&hc.subject),
        Op::HasItem(hi) => subject_is_target(&hi.subject),
        Op::HasNiche(hn) => subject_is_target(&hn.subject),
        Op::HasTag(ht) => subject_is_target(&ht.subject),
        Op::SharesTag(st) => subject_is_target(&st.subject_a) || subject_is_target(&st.subject_b),
        Op::HasStatus(hs) => subject_is_target(&hs.subject),
        Op::TracePresent(tp) => subject_is_target(&tp.subject),
        Op::KnowsEntity(ke) => subject_is_target(&ke.subject),
        Op::BoolLiteral(_)
        | Op::IntLiteral(_)
        | Op::TextLiteral(_)
        | Op::ImpliedByCount(_)
        | Op::ParticipantTagGroupCount(_)
        | Op::ChannelPool(_)
        | Op::And(_)
        | Op::Or(_)
        | Op::Not(_)
        | Op::Let(_)
        | Op::Compare(_)
        | Op::Arithmetic(_)
        | Op::Cond(_)
        | Op::Roll(_)
        | Op::RollUnder(_)
        | Op::RollTable(_)
        | Op::DslSource(_)
        | Op::OutcomeSequence(_)
        | Op::InvokeOutcomeGraph(_)
        | Op::ModifyResource(_)
        | Op::ApplyStatus(_)
        | Op::EmitTrace(_)
        | Op::Arbitrate(_) => false,
        // K-A Fact 2: `peers_near_target` IS a co-located-count / occupancy
        // read by construction — exactly what this validator exists to
        // detect (the earlier doc comment: "no such predicate opcode exists
        // today" no longer holds).
        Op::PeersNearTarget(_) => true,
        // ADR-0134 D-3: the `target` scope counts occupants of Target's own
        // place node — Target-anchored exactly like `PeersNearTarget`. The
        // traversal scopes (`to_node` / `from_node`) read the edge's incident
        // nodes, which the materialiser's sentinel target never carries.
        Op::OccupantsAt(op) => op.scope == "target",
    }
}

/// Static occupancy-read verdict for arrival-target-scoped affordance
/// reconciliation (#1014). Returns true only for DSL predicates that read a
/// co-located-count / occupancy fact.
///
/// K-A Fact 2 introduced the first such
/// opcode: `Op::PeersNearTarget` counts actors co-located within a radius of
/// Target. Every other arm stays false. Keep this match exhaustive: when a
/// future occupancy-reading op is introduced, the compiler forces this
/// validator-side descriptor to be updated before runtime `TargetScoped` can
/// remain sound.
pub fn predicate_reads_occupancy(expr: &Expression) -> bool {
    expr.nodes.iter().any(|node| match &node.op {
        Op::BoolLiteral(_)
        | Op::IntLiteral(_)
        | Op::TextLiteral(_)
        | Op::ReadInt(_)
        | Op::ReadEffectiveInt(_)
        | Op::ReadText(_)
        | Op::ReadBool(_)
        | Op::ReadResourceMax(_)
        | Op::ReadResourceMin(_)
        | Op::HasCapability(_)
        | Op::HasItem(_)
        | Op::HasNiche(_)
        | Op::HasTag(_)
        | Op::SharesTag(_)
        | Op::HasStatus(_)
        | Op::ImpliedByCount(_)
        | Op::ParticipantTagGroupCount(_)
        | Op::ChannelPool(_)
        | Op::TracePresent(_)
        | Op::KnowsEntity(_)
        | Op::And(_)
        | Op::Or(_)
        | Op::Not(_)
        | Op::Compare(_)
        | Op::Arithmetic(_)
        | Op::Cond(_)
        | Op::Roll(_)
        | Op::RollUnder(_)
        | Op::RollTable(_)
        | Op::DslSource(_)
        | Op::Let(_)
        | Op::OutcomeSequence(_)
        | Op::InvokeOutcomeGraph(_)
        | Op::ModifyResource(_)
        | Op::ApplyStatus(_)
        | Op::EmitTrace(_)
        | Op::Arbitrate(_) => false,
        // K-A Fact 2: `peers_near_target` IS
        // a co-located-count / occupancy read by construction — exactly what
        // this validator exists to detect.
        Op::PeersNearTarget(_) => true,
        // ADR-0134 D-3: `occupants_at` reads a place node's occupant set — an
        // occupancy read by definition. The materialiser's arrival-scope guard
        // (`any_template_reads_occupancy`) narrows this to templates it
        // actually offers; a traversal-only template does not demote arrivals.
        Op::OccupantsAt(_) => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_dsl::parse_predicate;

    #[test]
    fn exact_equipment_state_inputs_remain_state_scoped() {
        let expr = parse_predicate(
            "actor.equipped_item('demo/grip').stat('demo/charge') > 0 && actor.equipped_item('demo/bracket').stat('demo/charge') > 0",
        ).unwrap();
        assert_eq!(predicate_state_inputs(&expr), vec!["demo/charge"]);
    }

    #[test]
    fn state_inputs_collect_read_field_ids_across_subjects() {
        // target.stat + actor.state + world.state → all three field_ids, sorted/deduped.
        let expr = parse_predicate(
            r#"target.stat("srd/hp") > 0 && actor.state("core/ready") == true && world.state("srd/hp") == true"#,
        )
        .unwrap();
        assert_eq!(
            predicate_state_inputs(&expr),
            vec!["core/ready".to_string(), "srd/hp".to_string()],
            "subject-agnostic, sorted, deduped across subjects"
        );
    }

    #[test]
    fn state_inputs_empty_for_niche_and_capability_only_predicate() {
        // has_niche / has_capability read no object_state → no state inputs. This is
        // the load-bearing skip: a template that reads only niches/caps is NEVER
        // affected by a ModifyResource (AD-3: niches are not state-derived).
        let expr = parse_predicate(
            r#"actor.has_capability("core/attack") && target.has_niche("srd/monster")"#,
        )
        .unwrap();
        assert!(predicate_state_inputs(&expr).is_empty());
    }

    #[test]
    fn state_inputs_capture_nested_reads() {
        // A read buried inside cond/&&/comparison is still on the flat node list.
        let expr =
            parse_predicate(r#"(actor.stat("srd/hp") - target.stat("srd/ac")) > 0"#).unwrap();
        assert_eq!(
            predicate_state_inputs(&expr),
            vec!["srd/ac".to_string(), "srd/hp".to_string()]
        );
    }

    #[test]
    fn occupancy_reads_are_false_for_current_dsl_ops() {
        let expr = parse_predicate(
            r#"actor.has_capability("core/attack") && target.has_niche("srd/monster")"#,
        )
        .unwrap();
        assert!(
            !predicate_reads_occupancy(&expr),
            "current DSL has no co-located-count / occupancy read primitive"
        );
    }

    #[test]
    fn actor_capability_check_is_local() {
        let expr = parse_predicate(r#"actor.has_capability("core/walk")"#).unwrap();
        assert_eq!(classify(&expr), LocalityClass::Local);
    }

    /// #30 / sk-ct8.2: `resource_max`/`resource_min` classify by subject
    /// exactly like `stat`/`effective` — no world scan, same allow rule.
    #[test]
    fn resource_max_and_min_classify_by_subject_like_stat() {
        let actor_expr = parse_predicate(r#"actor.resource_max("srd/hp") >= 10"#).unwrap();
        assert_eq!(classify(&actor_expr), LocalityClass::Local);

        let target_expr = parse_predicate(r#"target.resource_min("srd/hp") <= 5"#).unwrap();
        assert_eq!(classify(&target_expr), LocalityClass::Place);
    }

    #[test]
    fn resource_max_and_min_do_not_read_occupancy() {
        let expr = parse_predicate(r#"actor.resource_max("srd/hp") >= 10"#).unwrap();
        assert!(!predicate_reads_occupancy(&expr));
    }

    #[test]
    fn shares_tag_actor_target_is_place_not_global() {
        // `shares_tag(actor, target, prefix)` reads only the two named subjects'
        // own tag pools — no world scan, no ADR-0027 D-1 storm. worst_case(Local,
        // Place) = Place.
        let expr = parse_predicate(r#"shares_tag(actor, target, "actor.language.")"#).unwrap();
        assert_eq!(
            classify(&expr),
            LocalityClass::Place,
            "two-subject tag intersection is Place-bounded, never Global"
        );
    }

    #[test]
    fn shares_tag_does_not_read_occupancy() {
        let expr = parse_predicate(r#"shares_tag(actor, target, "actor.language.")"#).unwrap();
        assert!(
            !predicate_reads_occupancy(&expr),
            "tags are not occupancy facts"
        );
    }

    #[test]
    fn location_trace_is_place() {
        let expr = parse_predicate(r#"location.trace("core/daylight")"#).unwrap();
        assert_eq!(classify(&expr), LocalityClass::Place);
    }

    #[test]
    fn target_niche_is_place() {
        let expr = parse_predicate(r#"target.has_niche("core/chest")"#).unwrap();
        assert_eq!(classify(&expr), LocalityClass::Place);
    }

    #[test]
    fn world_subject_is_global() {
        let expr = parse_predicate(r#"world.state("storming") == true"#).unwrap();
        assert_eq!(classify(&expr), LocalityClass::Global);
    }

    #[test]
    fn actor_stat_read_is_local() {
        // actor.state reads the subject actor's own field → Local
        let expr = parse_predicate(r#"actor.state("healthy") == true"#).unwrap();
        assert_eq!(classify(&expr), LocalityClass::Local);
    }

    #[test]
    fn and_of_actor_and_location_is_place() {
        let expr = parse_predicate(
            r#"actor.has_capability("core/walk") && location.trace("core/daylight")"#,
        )
        .unwrap();
        assert_eq!(classify(&expr), LocalityClass::Place);
    }

    #[test]
    fn reaction_trigger_event_read_is_local() {
        // §8.4 trigger: `$event.*` reads the fixed per-event intent facts (no
        // scan) → bounded Local under the trigger variant, even though the same
        // expression is Global under the eligibility/affordance `classify`.
        let expr = parse_predicate(r#"$event.kind == "r_move" && $event.detail.on_exit == "true""#)
            .unwrap();
        assert_eq!(classify_reaction_trigger(&expr), LocalityClass::Local);
        assert_eq!(classify(&expr), LocalityClass::Global);
    }

    #[test]
    fn reads_target_subject_true_for_target_stat_read() {
        // The exact shape D9 exists to catch: a resolution predicate reading
        // `target.stat(...)` (the pre-amendment AC doorman) must not let an
        // actor-provided template's untargeted variant materialise.
        let expr = parse_predicate(r#"target.stat("srd/ac") >= 1"#).unwrap();
        assert!(reads_target_subject(&expr));
    }

    #[test]
    fn reads_target_subject_false_for_actor_only_predicate() {
        let expr = parse_predicate(r#"actor.has_capability("srd/action/attack")"#).unwrap();
        assert!(!reads_target_subject(&expr));
    }

    #[test]
    fn reads_target_subject_false_for_pure_stochastic_check() {
        // A d20-vs-flat-DC resolution predicate with no target read at all —
        // exactly the shape an untargeted swing's resolution should have.
        let expr = parse_predicate(r#"d20 >= 15"#).unwrap();
        assert!(!reads_target_subject(&expr));
    }

    #[test]
    fn reads_target_subject_false_for_location_read() {
        // Location classifies `Place` too (same locality bound as Target) but
        // is NOT a target read — D9 must distinguish the two, unlike `classify`.
        let expr = parse_predicate(r#"location.trace("core/daylight")"#).unwrap();
        assert!(!reads_target_subject(&expr));
    }

    #[test]
    fn reads_target_subject_true_when_nested_in_compare() {
        let expr =
            parse_predicate(r#"(actor.stat("srd/hp") - target.stat("srd/ac")) > 0"#).unwrap();
        assert!(reads_target_subject(&expr));
    }

    #[test]
    fn reads_target_subject_true_for_shares_tag_second_subject() {
        let expr = parse_predicate(r#"shares_tag(actor, target, "actor.language.")"#).unwrap();
        assert!(reads_target_subject(&expr));
    }

    #[test]
    fn reaction_trigger_world_read_is_still_global() {
        // The trigger bound is otherwise identical: reading unbounded world
        // state is still Global (rejected by MF-VAL-239), `$event.*` aside.
        let expr = parse_predicate(r#"world.state("storming") == true"#).unwrap();
        assert_eq!(classify_reaction_trigger(&expr), LocalityClass::Global);
    }
}
