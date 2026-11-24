//! Runtime evaluator for canonical DSL expressions.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    validate_expression_for_insert, ArithmeticOp, CompareOp, Dice, EvaluationRng, EventRef,
    Expression, ExpressionKind, Node, NodeId, Op, SourceSpan, SubjectRef, ValidationError,
    ValueType,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectBoolFact {
    pub subject: SubjectRef,
    pub field_id: String,
    pub value: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectIntFact {
    pub subject: SubjectRef,
    pub field_id: String,
    pub value: i64,
}

/// #30 / sk-ct8.2: the EFFECTIVE per-entity resource bound for one
/// `(subject, resource_definition_id)` pair — an `entity_resource_bound`
/// override else the bundle-wide `resource_definition` bounds. Both
/// `Op::ReadResourceMax` and `Op::ReadResourceMin` read from the SAME fact
/// (one populator call per subject-resource pair already yields both
/// values), picking `max_value`/`min_value` respectively.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectResourceBoundFact {
    pub subject: SubjectRef,
    pub field_id: String,
    pub min_value: i64,
    pub max_value: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectTextFact {
    pub subject: SubjectRef,
    pub field_id: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectIdFact {
    pub subject: SubjectRef,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingFact {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollTableEntryFact {
    pub table_id: String,
    pub entry_id: String,
    pub weight: u32,
    pub result_ref: String,
}

/// sk-84i.2 (widened G-1): one authored DSL-source slot, keyed by
/// `(subject, slot)` — the same subject-parameterised shape every other fact
/// pool uses (`SubjectIntFact`, `SubjectTextFact`, …). Today the only
/// production populator is the actor's WIELDED item's `entity_definition`
/// (`entity_definition_item_dsl_source`, subject `ActorWieldedItem`,
/// pre-fetched via `resolve_wielded_item_entity`) — same "caller pre-fetches,
/// evaluator looks up by key" shape as `roll_table_entries`. A natural
/// attack / target-worn-armor / off-hand source is the same shape under a
/// different `subject`; wiring that population is sk-84i.4, not this PR. No
/// match for `(subject, slot)` means "nothing there" — `Op::DslSource` fails
/// closed either way (ADR-0035); see `EvaluationError::NoItemDslSourceFacts`
/// / `MissingItemDslSource` for the two distinct fail-closed shapes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemDslSourceFact {
    pub subject: SubjectRef,
    pub slot: String,
    /// Commit-time macro-expanded AST. Act time evaluates this tree; it
    /// never re-parses authored source (#2218).
    pub expr: Expression,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImpliedByCountFact {
    pub condition_definition_id: String,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParticipantTagGroupCountFact {
    pub prefix: String,
    pub excluding_condition_definition_id: Option<String>,
    pub count: i64,
}

/// K-A Fact 2: pre-populated fact for
/// `Op::PeersNearTarget`, keyed by the exact `(prefix, radius_mm,
/// excluding_condition_definition_id)` triple the authored call names —
/// same discipline as `ParticipantTagGroupCountFact`. The evaluator never
/// computes geometry or reads a database table; the server-side pre-scan
/// walker (`collect_peers_near_target_refs`) scopes population to exactly
/// the triples an expression literally references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeersNearTargetFact {
    pub prefix: String,
    pub radius_mm: i64,
    pub excluding_condition_definition_id: Option<String>,
    pub count: i64,
}

/// ADR-0134 D-3: pre-populated fact for `Op::OccupantsAt`, keyed by the
/// exact `(scope, excluding_condition_definition_id)` pair the authored call
/// names — same discipline as `PeersNearTargetFact`. The kernel populates
/// the scopes the current path knows (`to_node` / `from_node` on the
/// traversal path, `target` on the manipulate / resolution path); the
/// evaluator never reads a database table, and a pair the path did not
/// populate is an evaluation error, never a silent zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OccupantsAtFact {
    pub scope: String,
    pub excluding_condition_definition_id: Option<String>,
    pub count: i64,
}

/// Pre-populated fact for `Op::ChannelPool` (sk-ct8.4 / finding #86): the
/// authored `pool_ms_per_round` constant of one `economy_channel_definitions`
/// row, keyed by its literal `channel_id`. The evaluator never reads a
/// database table — callers scope-load only the channel ids their expression
/// literally references (`collect_channel_pool_refs`, server side).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelPoolFact {
    pub channel_id: String,
    pub pool_ms_per_round: i64,
}

/// Composed modifier bias installed into the evaluation context by the condition gate path.
/// Applied by `Roll` (single-die only), `RollUnder`, and `RollTable Int` arms —
/// `flat_bonus` shifts the raw draw, `roll_modifier` re-rolls and takes max
/// (Advantage) or min (Disadvantage).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollModifier {
    Advantage,
    Disadvantage,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComposedRollBias {
    pub flat_bonus: i64,
    pub roll_modifier: Option<RollModifier>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaluationContext {
    pub bool_fields: Vec<SubjectBoolFact>,
    pub int_fields: Vec<SubjectIntFact>,
    /// Effective-int overlay: `(subject, field_id) -> value` after modifier
    /// composition has been folded into the base `int_fields` value.
    /// Read EXCLUSIVELY by `Op::ReadEffectiveInt` (`actor.effective("x")`),
    /// which prefers the overlay and falls back to `int_fields` (base) when a
    /// target is absent. `Op::ReadInt` (`actor.stat(...)`) NEVER consults this
    /// overlay — it reads base only (the ADR-0035 zirkularity cut, now a
    /// structural guarantee). Empty by default, so `effective` falls back to base.
    pub effective_int_fields: Vec<SubjectIntFact>,
    /// #30 / sk-ct8.2: per-subject resource-bound facts, read by
    /// `Op::ReadResourceMax`/`Op::ReadResourceMin` (`actor.resource_max(...)`
    /// / `actor.resource_min(...)`). Empty by default — a subject with no
    /// populated resource-bound facts and a bare `resource_max`/`resource_min`
    /// read resolves to `UnknownField` (ADR-0035 D-4 missing-fact), same as
    /// every other read op.
    pub resource_bounds: Vec<SubjectResourceBoundFact>,
    pub text_fields: Vec<SubjectTextFact>,
    pub capabilities: Vec<SubjectIdFact>,
    pub items: Vec<SubjectIdFact>,
    pub niches: Vec<SubjectIdFact>,
    /// Authored tags carried by each subject — the union of the subject entity's
    /// instance `entity_tag` rows and its definition `entity_definition_tag`
    /// rows, populated by the caller (CR-7 / ADR-0038 D-3). `Op::HasTag` reads
    /// this pool; empty by default → `has_tag` is false (ADR-0035 missing-fact).
    pub tags: Vec<SubjectIdFact>,
    pub statuses: Vec<SubjectIdFact>,
    pub traces: Vec<SubjectIdFact>,
    /// Entities known by the observer, populated from `observer_knowledge`. (Item 16)
    pub known_entities: Vec<SubjectIdFact>,
    pub bindings: Vec<BindingFact>,
    pub roll_table_entries: Vec<RollTableEntryFact>,
    /// sk-84i.2 (widened G-1): authored DSL-source slots, consulted by
    /// `Op::DslSource` (`<subject>.source('slot')`). Empty by default — see
    /// `ItemDslSourceFact`.
    pub item_dsl_sources: Vec<ItemDslSourceFact>,
    /// Pre-populated by kernel callers for `implied_by_count`; empty default
    /// makes the operator safe outside remove-policy evaluation.
    pub implied_by_counts: Vec<ImpliedByCountFact>,
    pub participant_tag_group_counts: Vec<ParticipantTagGroupCountFact>,
    /// K-A Fact 2: pre-populated by kernel callers for `peers_near_target`;
    /// empty default makes the operator fail closed (`EvaluationError::UnknownField`)
    /// outside a scope that pre-loaded the referenced triple.
    pub peers_near_target: Vec<PeersNearTargetFact>,
    /// ADR-0134 D-3: pre-populated by kernel callers for `occupants_at`; empty
    /// default makes the operator fail closed (`EvaluationError::UnknownField`)
    /// outside a scope that pre-loaded the referenced `(scope, condition)` pair.
    pub occupants_at: Vec<OccupantsAtFact>,
    /// Pre-populated by kernel callers for `channel_pool`; empty default makes
    /// the operator fail closed (`EvaluationError::UnknownField`) outside a
    /// scope that pre-loaded the referenced channel(s).
    pub channel_pools: Vec<ChannelPoolFact>,
    pub event_kind: Option<String>,
    /// Composed modifier bias from active condition gates. Consulted by
    /// `Roll` (single-die only), `RollUnder`, and `RollTable Int` eval arms
    /// to apply §5.19 composition output to the predicate roll.
    pub roll_modifier_bias: Option<ComposedRollBias>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluationLimits {
    pub max_depth: u32,
    pub max_nodes: u32,
    pub max_draws: u32,
}

pub const MAX_DRAW_CEILING: u32 = 256;

impl Default for EvaluationLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_nodes: 512,
            max_draws: MAX_DRAW_CEILING,
        }
    }
}

impl EvaluationLimits {
    /// Returns a tight stochastic-draw budget for this expression, clamped to the
    /// finite ceiling required by ADR-0059.
    ///
    /// The `x2` headroom keeps the budget a sound upper bound when an active
    /// roll modifier causes `RollUnder`, an integer `RollTable` node, or a
    /// single-die (`count == 1`) `Roll` node (battle-test finding #23) to be
    /// evaluated twice.
    pub fn for_expression(expr: &Expression) -> Self {
        let default = Self::default();
        Self {
            max_draws: count_static_draws(expr)
                .saturating_mul(2)
                .min(MAX_DRAW_CEILING),
            ..default
        }
    }
}

pub fn count_static_draws(expr: &Expression) -> u32 {
    expr.nodes.iter().fold(0u32, |draws, node| match &node.op {
        Op::Roll(op) => draws.saturating_add(op.dice.count),
        Op::RollTable(_) => draws.saturating_add(1),
        // ADR-0059 (sk-84i.2, widened G-1): `<subject>.source(slot)`
        // evaluates a DATA-DEFINED sub-expression — an authored
        // `entity_definition_item_dsl_source` row — that this STATIC walk
        // cannot see into (the formula lives in a row, not this AST). Rather
        // than under-count (and reject legitimate multi-die item formulas
        // like `2d8` once the ×2 headroom in `EvaluationLimits::for_expression`
        // is applied to a too-small base), charge the node the full
        // un-doubled `MAX_DRAW_CEILING`: `for_expression`'s
        // `.min(MAX_DRAW_CEILING)` clamp then saturates the budget to the
        // ceiling whenever a `DslSource` node is present anywhere in the
        // expression. This makes the budget LOOSE for these expressions
        // (same as `EvaluationLimits::default()`) rather than tight — the
        // actual runtime draw count is still hard-capped by
        // `MAX_DRAW_CEILING` via `eval_dsl_source`'s shared `draws_used`
        // counter, so the runaway/DoS guard the ADR exists to preserve holds
        // even though the static estimate is maximally loose for this one
        // node shape. Honest cost (canon-warden A-5): a bomb in the
        // CALLER's OWN expression that also contains a `DslSource` node is
        // therefore caught only by the 256 runtime cap, not by the tight
        // static bound — `for_expression` degrades to `default()` for any
        // expression containing this node.
        Op::DslSource(_) => draws.saturating_add(MAX_DRAW_CEILING),
        _ => draws,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvaluationError {
    Validation(ValidationError),
    MissingNode {
        node_id: NodeId,
    },
    TypeMismatch {
        node_id: NodeId,
        expected: ValueType,
        actual: ValueType,
    },
    UnknownField {
        subject: SubjectRef,
        field_id: String,
    },
    DivisionByZero {
        node_id: NodeId,
    },
    DepthLimitExceeded {
        node_id: NodeId,
        max_depth: u32,
    },
    NodeLimitExceeded {
        actual: u32,
        max_nodes: u32,
    },
    DrawLimitExceeded {
        attempted: u32,
        max_draws: u32,
    },
    /// The caller's pre-drawn replay stream ran dry before the expression
    /// finished drawing (ISS1627) — distinct from `DrawLimitExceeded`, which
    /// fires when the configured `max_draws` ceiling itself is exceeded.
    /// This fires even comfortably under that ceiling, whenever a caller
    /// under-provisioned `DeterministicRng`'s draw vector relative to what
    /// the expression actually consumes.
    DrawStreamExhausted {
        node_id: NodeId,
        attempted: u32,
    },
    CycleDetected {
        node_id: NodeId,
    },
    UnsupportedInPredicate {
        node_id: NodeId,
    },
    EmptyRollTable {
        node_id: NodeId,
        table_id: String,
    },
    NonUniformRollTableIntEntry {
        node_id: NodeId,
        table_id: String,
        entry_id: String,
        result_ref: String,
    },
    UnknownBinding {
        binding: String,
    },
    /// sk-84i.2 fail-closed (ADR-0035, split canon-warden D-1): the
    /// evaluation context carries NO `item_dsl_sources` facts at all. This is
    /// the shape a call site that never wires this fact family produces
    /// (a kernel wiring gap, e.g. an `EvaluationContext` built without
    /// routing through `manipulation_facts`/`build_actor_subject_facts`) —
    /// and it is ALSO what "genuinely nothing wielded" produces at a wired
    /// call site, since the wielded-item populator only ever adds facts when
    /// an item is resolved. The kernel cannot distinguish these two causes
    /// from emptiness alone; this variant names the more general, honest
    /// condition rather than asserting either cause. Distinct from
    /// `MissingItemDslSource` (facts ARE populated, this particular
    /// `(subject, slot)` just isn't among them) so an operator reading the
    /// JSONL error can tell "the pool is empty" apart from "populated, but
    /// no match for this subject/slot".
    NoItemDslSourceFacts {
        node_id: NodeId,
        subject: SubjectRef,
        slot: String,
    },
    /// sk-84i.2 fail-closed (ADR-0035): `item_dsl_sources` is non-empty (this
    /// call site DOES wire the fact pool) but carries no `(subject, slot)`
    /// match — genuinely nothing wielded/worn/present for this subject, or
    /// the entity_definition carries no `entity_definition_item_dsl_source`
    /// row for `slot`. Same missing-fact shape as `RollTable`'s
    /// `ReadText`-resolved missing pointer — never a panic, never a silent
    /// zero.
    MissingItemDslSource {
        node_id: NodeId,
        subject: SubjectRef,
        slot: String,
    },
    /// sk-84i.2 recursion guard: an item's OWN source slot tried to call
    /// `.source(...)` again. Depth-limited to 1 — fails closed rather than
    /// evaluating unboundedly deep.
    DslSourceRecursionLimit {
        node_id: NodeId,
        subject: SubjectRef,
        slot: String,
    },
}

impl std::fmt::Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(err) => write!(f, "Validation error: {:?}", err),
            Self::MissingNode { node_id } => write!(f, "Missing node: {}", node_id),
            Self::TypeMismatch {
                node_id,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "Type mismatch at node {}: expected {:?}, got {:?}",
                    node_id, expected, actual
                )
            }
            Self::UnknownField { subject, field_id } => {
                write!(f, "Unknown field {:?}.{}", subject, field_id)
            }
            Self::DivisionByZero { node_id } => write!(f, "Division by zero at node {}", node_id),
            Self::DepthLimitExceeded { node_id, max_depth } => {
                write!(
                    f,
                    "Depth limit exceeded at node {} (max {})",
                    node_id, max_depth
                )
            }
            Self::NodeLimitExceeded { actual, max_nodes } => {
                write!(
                    f,
                    "Node limit exceeded: actual {} (max {})",
                    actual, max_nodes
                )
            }
            Self::DrawLimitExceeded {
                attempted,
                max_draws,
            } => {
                write!(
                    f,
                    "Draw limit exceeded: attempted {} (max {})",
                    attempted, max_draws
                )
            }
            Self::DrawStreamExhausted { node_id, attempted } => {
                write!(
                    f,
                    "Draw stream exhausted at node {} (attempted {})",
                    node_id, attempted
                )
            }
            Self::CycleDetected { node_id } => write!(f, "Cycle detected at node {}", node_id),
            Self::UnsupportedInPredicate { node_id } => {
                write!(f, "Unsupported operator in predicate at node {}", node_id)
            }
            Self::EmptyRollTable { node_id, table_id } => {
                write!(f, "Empty roll table at node {}: {}", node_id, table_id)
            }
            Self::NonUniformRollTableIntEntry {
                node_id,
                table_id,
                entry_id,
                result_ref,
            } => {
                write!(
                    f,
                    "Non-uniform roll table int entry at node {}: {}/{} -> {}",
                    node_id, table_id, entry_id, result_ref
                )
            }
            Self::UnknownBinding { binding } => write!(f, "Unknown binding: {}", binding),
            Self::NoItemDslSourceFacts {
                node_id,
                subject,
                slot,
            } => {
                write!(
                    f,
                    "No item DSL source facts for {:?} slot {} at node {}",
                    subject, slot, node_id
                )
            }
            Self::MissingItemDslSource {
                node_id,
                subject,
                slot,
            } => {
                write!(
                    f,
                    "Missing item DSL source for {:?} slot {} at node {}",
                    subject, slot, node_id
                )
            }
            Self::DslSourceRecursionLimit {
                node_id,
                subject,
                slot,
            } => {
                write!(
                    f,
                    "DSL source recursion limit exceeded for {:?} slot {} at node {}",
                    subject, slot, node_id
                )
            }
        }
    }
}

impl std::error::Error for EvaluationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RollTableIntValidationError {
    EmptyTable {
        node_id: NodeId,
        table_id: String,
    },
    NonUniformEntry {
        node_id: NodeId,
        table_id: String,
        entry_id: String,
        result_ref: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StochasticDraw {
    pub source_id: String,
    pub draw_index: u32,
    pub value: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateEvaluation {
    pub value: bool,
    pub draws_used: u32,
    pub stochastic_draws: Vec<StochasticDraw>,
}

/// Typed value produced by a traced expression evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum EvaluationValue {
    Bool(bool),
    Int(i64),
    Text(String),
}

impl EvaluationValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn value_type(&self) -> ValueType {
        match self {
            Self::Bool(_) => ValueType::Bool,
            Self::Int(_) => ValueType::Int,
            Self::Text(_) => ValueType::Text,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TraceScopeSegment {
    pub subject: SubjectRef,
    pub slot: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RollTableSelection {
    pub entry_id: String,
    pub kept: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TraceEntry {
    pub node_id: NodeId,
    pub operator: Option<String>,
    pub depth: u32,
    pub span: Option<SourceSpan>,
    pub scope: Vec<TraceScopeSegment>,
    pub value: Option<EvaluationValue>,
    pub error: Option<String>,
    pub table_entries: Option<Vec<RollTableSelection>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TracedEvaluation {
    pub value: EvaluationValue,
    pub draws_used: u32,
    pub stochastic_draws: Vec<StochasticDraw>,
    pub trace: Vec<TraceEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TracedEvaluationError {
    pub error: EvaluationError,
    pub trace: Vec<TraceEntry>,
}

impl std::fmt::Display for TracedEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

impl std::error::Error for TracedEvaluationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
enum RuntimeValue {
    Bool(bool),
    Int(i64),
    Text(String),
}

impl From<RuntimeValue> for EvaluationValue {
    fn from(val: RuntimeValue) -> Self {
        match val {
            RuntimeValue::Bool(b) => Self::Bool(b),
            RuntimeValue::Int(i) => Self::Int(i),
            RuntimeValue::Text(s) => Self::Text(s),
        }
    }
}

/// Evaluate a validated expression of any root type (Bool, Int, Text) and return
/// its typed value, stochastic draws, and an evaluation trace.
///
/// This entry point is for tooling and offline bench testing. The server never
/// calls it.
pub fn evaluate_expression_traced<R: EvaluationRng>(
    expr: &Expression,
    facts: &EvaluationContext,
    rng: &mut R,
    limits: EvaluationLimits,
) -> Result<TracedEvaluation, TracedEvaluationError> {
    if let Err(err) = validate_expression_for_insert(expr, expr.kind) {
        return Err(TracedEvaluationError {
            error: EvaluationError::Validation(err),
            trace: Vec::new(),
        });
    }

    if expr.nodes.len() as u32 > limits.max_nodes {
        return Err(TracedEvaluationError {
            error: EvaluationError::NodeLimitExceeded {
                actual: expr.nodes.len() as u32,
                max_nodes: limits.max_nodes,
            },
            trace: Vec::new(),
        });
    }

    let mut state = EvaluationState {
        expr,
        facts,
        rng,
        limits,
        draws_used: 0,
        stochastic_draws: Vec::new(),
        active_nodes: BTreeSet::new(),
        lexical_bindings: Vec::new(),
        suppress_roll_bias: false,
        offer_satisfiable_mode: false,
        stochastic_bindings: BTreeSet::new(),
        dsl_source_depth: 0,
        trace: Some(Vec::new()),
        current_scope: Vec::new(),
        pending_roll_table_selections: None,
    };

    match state.eval_node(expr.root_node_id, 1) {
        Ok(runtime_val) => Ok(TracedEvaluation {
            value: EvaluationValue::from(runtime_val),
            draws_used: state.draws_used,
            stochastic_draws: state.stochastic_draws,
            trace: state.trace.unwrap_or_default(),
        }),
        Err(error) => Err(TracedEvaluationError {
            error,
            trace: state.trace.unwrap_or_default(),
        }),
    }
}

/// Evaluate a validated predicate expression from typed AST data.
///
/// This is the reducer-facing hot-path API: callers pass the canonical
/// `Expression` row data and never source text. `parse_predicate_authoring`
/// remains an authoring / row-insert validation helper.
pub fn evaluate_predicate_expression<R: EvaluationRng>(
    expr: &Expression,
    facts: &EvaluationContext,
    rng: &mut R,
    limits: EvaluationLimits,
) -> Result<bool, EvaluationError> {
    evaluate_predicate_expression_with_report(expr, facts, rng, limits).map(|result| result.value)
}

/// Evaluate an Outcome-kind expression and return its integer value.
///
/// Used by the condition gate path to evaluate `modifier_expression` fields
/// on `ModifierTemplate` rows. Callers extract `delta_kind` from tags and
/// use this value as the delta payload.
pub fn evaluate_int_expression<R: EvaluationRng>(
    expr: &Expression,
    facts: &EvaluationContext,
    rng: &mut R,
    limits: EvaluationLimits,
) -> Result<i64, EvaluationError> {
    validate_expression_for_insert(expr, ExpressionKind::Outcome)
        .map_err(EvaluationError::Validation)?;
    if expr.nodes.len() as u32 > limits.max_nodes {
        return Err(EvaluationError::NodeLimitExceeded {
            actual: expr.nodes.len() as u32,
            max_nodes: limits.max_nodes,
        });
    }
    let mut state = EvaluationState {
        expr,
        facts,
        rng,
        limits,
        draws_used: 0,
        stochastic_draws: Vec::new(),
        active_nodes: BTreeSet::new(),
        lexical_bindings: Vec::new(),
        suppress_roll_bias: false,
        offer_satisfiable_mode: false,
        stochastic_bindings: BTreeSet::new(),
        dsl_source_depth: 0,
        trace: None,
        current_scope: Vec::new(),
        pending_roll_table_selections: None,
    };
    state.eval_int(expr.root_node_id, 1)
}

pub fn evaluate_predicate_expression_with_report<R: EvaluationRng>(
    expr: &Expression,
    facts: &EvaluationContext,
    rng: &mut R,
    limits: EvaluationLimits,
) -> Result<PredicateEvaluation, EvaluationError> {
    validate_expression_for_insert(expr, ExpressionKind::Predicate)
        .map_err(EvaluationError::Validation)?;
    if expr.nodes.len() as u32 > limits.max_nodes {
        return Err(EvaluationError::NodeLimitExceeded {
            actual: expr.nodes.len() as u32,
            max_nodes: limits.max_nodes,
        });
    }

    let mut state = EvaluationState {
        expr,
        facts,
        rng,
        limits,
        draws_used: 0,
        stochastic_draws: Vec::new(),
        active_nodes: BTreeSet::new(),
        lexical_bindings: Vec::new(),
        suppress_roll_bias: false,
        offer_satisfiable_mode: false,
        stochastic_bindings: BTreeSet::new(),
        dsl_source_depth: 0,
        trace: None,
        current_scope: Vec::new(),
        pending_roll_table_selections: None,
    };
    let value = state.eval_bool(expr.root_node_id, 1)?;
    Ok(PredicateEvaluation {
        value,
        draws_used: state.draws_used,
        stochastic_draws: state.stochastic_draws,
    })
}

/// Evaluate a predicate for OFFER/materialise-time attemptability (#1891).
///
/// The materialiser decides WHETHER to advertise an affordance — never
/// whether it will succeed. Deterministic conjuncts (capability gates,
/// tag/state reads, target-state gates like `target.stat('srd/ac') >= 1`)
/// are evaluated normally against `facts`. Any stochastic COMPARISON term —
/// an `Op::Compare` whose operand subtree contains a `Roll`/`RollTable` draw,
/// or an `Op::RollUnder` (which is a draw by construction) — is treated as
/// SATISFIED without drawing entropy: attemptability must never hinge on a
/// die. The resolution expression owns the stochastic part of the check.
///
/// No `EvaluationRng` is accepted: this function draws NOTHING, structurally
/// (every stochastic-draw-producing Op variant valid in a `Predicate`
/// expression — `Roll`, `RollTable` — only ever reaches a boolean result
/// through `Compare` or `RollUnder`, both short-circuited below before their
/// operands are visited; a `Let`-bound value with a stochastic subtree is
/// DEFERRED by `Op::Let` itself rather than evaluated, so a die laundered
/// through a binding name — invisible to a purely structural AST walk,
/// since a binding is resolved by name at eval time, not by a node-id edge
/// — never reaches the rng either, see `subtree_has_stochastic_draw`). A
/// caller with a fully deterministic predicate gets byte-identical
/// behaviour to `evaluate_predicate_expression` minus the (never-drawn)
/// `rng` plumbing.
///
/// This is deliberately crude at the extremes: `d20 >= 30` (mechanically
/// impossible on any real d20) still offers, same as `d20 >= 9`, because the
/// function never inspects the threshold — only "does this comparison draw
/// at all". That is intentional (operator direction, #1891): attemptability
/// is never a function of how LIKELY the die is to land right, only of
/// whether the deterministic gates around it hold; a predicate authored with
/// an unreachable DC is a content bug for authoring lint to catch, not
/// something this kernel-level fix is scoped to detect.
///
/// An offered action can still fail when its resolution predicate runs.
pub fn evaluate_offer_predicate_expression(
    expr: &Expression,
    facts: &EvaluationContext,
    limits: EvaluationLimits,
) -> Result<bool, EvaluationError> {
    validate_expression_for_insert(expr, ExpressionKind::Predicate)
        .map_err(EvaluationError::Validation)?;
    if expr.nodes.len() as u32 > limits.max_nodes {
        return Err(EvaluationError::NodeLimitExceeded {
            actual: expr.nodes.len() as u32,
            max_nodes: limits.max_nodes,
        });
    }

    let mut rng = NeverDrawnRng;
    let mut state = EvaluationState {
        expr,
        facts,
        rng: &mut rng,
        limits,
        draws_used: 0,
        stochastic_draws: Vec::new(),
        active_nodes: BTreeSet::new(),
        lexical_bindings: Vec::new(),
        suppress_roll_bias: false,
        offer_satisfiable_mode: true,
        stochastic_bindings: BTreeSet::new(),
        dsl_source_depth: 0,
        trace: None,
        current_scope: Vec::new(),
        pending_roll_table_selections: None,
    };
    state.eval_bool(expr.root_node_id, 1)
}

/// Never-called `EvaluationRng`: `evaluate_offer_predicate_expression` never
/// reaches a `Roll`/`RollTable` node (`Compare`, `RollUnder`, and `Let` all
/// short-circuit/defer a stochastic subtree first in offer-satisfiable
/// mode — see `subtree_has_stochastic_draw` and the `Op::Let` arm below), so
/// this panics rather than silently returning a fixed value if that
/// invariant is ever broken by a future edit — a loud failure in tests beats
/// a quietly-wrong offer.
struct NeverDrawnRng;

impl EvaluationRng for NeverDrawnRng {
    fn next_u32(&mut self) -> u32 {
        unreachable!(
            "evaluate_offer_predicate_expression must never draw: Compare/RollUnder/Let \
             short-circuit or defer stochastic subtrees before any Roll/RollTable node is visited"
        )
    }
}

/// True if `node_id`'s subtree (inclusive) contains a stochastic draw
/// (`Op::Roll`/`Op::RollTable`), OR reads a `Let`-bound name already present
/// in `stochastic_bindings`.
///
/// #1891 canon-warden follow-up: a die laundered through
/// `let roll = d20; roll >= 15` is NOT structurally reachable from the
/// consuming `Compare`'s operand via `Op::child_nodes()` — a binding read
/// (`Op::ReadInt`/`Op::ReadBool`/`Op::ReadText`/`Op::ReadEffectiveInt` with
/// `subject: SubjectRef::Binding(name)`) is resolved by NAME at eval time
/// (`EvaluationState::lexical_binding_value`), not by a node-id edge — so a
/// purely-structural walk from the Compare's operand missed it and evaluated
/// straight through to a live draw (a reducer-aborting panic under
/// `NeverDrawnRng`, or silently-wrong live entropy under the real evaluator).
/// `stochastic_bindings` is threaded in by `Op::Let`'s own offer-mode
/// handling below, which — for exactly this reason — DEFERS (never
/// evaluates, never draws) a stochastic-valued binding instead of drawing,
/// and records its name here so every downstream reference is recognised.
///
/// Otherwise walks via the existing generic `Op::child_nodes()` (already
/// used by `validate_expression_for_insert`), so it covers every nesting
/// shape (`Cond` branches, `Arithmetic`, chained `Let`s, …) without a
/// per-variant enumeration here.
fn subtree_has_stochastic_draw(
    expr: &Expression,
    node_id: NodeId,
    stochastic_bindings: &BTreeSet<String>,
) -> bool {
    let Some(node) = expr.node(node_id) else {
        return false;
    };
    match &node.op {
        // sk-84i.2 (widened G-1): `<subject>.source(...)` splices in a
        // data-driven sub-expression that MAY itself draw (e.g. an item's
        // `srd/damage` slot is `2d8`) — treated as stochastic
        // unconditionally, same as `RollTable` (whose own draw count is
        // likewise invisible to a purely-structural walk over THIS
        // expression's nodes).
        Op::Roll(_) | Op::RollTable(_) | Op::DslSource(_) => return true,
        Op::ReadInt(read)
        | Op::ReadEffectiveInt(read)
        | Op::ReadBool(read)
        | Op::ReadText(read)
        | Op::ReadResourceMax(read)
        | Op::ReadResourceMin(read) => {
            if let SubjectRef::Binding(binding) = &read.subject {
                if stochastic_bindings.contains(&binding.name) {
                    return true;
                }
            }
        }
        _ => {}
    }
    node.op
        .child_nodes()
        .iter()
        .any(|&child| subtree_has_stochastic_draw(expr, child, stochastic_bindings))
}

/// The literal table id of a `roll_table(...)` argument, when the argument is a
/// `TextLiteral` node. Returns `None` for a runtime `ReadText` table id (Option
/// C, G3) — that case is validated at runtime (fail-closed, ADR-0035), not
/// statically.
fn literal_text_node(expr: &Expression, node_id: NodeId) -> Option<String> {
    match expr.node(node_id).map(|node| &node.op) {
        Some(Op::TextLiteral(lit)) => Some(lit.value.clone()),
        _ => None,
    }
}

pub fn validate_int_roll_table_usage(
    expr: &Expression,
    entries: &[RollTableEntryFact],
) -> Result<(), RollTableIntValidationError> {
    for node in &expr.nodes {
        let Op::RollTable(op) = &node.op else {
            continue;
        };
        if node.value_type != ValueType::Int {
            continue;
        }
        // Option C (G3): static uniform-entry checks apply only to a LITERAL
        // table id; a runtime `ReadText` table id is resolved + drawn at eval,
        // where an empty/dangling table fails closed (ADR-0035).
        let Some(table_id) = literal_text_node(expr, op.table_id_node) else {
            continue;
        };
        let matching: Vec<&RollTableEntryFact> = entries
            .iter()
            .filter(|entry| entry.table_id == table_id && entry.weight > 0)
            .collect();
        if matching.is_empty() {
            return Err(RollTableIntValidationError::EmptyTable {
                node_id: node.node_id,
                table_id: table_id.clone(),
            });
        }
        for entry in matching {
            if entry.result_ref.parse::<i64>().is_err() {
                return Err(RollTableIntValidationError::NonUniformEntry {
                    node_id: node.node_id,
                    table_id: table_id.clone(),
                    entry_id: entry.entry_id.clone(),
                    result_ref: entry.result_ref.clone(),
                });
            }
        }
    }
    Ok(())
}

struct EvaluationState<'a, R: EvaluationRng> {
    expr: &'a Expression,
    facts: &'a EvaluationContext,
    rng: &'a mut R,
    limits: EvaluationLimits,
    draws_used: u32,
    stochastic_draws: Vec<StochasticDraw>,
    active_nodes: BTreeSet<NodeId>,
    lexical_bindings: Vec<(String, RuntimeValue)>,
    /// Battle-test finding #23 guard: `Op::RollUnder` owns bias-application
    /// for its `roll_node` subtree (it decides how many times to draw it and
    /// how to combine the draws) — a nested `Op::Roll` inside that subtree
    /// must draw RAW, or the bias would double-apply (RollUnder's own re-roll
    /// PLUS the nested `Op::Roll`'s own re-roll). Set by `Op::RollUnder`
    /// around its `roll_node` evaluation only; `Op::Roll` consults it before
    /// consulting `roll_modifier_bias` itself. `Op::RollTable`'s Int arm
    /// needs no such guard — its draw (`self.roll_table_int`) never recurses
    /// through `eval_node`, so no nested `Op::Roll` can be reached from it.
    suppress_roll_bias: bool,
    /// #1891: offer/materialise-time attemptability mode. When `true`,
    /// `Op::Compare` and `Op::RollUnder` short-circuit a stochastic operand
    /// to `true` (satisfied) instead of drawing — see
    /// `evaluate_offer_predicate_expression`. `false` for every other
    /// evaluation entry point (act-path resolution keeps drawing real dice).
    offer_satisfiable_mode: bool,
    /// #1891 canon-warden follow-up: names of `Let`-bound values that
    /// `offer_satisfiable_mode` DEFERRED (never evaluated, never drew)
    /// because their value's subtree was itself stochastic. Consulted by
    /// `subtree_has_stochastic_draw` so a later reference to the binding —
    /// by name, not by node-id edge — is recognised as stochastic too.
    /// Scoped like `lexical_bindings`: inserted before evaluating a deferred
    /// `Let`'s body, removed after. Always empty outside offer mode.
    stochastic_bindings: BTreeSet<String>,
    /// sk-84i.2 recursion guard: how many `Op::DslSource` frames are
    /// currently nested. `0` at every top-level entry point; `eval_dsl_source`
    /// rejects with `DslSourceRecursionLimit` at `1` (depth-limited to
    /// exactly one level — an item source may not itself call
    /// `.source(...)` again, fails closed rather than silently no-op'ing).
    dsl_source_depth: u32,
    trace: Option<Vec<TraceEntry>>,
    current_scope: Vec<TraceScopeSegment>,
    pending_roll_table_selections: Option<Vec<RollTableSelection>>,
}

impl<R: EvaluationRng> EvaluationState<'_, R> {
    fn nested_trace_scope(&self, subject: &SubjectRef, slot: &str) -> Vec<TraceScopeSegment> {
        // Vec::new does not allocate. Reducer-facing entry points must not
        // clone subjects or slots merely to evaluate a nested source.
        if self.trace.is_none() {
            return Vec::new();
        }
        let mut scope = self.current_scope.clone();
        scope.push(TraceScopeSegment {
            subject: subject.clone(),
            slot: slot.to_string(),
        });
        scope
    }

    fn push_trace_entry(
        &mut self,
        node_id: NodeId,
        depth: u32,
        result: Result<&RuntimeValue, &EvaluationError>,
    ) {
        if let Some(trace) = self.trace.as_mut() {
            let node = self.expr.node(node_id);
            let operator = node.map(|n| n.op.variant_name().to_string());
            let span = node.map(|n| n.span);
            let (value, error) = match result {
                Ok(val) => (Some(EvaluationValue::from((*val).clone())), None),
                Err(err) => (None, Some(err.to_string())),
            };
            let table_entries = self.pending_roll_table_selections.take();
            trace.push(TraceEntry {
                node_id,
                operator,
                depth,
                span,
                scope: self.current_scope.clone(),
                value,
                error,
                table_entries,
            });
        }
    }

    fn eval_bool(&mut self, node_id: NodeId, depth: u32) -> Result<bool, EvaluationError> {
        match self.eval_node(node_id, depth)? {
            RuntimeValue::Bool(value) => Ok(value),
            other => Err(self.type_mismatch(node_id, ValueType::Bool, other)),
        }
    }

    fn eval_int(&mut self, node_id: NodeId, depth: u32) -> Result<i64, EvaluationError> {
        match self.eval_node(node_id, depth)? {
            RuntimeValue::Int(value) => Ok(value),
            other => Err(self.type_mismatch(node_id, ValueType::Int, other)),
        }
    }

    fn eval_node(&mut self, node_id: NodeId, depth: u32) -> Result<RuntimeValue, EvaluationError> {
        if depth > self.limits.max_depth {
            let err = EvaluationError::DepthLimitExceeded {
                node_id,
                max_depth: self.limits.max_depth,
            };
            self.push_trace_entry(node_id, depth, Err(&err));
            return Err(err);
        }
        if !self.active_nodes.insert(node_id) {
            let err = EvaluationError::CycleDetected { node_id };
            self.push_trace_entry(node_id, depth, Err(&err));
            return Err(err);
        }

        let node = match self.expr.node(node_id) {
            Some(node) => node,
            None => {
                let err = EvaluationError::MissingNode { node_id };
                self.push_trace_entry(node_id, depth, Err(&err));
                self.active_nodes.remove(&node_id);
                return Err(err);
            }
        };
        let result = self.eval_node_inner(node, depth);
        self.active_nodes.remove(&node_id);
        self.push_trace_entry(node_id, depth, result.as_ref());
        result
    }

    fn eval_node_inner(
        &mut self,
        node: &Node,
        depth: u32,
    ) -> Result<RuntimeValue, EvaluationError> {
        match &node.op {
            Op::BoolLiteral(op) => Ok(RuntimeValue::Bool(op.value)),
            Op::IntLiteral(op) => Ok(RuntimeValue::Int(op.value)),
            Op::TextLiteral(op) => Ok(RuntimeValue::Text(op.value.clone())),
            Op::ReadBool(op) => {
                // #1891: a bare read of a stochastic `Let` binding (no
                // enclosing Compare/RollUnder to short-circuit it — e.g. a
                // `hit` tail reading a Bool binding directly) would be
                // satisfied in offer mode; `Op::Let` never pushed a real
                // value for it, so falling through would be an UnknownField.
                // Currently unreachable via THIS grammar (`let` values are
                // Int-only — `parse_let_bool`/`parse_let_int` both route
                // through `parse_int_expr`, so `stochastic_bindings` only
                // ever names Int bindings) — kept as defense-in-depth should
                // the grammar ever grow a Bool-valued `let`.
                if self.offer_satisfiable_mode {
                    if let SubjectRef::Binding(binding) = &op.subject {
                        if self.stochastic_bindings.contains(&binding.name) {
                            return Ok(RuntimeValue::Bool(true));
                        }
                    }
                }
                if let Some(value) = self.lexical_binding_value(&op.subject) {
                    return Ok(value);
                }
                self.facts
                    .bool_fields
                    .iter()
                    .find(|fact| fact.subject == op.subject && fact.field_id == op.field_id)
                    .map(|fact| RuntimeValue::Bool(fact.value))
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: op.subject.clone(),
                        field_id: op.field_id.clone(),
                    })
            }
            Op::ReadInt(op) => {
                if let Some(value) = self.lexical_binding_value(&op.subject) {
                    return Ok(value);
                }
                // `stat` reads the BASE value only. The effective overlay is read
                // exclusively by `Op::ReadEffectiveInt` (`actor.effective(...)`);
                // keeping `stat` base preserves the ADR-0035 zirkularity cut so
                // template predicates never gate on modifier-composed stats (#1247).
                self.facts
                    .int_fields
                    .iter()
                    .find(|fact| fact.subject == op.subject && fact.field_id == op.field_id)
                    .map(|fact| RuntimeValue::Int(fact.value))
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: op.subject.clone(),
                        field_id: op.field_id.clone(),
                    })
            }
            Op::ReadResourceMax(op) => {
                if let Some(value) = self.lexical_binding_value(&op.subject) {
                    return Ok(value);
                }
                // #30 / sk-ct8.2: the EFFECTIVE per-entity max bound —
                // `entity_resource_bound` override else the bundle-wide
                // `resource_definition.max_value`, resolved server-side by the
                // ONE populator (`effective_resource_bounds`) that also drives
                // `ModifyResource`'s clamp, so the read and the clamp never
                // diverge.
                self.facts
                    .resource_bounds
                    .iter()
                    .find(|fact| fact.subject == op.subject && fact.field_id == op.field_id)
                    .map(|fact| RuntimeValue::Int(fact.max_value))
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: op.subject.clone(),
                        field_id: op.field_id.clone(),
                    })
            }
            Op::ReadResourceMin(op) => {
                if let Some(value) = self.lexical_binding_value(&op.subject) {
                    return Ok(value);
                }
                // #30 / sk-ct8.2: sibling of `ReadResourceMax` — same fact,
                // `min_value` leg.
                self.facts
                    .resource_bounds
                    .iter()
                    .find(|fact| fact.subject == op.subject && fact.field_id == op.field_id)
                    .map(|fact| RuntimeValue::Int(fact.min_value))
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: op.subject.clone(),
                        field_id: op.field_id.clone(),
                    })
            }
            Op::ReadEffectiveInt(op) => {
                if let Some(value) = self.lexical_binding_value(&op.subject) {
                    return Ok(value);
                }
                // #1247: read the EFFECTIVE value — the composed `effective_int_fields`
                // overlay wins when populated (base + active modifier composition),
                // else fall back to the base fact. The author opts into this read
                // explicitly via `actor.effective("x")`; the kernel composes the
                // equipped numeric overlay in dependency order, refusing cycles before
                // evaluation. Predicate contexts retain ADR-0035's separate
                // base/equipment-only boundary. Other callers supply their
                // own admitted facts; this reader adds no recursive collection.
                if let Some(fact) = self
                    .facts
                    .effective_int_fields
                    .iter()
                    .find(|fact| fact.subject == op.subject && fact.field_id == op.field_id)
                {
                    return Ok(RuntimeValue::Int(fact.value));
                }
                self.facts
                    .int_fields
                    .iter()
                    .find(|fact| fact.subject == op.subject && fact.field_id == op.field_id)
                    .map(|fact| RuntimeValue::Int(fact.value))
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: op.subject.clone(),
                        field_id: op.field_id.clone(),
                    })
            }
            Op::ReadText(op) => {
                if let Some(value) = self.lexical_binding_value(&op.subject) {
                    return Ok(value);
                }
                if let Some(value) = self.event_text_value(&op.subject, &op.field_id) {
                    return Ok(RuntimeValue::Text(value));
                }
                self.facts
                    .text_fields
                    .iter()
                    .find(|fact| fact.subject == op.subject && fact.field_id == op.field_id)
                    .map(|fact| RuntimeValue::Text(fact.value.clone()))
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: op.subject.clone(),
                        field_id: op.field_id.clone(),
                    })
            }
            Op::HasCapability(op) => Ok(RuntimeValue::Bool(has_id(
                &self.facts.capabilities,
                &op.subject,
                &op.capability_id,
            ))),
            Op::HasItem(op) => {
                let item_def_id = self.resolve_binding_value(&op.item_def_id)?;
                Ok(RuntimeValue::Bool(has_id(
                    &self.facts.items,
                    &op.subject,
                    &item_def_id,
                )))
            }
            Op::HasNiche(op) => Ok(RuntimeValue::Bool(has_id(
                &self.facts.niches,
                &op.subject,
                &op.niche_id,
            ))),
            Op::HasTag(op) => Ok(RuntimeValue::Bool(has_id(
                &self.facts.tags,
                &op.subject,
                &op.tag,
            ))),
            Op::SharesTag(op) => Ok(RuntimeValue::Bool(shares_tag(
                &self.facts.tags,
                &op.subject_a,
                &op.subject_b,
                &op.prefix,
            ))),
            Op::HasStatus(op) => Ok(RuntimeValue::Bool(has_id(
                &self.facts.statuses,
                &op.subject,
                &op.condition_definition_id,
            ))),
            Op::ImpliedByCount(op) => {
                let value = match op.condition_definition_id.as_deref() {
                    Some(condition_definition_id) => self
                        .facts
                        .implied_by_counts
                        .iter()
                        .find(|fact| fact.condition_definition_id == condition_definition_id)
                        .map(|fact| fact.count)
                        .unwrap_or(0),
                    None => self
                        .facts
                        .implied_by_counts
                        .first()
                        .map(|fact| fact.count)
                        .unwrap_or(0),
                };
                Ok(RuntimeValue::Int(value))
            }
            Op::ParticipantTagGroupCount(op) => {
                let value = self
                    .facts
                    .participant_tag_group_counts
                    .iter()
                    .find(|fact| {
                        fact.prefix == op.prefix
                            && fact.excluding_condition_definition_id
                                == op.excluding_condition_definition_id
                    })
                    .map(|fact| fact.count)
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: SubjectRef::Session,
                        field_id: format!(
                            "participant_tag_group_count:{}:{:?}",
                            op.prefix, op.excluding_condition_definition_id
                        ),
                    })?;
                Ok(RuntimeValue::Int(value))
            }
            Op::ChannelPool(op) => {
                let value = self
                    .facts
                    .channel_pools
                    .iter()
                    .find(|fact| fact.channel_id == op.channel_id)
                    .map(|fact| fact.pool_ms_per_round)
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: SubjectRef::World,
                        field_id: format!("channel_pool:{}", op.channel_id),
                    })?;
                Ok(RuntimeValue::Int(value))
            }
            Op::PeersNearTarget(op) => {
                let value = self
                    .facts
                    .peers_near_target
                    .iter()
                    .find(|fact| {
                        fact.prefix == op.prefix
                            && fact.radius_mm == op.radius_mm
                            && fact.excluding_condition_definition_id
                                == op.excluding_condition_definition_id
                    })
                    .map(|fact| fact.count)
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: SubjectRef::Target,
                        field_id: format!(
                            "peers_near_target:{}:{}:{:?}",
                            op.prefix, op.radius_mm, op.excluding_condition_definition_id
                        ),
                    })?;
                Ok(RuntimeValue::Int(value))
            }
            Op::OccupantsAt(op) => {
                let value = self
                    .facts
                    .occupants_at
                    .iter()
                    .find(|fact| {
                        fact.scope == op.scope
                            && fact.excluding_condition_definition_id
                                == op.excluding_condition_definition_id
                    })
                    .map(|fact| fact.count)
                    .ok_or_else(|| EvaluationError::UnknownField {
                        subject: SubjectRef::Target,
                        field_id: format!(
                            "occupants_at:{}:{:?}",
                            op.scope, op.excluding_condition_definition_id
                        ),
                    })?;
                Ok(RuntimeValue::Int(value))
            }
            Op::TracePresent(op) => Ok(RuntimeValue::Bool(has_id(
                &self.facts.traces,
                &op.subject,
                &op.trace_kind_id,
            ))),
            Op::KnowsEntity(op) => {
                let entity_id = self.resolve_binding_value(&op.entity_id)?;
                Ok(RuntimeValue::Bool(has_id(
                    &self.facts.known_entities,
                    &op.subject,
                    &entity_id,
                )))
            }
            Op::And(op) => {
                for operand in &op.operands {
                    if !self.eval_bool(*operand, depth + 1)? {
                        return Ok(RuntimeValue::Bool(false));
                    }
                }
                Ok(RuntimeValue::Bool(true))
            }
            Op::Or(op) => {
                for operand in &op.operands {
                    if self.eval_bool(*operand, depth + 1)? {
                        return Ok(RuntimeValue::Bool(true));
                    }
                }
                Ok(RuntimeValue::Bool(false))
            }
            Op::Not(op) => Ok(RuntimeValue::Bool(!self.eval_bool(op.operand, depth + 1)?)),
            Op::Compare(op) => {
                // #1891: offer mode never draws — a stochastic COMPARISON
                // term (either operand's subtree contains a Roll/RollTable,
                // directly OR laundered through a `Let`-bound name recorded
                // in `stochastic_bindings`) is satisfied by construction,
                // without evaluating either side. Deterministic Compares
                // (both operands draw-free) are untouched — same eval path
                // as always.
                if self.offer_satisfiable_mode
                    && (subtree_has_stochastic_draw(self.expr, op.left, &self.stochastic_bindings)
                        || subtree_has_stochastic_draw(
                            self.expr,
                            op.right,
                            &self.stochastic_bindings,
                        ))
                {
                    return Ok(RuntimeValue::Bool(true));
                }
                let left = self.eval_node(op.left, depth + 1)?;
                let right = self.eval_node(op.right, depth + 1)?;
                let value = compare_runtime_values(node.node_id, op.op, left, right)?;
                Ok(RuntimeValue::Bool(value))
            }
            Op::Arithmetic(op) => {
                let left = self.eval_int(op.left, depth + 1)?;
                let right = self.eval_int(op.right, depth + 1)?;
                let value = match op.op {
                    ArithmeticOp::Add => left + right,
                    ArithmeticOp::Sub => left - right,
                    ArithmeticOp::Mul => left * right,
                    ArithmeticOp::Div => {
                        if right == 0 {
                            return Err(EvaluationError::DivisionByZero {
                                node_id: node.node_id,
                            });
                        }
                        left / right
                    }
                    ArithmeticOp::Max => left.max(right),
                    ArithmeticOp::Min => left.min(right),
                };
                Ok(RuntimeValue::Int(value))
            }
            Op::Cond(op) => {
                // Lazy: evaluate the gate, then exactly one branch. The untaken
                // branch (and any roll inside it) is never visited (ADR-0045
                // D-3) — this is the distinct code path no And/Or/Let composes.
                let chosen = if self.eval_bool(op.cond, depth + 1)? {
                    op.then_node
                } else {
                    op.else_node
                };
                Ok(RuntimeValue::Int(self.eval_int(chosen, depth + 1)?))
            }
            Op::Roll(op) => {
                let raw = self.roll(node.node_id, &op.dice)?;
                // Battle-test finding #23: a bare `d20`/`XdN` term (the grammar
                // every check/attack-roll idiom parses its die term to) now
                // consults the SAME `roll_modifier_bias` `Op::RollUnder` /
                // `Op::RollTable` already do — but ONLY for a single-die roll
                // (`dice.count == 1`). Structural invariant, not a game-content
                // rule: a bias installed on a check axis applies to a single
                // sample, never to an aggregated multi-die sum — gating on
                // `count == 1` keeps an `XdN` expression byte-identical even
                // while a bias is active during its evaluation elsewhere in the
                // same predicate or consequence.
                // Direction mirrors `Op::RollTable` (not `Op::RollUnder`): a
                // bare roll is always consumed by a `>= DC`-shaped comparison,
                // so higher wins — Advantage takes the max of two
                // flat-bonus-adjusted draws, Disadvantage the min. Both draws
                // land in `stochastic_draws` independently (same as
                // `RollUnder`/`RollTable`'s own second `self.roll`/
                // `self.roll_table_int` call) — no new provenance field.
                if op.dice.count != 1 || self.suppress_roll_bias {
                    return Ok(RuntimeValue::Int(raw));
                }
                let effective = match self.facts.roll_modifier_bias.as_ref() {
                    None => raw,
                    Some(bias) => {
                        let biased = raw.saturating_add(bias.flat_bonus);
                        match bias.roll_modifier {
                            None => biased,
                            Some(RollModifier::Advantage) => {
                                let raw2 = self.roll(node.node_id, &op.dice)?;
                                biased.max(raw2.saturating_add(bias.flat_bonus))
                            }
                            Some(RollModifier::Disadvantage) => {
                                let raw2 = self.roll(node.node_id, &op.dice)?;
                                biased.min(raw2.saturating_add(bias.flat_bonus))
                            }
                        }
                    }
                };
                Ok(RuntimeValue::Int(effective))
            }
            Op::RollUnder(op) => {
                // #1891: `RollUnder` is a draw by construction — offer mode
                // treats it as satisfied without rolling, same as a
                // stochastic `Compare` above.
                if self.offer_satisfiable_mode {
                    return Ok(RuntimeValue::Bool(true));
                }
                // Battle-test finding #23: `RollUnder` owns bias-application
                // for its `roll_node` subtree end-to-end (it decides how many
                // times to draw and how to combine) — suppress `Op::Roll`'s
                // own (new) bias consultation for every nested evaluation of
                // `roll_node` here, or a bare-`d20`-shaped `roll_node` would
                // double-apply the bias (once via its own re-roll, once via
                // RollUnder's). Restored to the caller's prior value after,
                // not force-reset to `false`, in case this arm is ever
                // reached while already nested under another suppressing arm.
                let previous_suppress_roll_bias = self.suppress_roll_bias;
                self.suppress_roll_bias = true;
                let roll1 = self.eval_int(op.roll_node, depth + 1)?;
                self.suppress_roll_bias = previous_suppress_roll_bias;
                let target = self.eval_int(op.target_node, depth + 1)?;
                let effective_roll = match self.facts.roll_modifier_bias.as_ref() {
                    None => roll1,
                    Some(bias) => {
                        let biased1 = roll1.saturating_add(bias.flat_bonus);
                        match bias.roll_modifier {
                            None => biased1,
                            Some(RollModifier::Advantage) => {
                                // Advantage on RollUnder: lower roll passes more easily → min.
                                self.suppress_roll_bias = true;
                                let roll2 = self.eval_int(op.roll_node, depth + 1)?;
                                self.suppress_roll_bias = previous_suppress_roll_bias;
                                biased1.min(roll2.saturating_add(bias.flat_bonus))
                            }
                            Some(RollModifier::Disadvantage) => {
                                // Disadvantage on RollUnder: higher roll passes less easily → max.
                                self.suppress_roll_bias = true;
                                let roll2 = self.eval_int(op.roll_node, depth + 1)?;
                                self.suppress_roll_bias = previous_suppress_roll_bias;
                                biased1.max(roll2.saturating_add(bias.flat_bonus))
                            }
                        }
                    }
                };
                Ok(RuntimeValue::Bool(effective_roll <= target))
            }
            Op::RollTable(op) if node.value_type == ValueType::Int => {
                // Option C (G3 / #1254): resolve the text-valued table-id node — a
                // `TextLiteral` for a literal `roll_table('lit')`, or a `ReadText`
                // (e.g. the wielded weapon's profile pointer) for a runtime id. A
                // missing pointer propagates `UnknownField` → fails closed
                // (ADR-0035), never a panic. The text read draws no RNG; the table
                // draw stays the static 1 (`count_static_draws` unchanged).
                let table_id = match self.eval_node(op.table_id_node, depth + 1)? {
                    RuntimeValue::Text(s) => s,
                    other => {
                        return Err(self.type_mismatch(op.table_id_node, ValueType::Text, other))
                    }
                };
                let raw = self.roll_table_int(node.node_id, &table_id)?;
                let (effective, kept_index) = match self.facts.roll_modifier_bias.as_ref() {
                    None => (raw, 0),
                    Some(bias) => {
                        let biased = raw.saturating_add(bias.flat_bonus);
                        match bias.roll_modifier {
                            None => (biased, 0),
                            Some(RollModifier::Advantage) => {
                                let raw2 = self.roll_table_int(node.node_id, &table_id)?;
                                let biased2 = raw2.saturating_add(bias.flat_bonus);
                                if biased2 >= biased {
                                    (biased2, 1)
                                } else {
                                    (biased, 0)
                                }
                            }
                            Some(RollModifier::Disadvantage) => {
                                let raw2 = self.roll_table_int(node.node_id, &table_id)?;
                                let biased2 = raw2.saturating_add(bias.flat_bonus);
                                if biased <= biased2 {
                                    (biased, 0)
                                } else {
                                    (biased2, 1)
                                }
                            }
                        }
                    }
                };
                if let Some(selections) = self.pending_roll_table_selections.as_mut() {
                    selections[kept_index].kept = true;
                }
                Ok(RuntimeValue::Int(effective))
            }
            Op::DslSource(op) if node.value_type == ValueType::Int => {
                // sk-84i.2 (widened G-1): resolve the text-valued slot-id
                // node — same Option C shape `RollTable.table_id_node` uses
                // (literal or `<subject>.text('id')` runtime read).
                let slot = match self.eval_node(op.slot_node, depth + 1)? {
                    RuntimeValue::Text(s) => s,
                    other => return Err(self.type_mismatch(op.slot_node, ValueType::Text, other)),
                };
                self.eval_dsl_source(node.node_id, &op.subject, &slot, depth)
                    .map(RuntimeValue::Int)
            }
            Op::Let(op) => {
                // #1891: in offer mode, a value whose subtree draws (directly,
                // or transitively via an already-deferred stochastic binding)
                // is NEVER evaluated — deferring avoids the draw entirely,
                // rather than relying on a downstream Compare/RollUnder to
                // short-circuit it (the bug this canon-warden finding fixed:
                // `let roll = d20; roll >= 15` reaches `Op::Let` FIRST, before
                // any Compare gets a chance to guard the draw). The binding
                // name is recorded so every later reference — by name, not by
                // node-id edge — is recognised as stochastic too.
                if self.offer_satisfiable_mode
                    && subtree_has_stochastic_draw(self.expr, op.value, &self.stochastic_bindings)
                {
                    self.stochastic_bindings.insert(op.name.clone());
                    let body = self.eval_node(op.body, depth + 1);
                    self.stochastic_bindings.remove(&op.name);
                    return body;
                }
                let value = self.eval_node(op.value, depth + 1)?;
                self.lexical_bindings.push((op.name.clone(), value));
                let body = self.eval_node(op.body, depth + 1);
                self.lexical_bindings.pop();
                body
            }
            Op::RollTable(_)
            | Op::DslSource(_)
            | Op::OutcomeSequence(_)
            | Op::InvokeOutcomeGraph(_)
            | Op::ModifyResource(_)
            | Op::ApplyStatus(_)
            | Op::EmitTrace(_)
            | Op::Arbitrate(_) => Err(EvaluationError::UnsupportedInPredicate {
                node_id: node.node_id,
            }),
        }
    }

    fn roll(&mut self, node_id: NodeId, dice: &Dice) -> Result<i64, EvaluationError> {
        let attempted = self.draws_used.saturating_add(dice.count);
        if attempted > self.limits.max_draws {
            return Err(EvaluationError::DrawLimitExceeded {
                attempted,
                max_draws: self.limits.max_draws,
            });
        }

        let mut total = 0i64;
        for _ in 0..dice.count {
            let draw_index = self.draws_used;
            self.draws_used += 1;
            let sides = dice.sides.max(1);
            let value = self.rng.roll_die(sides) as i64;
            if self.rng.exhausted() {
                return Err(EvaluationError::DrawStreamExhausted {
                    node_id,
                    attempted: self.draws_used,
                });
            }
            self.stochastic_draws.push(StochasticDraw {
                source_id: format!("dsl_node:{node_id}"),
                draw_index,
                value,
            });
            total += value;
        }
        Ok(total)
    }

    fn roll_table_int(&mut self, node_id: NodeId, table_id: &str) -> Result<i64, EvaluationError> {
        let mut candidates: Vec<(&RollTableEntryFact, i64)> = self
            .facts
            .roll_table_entries
            .iter()
            .filter(|entry| entry.table_id == table_id && entry.weight > 0)
            .map(|entry| {
                let value = entry.result_ref.parse::<i64>().map_err(|_| {
                    EvaluationError::NonUniformRollTableIntEntry {
                        node_id,
                        table_id: table_id.to_string(),
                        entry_id: entry.entry_id.clone(),
                        result_ref: entry.result_ref.clone(),
                    }
                })?;
                Ok((entry, value))
            })
            .collect::<Result<_, _>>()?;
        candidates.sort_by(|(left, _), (right, _)| left.entry_id.cmp(&right.entry_id));
        if candidates.is_empty() {
            return Err(EvaluationError::EmptyRollTable {
                node_id,
                table_id: table_id.to_string(),
            });
        }

        let attempted = self.draws_used.saturating_add(1);
        if attempted > self.limits.max_draws {
            return Err(EvaluationError::DrawLimitExceeded {
                attempted,
                max_draws: self.limits.max_draws,
            });
        }

        let draw_index = self.draws_used;
        self.draws_used += 1;
        let total_weight: u64 = candidates
            .iter()
            .map(|(entry, _)| entry.weight as u64)
            .sum();
        let draw = self.rng.next_u32();
        if self.rng.exhausted() {
            return Err(EvaluationError::DrawStreamExhausted {
                node_id,
                attempted: self.draws_used,
            });
        }
        let mut ticket = draw as u64 % total_weight;
        for (entry, value) in candidates {
            if ticket < entry.weight as u64 {
                self.stochastic_draws.push(StochasticDraw {
                    source_id: format!("roll_table:{table_id}"),
                    draw_index,
                    value,
                });
                if self.trace.is_some() {
                    self.pending_roll_table_selections
                        .get_or_insert_with(Vec::new)
                        .push(RollTableSelection {
                            entry_id: entry.entry_id.clone(),
                            kept: false,
                        });
                }
                return Ok(value);
            }
            ticket -= entry.weight as u64;
        }

        unreachable!("positive total_weight always selects a candidate")
    }

    /// sk-84i.2 (widened G-1): evaluate `subject`'s named DSL-source slot
    /// inline, in `self`'s OWN fact context — `actor`/`target` are never
    /// re-derived (the kernel-guaranteed subject binding: the item
    /// expression sees the SAME `EvaluationContext` the caller does, so
    /// `actor` stays the wielder and `target` stays the surrounding act's
    /// target).
    ///
    /// Fail-closed (ADR-0035), split per canon-warden D-1 into two distinct
    /// shapes so the JSONL tells an operator the truth: if `item_dsl_sources`
    /// is entirely empty, this call site plausibly never wires the fact pool
    /// at all — `NoItemDslSourceFacts` (a kernel wiring gap, not a content
    /// fact about `subject`). If the pool IS populated but carries no
    /// `(subject, slot)` match, that is a genuine missing fact —
    /// `MissingItemDslSource`. Neither is ever a silent zero. The nested
    /// evaluation uses the commit-cached `expr` on the matching fact (#2218);
    /// there is no act-time re-parse of authored source.
    ///
    /// Recursion guard: `dsl_source_depth` is depth-limited to 1 — the
    /// item's OWN source may not itself call `.source(...)` again.
    ///
    /// Draw budget (ADR-0059): the nested evaluation shares `self.rng` and is
    /// capped at `self.limits.max_draws - self.draws_used` — the REMAINING
    /// budget, not a fresh one — so the caller's expression and the item's
    /// expression draw against ONE combined ceiling. Every draw the nested
    /// evaluation performs (and its `StochasticDraw` provenance) is folded
    /// back into `self` on return, so a bomb expression (e.g. an authored
    /// slot containing `300d6`) trips `DrawLimitExceeded` instead of running
    /// unbounded, and a legitimate roll's provenance is visible to the
    /// outer caller exactly as if it had been drawn directly. See
    /// `count_static_draws`'s `Op::DslSource` arm for how the STATIC
    /// pre-count sizes the ceiling this shares.
    fn eval_dsl_source(
        &mut self,
        node_id: NodeId,
        subject: &SubjectRef,
        slot: &str,
        depth: u32,
    ) -> Result<i64, EvaluationError> {
        let fact = match self
            .facts
            .item_dsl_sources
            .iter()
            .find(|fact| &fact.subject == subject && fact.slot == slot)
        {
            Some(fact) => fact,
            // Nothing was populated for THIS call site at all — plausibly a
            // kernel wiring gap, not a content fact about `subject` (D-1).
            None if self.facts.item_dsl_sources.is_empty() => {
                return Err(EvaluationError::NoItemDslSourceFacts {
                    node_id,
                    subject: subject.clone(),
                    slot: slot.to_string(),
                });
            }
            // The pool IS populated (this call site wires it) but nothing
            // matches — a genuine missing fact.
            None => {
                return Err(EvaluationError::MissingItemDslSource {
                    node_id,
                    subject: subject.clone(),
                    slot: slot.to_string(),
                });
            }
        };

        if self.dsl_source_depth >= 1 {
            return Err(EvaluationError::DslSourceRecursionLimit {
                node_id,
                subject: subject.clone(),
                slot: slot.to_string(),
            });
        }

        let parsed = &fact.expr;

        let nested_scope = self.nested_trace_scope(subject, slot);

        let mut nested = EvaluationState {
            expr: parsed,
            facts: self.facts,
            rng: &mut *self.rng,
            limits: EvaluationLimits {
                max_depth: self.limits.max_depth,
                max_nodes: self.limits.max_nodes,
                max_draws: self.limits.max_draws.saturating_sub(self.draws_used),
            },
            draws_used: 0,
            stochastic_draws: Vec::new(),
            active_nodes: BTreeSet::new(),
            lexical_bindings: Vec::new(),
            suppress_roll_bias: false,
            offer_satisfiable_mode: self.offer_satisfiable_mode,
            stochastic_bindings: BTreeSet::new(),
            dsl_source_depth: self.dsl_source_depth + 1,
            trace: self.trace.as_ref().map(|_| Vec::new()),
            current_scope: nested_scope,
            pending_roll_table_selections: None,
        };
        let eval_res = nested.eval_int(parsed.root_node_id, depth + 1);
        if let Some(nested_trace) = nested.trace {
            if let Some(caller_trace) = self.trace.as_mut() {
                caller_trace.extend(nested_trace);
            }
        }
        let value = eval_res?;
        self.draws_used = self.draws_used.saturating_add(nested.draws_used);
        self.stochastic_draws.extend(nested.stochastic_draws);
        Ok(value)
    }

    fn type_mismatch(
        &self,
        node_id: NodeId,
        expected: ValueType,
        actual: RuntimeValue,
    ) -> EvaluationError {
        let actual = match actual {
            RuntimeValue::Bool(_) => ValueType::Bool,
            RuntimeValue::Int(_) => ValueType::Int,
            RuntimeValue::Text(_) => ValueType::Text,
        };
        EvaluationError::TypeMismatch {
            node_id,
            expected,
            actual,
        }
    }

    fn lexical_binding_value(&self, subject: &SubjectRef) -> Option<RuntimeValue> {
        let SubjectRef::Binding(binding) = subject else {
            return None;
        };
        self.lexical_bindings
            .iter()
            .rev()
            .find(|(name, _)| name == &binding.name)
            .map(|(_, value)| value.clone())
    }

    fn event_text_value(&self, subject: &SubjectRef, field_id: &str) -> Option<String> {
        match subject {
            SubjectRef::Event(EventRef::Kind) if field_id == "kind" => {
                self.facts.event_kind.clone()
            }
            _ => None,
        }
    }

    fn resolve_binding_value(&self, value: &str) -> Result<String, EvaluationError> {
        if let Some(name) = value.strip_prefix("$bindings.") {
            return self
                .facts
                .bindings
                .iter()
                .find(|binding| binding.name == name)
                .map(|binding| binding.value.clone())
                .ok_or_else(|| EvaluationError::UnknownBinding {
                    binding: name.to_string(),
                });
        }
        Ok(value.to_string())
    }
}

fn has_id(facts: &[SubjectIdFact], subject: &SubjectRef, id: &str) -> bool {
    facts
        .iter()
        .any(|fact| &fact.subject == subject && fact.id == id)
}

/// `shares_tag` eval: true iff the prefix-filtered tag set of `subject_a` and
/// that of `subject_b` intersect. Reads only the two named subjects' own tag
/// pools — no world scan. Content-neutral: `prefix` is an authored string the
/// substrate never interprets.
fn shares_tag(
    facts: &[SubjectIdFact],
    subject_a: &SubjectRef,
    subject_b: &SubjectRef,
    prefix: &str,
) -> bool {
    let tags_of = |subject: &SubjectRef| -> Vec<&str> {
        facts
            .iter()
            .filter(|fact| &fact.subject == subject && fact.id.starts_with(prefix))
            .map(|fact| fact.id.as_str())
            .collect()
    };
    let a_tags = tags_of(subject_a);
    a_tags
        .iter()
        .any(|tag| tags_of(subject_b).iter().any(|other| other == tag))
}

fn compare_runtime_values(
    node_id: NodeId,
    op: CompareOp,
    left: RuntimeValue,
    right: RuntimeValue,
) -> Result<bool, EvaluationError> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => Ok(match op {
            CompareOp::Eq => left == right,
            CompareOp::Ne => left != right,
            CompareOp::Lt => left < right,
            CompareOp::Lte => left <= right,
            CompareOp::Gt => left > right,
            CompareOp::Gte => left >= right,
        }),
        (RuntimeValue::Text(left), RuntimeValue::Text(right)) => match op {
            CompareOp::Eq => Ok(left == right),
            CompareOp::Ne => Ok(left != right),
            _ => Err(EvaluationError::TypeMismatch {
                node_id,
                expected: ValueType::Int,
                actual: ValueType::Text,
            }),
        },
        (RuntimeValue::Bool(left), RuntimeValue::Bool(right)) => match op {
            CompareOp::Eq => Ok(left == right),
            CompareOp::Ne => Ok(left != right),
            _ => Err(EvaluationError::TypeMismatch {
                node_id,
                expected: ValueType::Int,
                actual: ValueType::Bool,
            }),
        },
        (_, actual) => Err(EvaluationError::TypeMismatch {
            node_id,
            expected: ValueType::Int,
            actual: runtime_value_type(&actual),
        }),
    }
}

fn runtime_value_type(value: &RuntimeValue) -> ValueType {
    match value {
        RuntimeValue::Bool(_) => ValueType::Bool,
        RuntimeValue::Int(_) => ValueType::Int,
        RuntimeValue::Text(_) => ValueType::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        parse_int_expression, parse_predicate, Arithmetic, BoolLiteral, ChannelPool, Compare,
        CompareOp, DeterministicRng, DslSource, HasCapability, ImpliedByCount, IntLiteral,
        OccupantsAt, Operands, ParticipantTagGroupCount, PeersNearTarget, ReadField, Roll,
        RollTable, RollUnder, SharesTag, TextLiteral, UnaryNode,
    };

    fn node(node_id: NodeId, value_type: ValueType, op: Op) -> Node {
        Node {
            node_id,
            value_type,
            op,
            span: crate::SourceSpan::default(),
        }
    }

    fn parse(source: &str) -> Expression {
        parse_predicate(source).expect("predicate source should parse")
    }

    fn d20_entries() -> Vec<RollTableEntryFact> {
        vec![
            RollTableEntryFact {
                table_id: "core/d20".into(),
                entry_id: "core/d20:1".into(),
                weight: 1,
                result_ref: "1".into(),
            },
            RollTableEntryFact {
                table_id: "core/d20".into(),
                entry_id: "core/d20:2".into(),
                weight: 1,
                result_ref: "2".into(),
            },
        ]
    }

    fn roll_under_expr(sides: u32, target: i64) -> Expression {
        Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 3,
            nodes: vec![
                node(
                    1,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice { count: 1, sides },
                    }),
                ),
                node(
                    2,
                    ValueType::Int,
                    Op::IntLiteral(IntLiteral { value: target }),
                ),
                node(
                    3,
                    ValueType::Bool,
                    Op::RollUnder(RollUnder {
                        roll_node: 1,
                        target_node: 2,
                    }),
                ),
            ],
        }
    }

    fn additive_roll_expr() -> Expression {
        Expression {
            kind: ExpressionKind::Outcome,
            root_node_id: 5,
            nodes: vec![
                node(
                    1,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice {
                            count: 1,
                            sides: 20,
                        },
                    }),
                ),
                node(
                    2,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice {
                            count: 20,
                            sides: 6,
                        },
                    }),
                ),
                node(
                    3,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice {
                            count: 20,
                            sides: 6,
                        },
                    }),
                ),
                node(
                    4,
                    ValueType::Int,
                    Op::Arithmetic(Arithmetic {
                        op: ArithmeticOp::Add,
                        left: 1,
                        right: 2,
                    }),
                ),
                node(
                    5,
                    ValueType::Int,
                    Op::Arithmetic(Arithmetic {
                        op: ArithmeticOp::Add,
                        left: 4,
                        right: 3,
                    }),
                ),
            ],
        }
    }

    fn roll_under_dice_expr(count: u32, sides: u32, target: i64) -> Expression {
        Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 3,
            nodes: vec![
                node(
                    1,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice { count, sides },
                    }),
                ),
                node(
                    2,
                    ValueType::Int,
                    Op::IntLiteral(IntLiteral { value: target }),
                ),
                node(
                    3,
                    ValueType::Bool,
                    Op::RollUnder(RollUnder {
                        roll_node: 1,
                        target_node: 2,
                    }),
                ),
            ],
        }
    }

    #[test]
    fn count_static_draws_sums_dice_and_roll_tables() {
        assert_eq!(count_static_draws(&additive_roll_expr()), 41);

        let roll_table_expr = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 3,
            nodes: vec![
                node(1, ValueType::Int, Op::IntLiteral(IntLiteral { value: 0 })),
                node(
                    2,
                    ValueType::Int,
                    Op::RollTable(RollTable {
                        table_id_node: 4,
                        draw_node: 1,
                    }),
                ),
                node(
                    3,
                    ValueType::Bool,
                    Op::RollUnder(RollUnder {
                        roll_node: 2,
                        target_node: 1,
                    }),
                ),
                node(
                    4,
                    ValueType::Text,
                    Op::TextLiteral(TextLiteral {
                        value: "core/d20".into(),
                    }),
                ),
            ],
        };

        assert_eq!(count_static_draws(&roll_table_expr), 1);
    }

    #[test]
    fn evaluation_limits_size_draw_budget_from_expression() {
        let expr = additive_roll_expr();
        let limits = EvaluationLimits::for_expression(&expr);

        assert_eq!(EvaluationLimits::default().max_draws, MAX_DRAW_CEILING);
        assert_eq!(limits.max_depth, EvaluationLimits::default().max_depth);
        assert_eq!(limits.max_nodes, EvaluationLimits::default().max_nodes);
        assert_eq!(limits.max_draws, 82);

        let oversized = roll_under_dice_expr(200, 6, 700);
        assert_eq!(
            EvaluationLimits::for_expression(&oversized).max_draws,
            MAX_DRAW_CEILING
        );
    }

    #[test]
    fn expression_draw_budget_covers_biased_high_draw_roll_under() {
        let expr = roll_under_dice_expr(20, 6, 120);
        let facts = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Advantage),
            }),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![0; 40]);
        let result = evaluate_predicate_expression_with_report(
            &expr,
            &facts,
            &mut rng,
            EvaluationLimits::for_expression(&expr),
        )
        .expect("20d6 with advantage should fit its expression-sized draw budget");

        assert!(result.value);
        assert_eq!(count_static_draws(&expr), 20);
        assert_eq!(EvaluationLimits::for_expression(&expr).max_draws, 40);
        assert_eq!(result.draws_used, 40);
    }

    #[test]
    fn expression_draw_budget_keeps_ceiling_fail_closed() {
        let expr = roll_under_dice_expr(200, 6, 1200);
        let facts = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Advantage),
            }),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![0; MAX_DRAW_CEILING as usize]);

        assert_eq!(
            evaluate_predicate_expression_with_report(
                &expr,
                &facts,
                &mut rng,
                EvaluationLimits::for_expression(&expr),
            ),
            Err(EvaluationError::DrawLimitExceeded {
                attempted: 400,
                max_draws: MAX_DRAW_CEILING
            })
        );
    }

    // COND-1 (ADR-0038 D-3 / CR-7 slice C): the `has_tag` read leg is a genuine
    // §17 K-5 `DslOp` growth, landed RED-first. This fixture is committed FAILING
    // before the `Op::HasTag` arm exists — `has_tag` is not yet a recognized
    // bool-method primitive, so `parse_method_bool` rejects it as
    // `MF-DSL-UNKNOWN-PRIMITIVE`. The slice-C arm patch flips both asserts to
    // pass; the eval-over-both-table-sources proof lands alongside in
    // `k_op_has_tag_evaluates_instance_and_definition_origin_facts` (added green).
    // Both SubjectRef kinds the eval arm handles are exercised at the parse leg:
    // `Target` (Redbrand-marked townsfolk gating) and `Actor` (group membership).
    #[test]
    fn k_op_has_tag_parses_for_target_and_actor_subjects() {
        assert!(
            parse_predicate("target.has_tag(\"redbrand_marked\")").is_ok(),
            "target.has_tag(...) must parse once Op::HasTag lands (slice C); \
             RED before the arm — has_tag is not yet a bool-method primitive"
        );
        assert!(
            parse_predicate("actor.has_tag(\"srd.party.redbrands\")").is_ok(),
            "actor.has_tag(...) must parse once Op::HasTag lands (slice C)"
        );
    }

    // COND-1 green half: `has_tag` evaluates correctly over the `tags` pool for
    // BOTH the instance and the definition table source, and for both SubjectRef
    // kinds. The pool is content-neutral — population (not eval) decides which
    // table a fact came from — so here we seed one fact documented as
    // instance-origin (`entity_tag`) and one as definition-origin
    // (`entity_definition_tag`); the eval arm resolving BOTH proves the union the
    // caller materialises (the live X11 beat is the end-to-end wrong-table guard).
    #[test]
    fn k_op_has_tag_evaluates_instance_and_definition_origin_facts() {
        let mut rng = DeterministicRng::new(vec![]);
        let facts = EvaluationContext {
            tags: vec![
                // instance-origin: the marked man's own `entity_tag` row.
                SubjectIdFact {
                    subject: SubjectRef::Target,
                    id: "redbrand_marked".into(),
                },
                // definition-origin: a tag on the man's `entity_definition_tag`.
                SubjectIdFact {
                    subject: SubjectRef::Target,
                    id: "townsfolk".into(),
                },
                // actor-subject tag: group membership.
                SubjectIdFact {
                    subject: SubjectRef::Actor,
                    id: "srd.party.redbrands".into(),
                },
            ],
            ..EvaluationContext::default()
        };
        let mut eval = |source: &str| {
            let expr = parse_predicate(source).expect("has_tag predicate should parse (slice C)");
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default())
        };
        assert_eq!(
            eval("target.has_tag(\"redbrand_marked\")"),
            Ok(true),
            "instance leg"
        );
        assert_eq!(
            eval("target.has_tag(\"townsfolk\")"),
            Ok(true),
            "definition leg"
        );
        assert_eq!(
            eval("actor.has_tag(\"srd.party.redbrands\")"),
            Ok(true),
            "actor subject"
        );
        // Absent tag, and right tag on the WRONG subject, both resolve false.
        assert_eq!(
            eval("target.has_tag(\"glasstaff\")"),
            Ok(false),
            "absent tag"
        );
        assert_eq!(
            eval("actor.has_tag(\"redbrand_marked\")"),
            Ok(false),
            "instance tag belongs to the Target, not the Actor"
        );
    }

    // ── shares_tag (ADR-0021 D-2 open-vocab primitive, #1242) ───────────────

    /// K-5 FALSIFIER (red-first). The merge gate for the new `DslOp` variant: a
    /// modder adds a NEW `actor.language.elvish` tag to two creatures and a niche
    /// authored with the FIXED prefix predicate `shares_tag(actor, target,
    /// "actor.language.")`; communication gates correctly between them with NO
    /// edit to that predicate. This passes with `shares_tag` and is impossible
    /// with a static disjunction (which would need the predicate re-edited to add
    /// an `elvish` arm). Committed FAILING before the op exists — `shares_tag`
    /// does not parse, so the predicate is rejected at parse time.
    #[test]
    fn k5_shares_tag_open_vocab_gates_a_newly_authored_language_without_predicate_edit() {
        // The core predicate — authored ONCE, never re-edited per language.
        const COMMUNICATION_PREDICATE: &str = "shares_tag(actor, target, \"actor.language.\")";
        let mut rng = DeterministicRng::new(vec![]);

        // A modder adds a brand-new language family member — `actor.language.elvish`
        // — to BOTH creatures. No core-predicate edit accompanies it.
        let shared = EvaluationContext {
            tags: vec![
                SubjectIdFact {
                    subject: SubjectRef::Actor,
                    id: "actor.language.elvish".into(),
                },
                SubjectIdFact {
                    subject: SubjectRef::Target,
                    id: "actor.language.elvish".into(),
                },
            ],
            ..EvaluationContext::default()
        };
        let expr = parse_predicate(COMMUNICATION_PREDICATE)
            .expect("shares_tag predicate must parse once Op::SharesTag lands");
        assert_eq!(
            evaluate_predicate_expression(&expr, &shared, &mut rng, EvaluationLimits::default()),
            Ok(true),
            "a shared newly-authored language → communication eligible, with zero predicate edit",
        );

        // A disjoint pair (each speaks a DIFFERENT language) → NOT eligible.
        let disjoint = EvaluationContext {
            tags: vec![
                SubjectIdFact {
                    subject: SubjectRef::Actor,
                    id: "actor.language.elvish".into(),
                },
                SubjectIdFact {
                    subject: SubjectRef::Target,
                    id: "actor.language.dwarvish".into(),
                },
            ],
            ..EvaluationContext::default()
        };
        assert_eq!(
            evaluate_predicate_expression(&expr, &disjoint, &mut rng, EvaluationLimits::default()),
            Ok(false),
            "no shared language → communication not eligible (the universal-target escape hatch is closed)",
        );
    }

    /// Eval truth table over the prefix-filtered set intersection.
    #[test]
    fn shares_tag_eval_truth_table() {
        let eval = |tags: Vec<SubjectIdFact>| {
            let facts = EvaluationContext {
                tags,
                ..EvaluationContext::default()
            };
            let expr = parse_predicate("shares_tag(actor, target, \"actor.language.\")")
                .expect("shares_tag should parse");
            evaluate_predicate_expression(
                &expr,
                &facts,
                &mut DeterministicRng::new(vec![]),
                EvaluationLimits::default(),
            )
        };
        let lang = |subject, id: &str| SubjectIdFact {
            subject,
            id: id.into(),
        };

        // shared-in-family → true
        assert_eq!(
            eval(vec![
                lang(SubjectRef::Actor, "actor.language.common"),
                lang(SubjectRef::Target, "actor.language.common"),
            ]),
            Ok(true),
            "shared family member"
        );
        // disjoint → false
        assert_eq!(
            eval(vec![
                lang(SubjectRef::Actor, "actor.language.common"),
                lang(SubjectRef::Target, "actor.language.goblin"),
            ]),
            Ok(false),
            "disjoint languages"
        );
        // neither-has-family → false
        assert_eq!(
            eval(vec![
                lang(SubjectRef::Actor, "actor.faction.lords_alliance"),
                lang(SubjectRef::Target, "actor.faction.lords_alliance"),
            ]),
            Ok(false),
            "shared tag but outside the queried family"
        );
        // one-has → false
        assert_eq!(
            eval(vec![lang(SubjectRef::Actor, "actor.language.common")]),
            Ok(false),
            "only one subject carries a language"
        );
        // prefix-matches-nothing → false (empty pool)
        assert_eq!(eval(vec![]), Ok(false), "no tags at all");
    }

    // COND-1 arm-count pin: `Op` is a §17.2 closed kernel surface. This
    // exhaustive match breaks (and this count must move) the moment a variant
    // is added or removed.
    //
    // sk-ct8 / sk-84i.2 merge reconciliation: the 34th arm is sk-84i.2's
    // `DslSource`; the 35th arm is sk-ct8.4 / finding #86's `ChannelPool`;
    // the 36th and 37th arms are sk-ct8.2 (#30)'s per-entity resource bound
    // reads `ReadResourceMax`/`ReadResourceMin`; the 38th is ADR-0105's
    // `PeersNearTarget`; the 39th is ADR-0134 D-3's `OccupantsAt`.
    #[test]
    fn k_op_arm_set_pinned_at_39_with_peers_near_target_and_occupants_at() {
        fn discriminant(op: &Op) -> u32 {
            match op {
                Op::BoolLiteral(_) => 1,
                Op::IntLiteral(_) => 2,
                Op::TextLiteral(_) => 3,
                Op::ReadInt(_) => 4,
                Op::ReadEffectiveInt(_) => 31,
                Op::ReadText(_) => 5,
                Op::ReadBool(_) => 6,
                Op::HasCapability(_) => 7,
                Op::HasItem(_) => 8,
                Op::HasNiche(_) => 9,
                Op::HasTag(_) => 10,
                Op::HasStatus(_) => 11,
                Op::ImpliedByCount(_) => 12,
                Op::TracePresent(_) => 13,
                Op::KnowsEntity(_) => 14,
                Op::And(_) => 15,
                Op::Or(_) => 16,
                Op::Not(_) => 17,
                Op::Compare(_) => 18,
                Op::Arithmetic(_) => 19,
                Op::Roll(_) => 20,
                Op::RollUnder(_) => 21,
                Op::RollTable(_) => 22,
                Op::Let(_) => 23,
                Op::OutcomeSequence(_) => 24,
                Op::InvokeOutcomeGraph(_) => 25,
                Op::ModifyResource(_) => 26,
                Op::ApplyStatus(_) => 27,
                Op::EmitTrace(_) => 28,
                Op::Arbitrate(_) => 29,
                Op::Cond(_) => 30,
                Op::SharesTag(_) => 32,
                Op::ParticipantTagGroupCount(_) => 33,
                Op::DslSource(_) => 34,
                Op::ChannelPool(_) => 35,
                Op::ReadResourceMax(_) => 36,
                Op::ReadResourceMin(_) => 37,
                Op::PeersNearTarget(_) => 38,
                Op::OccupantsAt(_) => 39,
            }
        }
        assert_eq!(
            discriminant(&Op::ImpliedByCount(ImpliedByCount {
                condition_definition_id: Some("core/condition/incapacitated".into()),
            })),
            12,
            "ImpliedByCount keeps discriminant 12"
        );
        // #1247 added the 31st arm: the effective (modifier-composed) int read,
        // kept distinct from ReadInt (4) so the count-must-move pin stays visible.
        assert_eq!(
            discriminant(&Op::ReadEffectiveInt(ReadField {
                subject: SubjectRef::Actor,
                field_id: "homebrew/Momentum".into(),
            })),
            31,
            "#1247 added ReadEffectiveInt (actor.effective(...)) at discriminant 31"
        );
        // main added the 32nd arm: SharesTag (shares_tag set-intersection).
        assert_eq!(
            discriminant(&Op::SharesTag(SharesTag {
                subject_a: SubjectRef::Actor,
                subject_b: SubjectRef::Target,
                prefix: "faction/".into(),
            })),
            32,
            "Op arm-set now 32; SharesTag is the 32nd arm"
        );
        assert_eq!(
            discriminant(&Op::ParticipantTagGroupCount(ParticipantTagGroupCount {
                prefix: "faction:".into(),
                excluding_condition_definition_id: Some("core/condition/incapacitated".into()),
            })),
            33,
            "Op arm-set now 33; ParticipantTagGroupCount is the 33rd arm"
        );
        // sk-84i.2 added the 34th arm: DslSource (`<subject>.source('slot')`,
        // widened G-1 from the original hardcoded `wielded_source('slot')`).
        assert_eq!(
            discriminant(&Op::DslSource(DslSource {
                subject: SubjectRef::ActorWieldedItem,
                slot_node: 1,
            })),
            34,
            "Op arm-set now 34; DslSource is the 34th arm"
        );
        // sk-ct8.4 / finding #86 added the 35th arm: ChannelPool.
        assert_eq!(
            discriminant(&Op::ChannelPool(ChannelPool {
                channel_id: "core/action_class/primary".into(),
            })),
            35,
            "Op arm-set now 35; ChannelPool is the 35th arm"
        );
        // sk-ct8.2 (#30) added the 36th and 37th arms: the per-entity resource
        // bound reads.
        assert_eq!(
            discriminant(&Op::ReadResourceMax(ReadField {
                subject: SubjectRef::Actor,
                field_id: "srd/hp".into(),
            })),
            36,
            "sk-ct8.2 added ReadResourceMax (actor.resource_max(...)) at discriminant 36"
        );
        assert_eq!(
            discriminant(&Op::ReadResourceMin(ReadField {
                subject: SubjectRef::Actor,
                field_id: "srd/hp".into(),
            })),
            37,
            "sk-ct8.2 added ReadResourceMin (actor.resource_min(...)) at discriminant 37"
        );
        // K-A Fact 2 added the 38th arm:
        // PeersNearTarget.
        assert_eq!(
            discriminant(&Op::PeersNearTarget(PeersNearTarget {
                prefix: "srd.faction.".into(),
                radius_mm: 1524,
                excluding_condition_definition_id: Some("srd/condition/incapacitated".into()),
            })),
            38,
            "PeersNearTarget keeps discriminant 38"
        );
        // ADR-0134 D-3 added the 39th arm: OccupantsAt.
        assert_eq!(
            discriminant(&Op::OccupantsAt(OccupantsAt {
                scope: "to_node".into(),
                excluding_condition_definition_id: Some("srd/condition/dead".into()),
            })),
            39,
            "Op arm-set now 39; OccupantsAt is the 39th arm"
        );
    }

    #[test]
    fn implied_by_count_empty_context_defaults_to_zero() {
        let expr = parse(r#"implied_by_count("core/condition/incapacitated") == 0"#);
        let mut rng = DeterministicRng::new(vec![]);

        assert_eq!(
            evaluate_predicate_expression(
                &expr,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default()
            ),
            Ok(true)
        );
    }

    #[test]
    fn implied_by_count_reads_explicit_child_count() {
        let expr = parse(r#"implied_by_count("core/condition/incapacitated") == 2"#);
        let facts = EvaluationContext {
            implied_by_counts: vec![ImpliedByCountFact {
                condition_definition_id: "core/condition/incapacitated".into(),
                count: 2,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);

        assert_eq!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );
    }

    #[test]
    fn participant_tag_group_count_reads_explicit_fact_and_missing_fact_errors() {
        let expr = parse(
            r#"participant_tag_group_count("faction:", "core/condition/incapacitated") == 1"#,
        );
        let mut rng = DeterministicRng::new(vec![]);

        let missing = evaluate_predicate_expression(
            &expr,
            &EvaluationContext::default(),
            &mut rng,
            EvaluationLimits::default(),
        );
        assert!(matches!(missing, Err(EvaluationError::UnknownField { .. })));

        let facts = EvaluationContext {
            participant_tag_group_counts: vec![ParticipantTagGroupCountFact {
                prefix: "faction:".into(),
                excluding_condition_definition_id: Some("core/condition/incapacitated".into()),
                count: 1,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );

        // Control: two groups still standing must NOT satisfy the terminal
        // predicate — proves the count is honoured, not raw group presence.
        let control = parse(
            r#"participant_tag_group_count("faction:", "core/condition/incapacitated") <= 1"#,
        );
        let facts = EvaluationContext {
            participant_tag_group_counts: vec![ParticipantTagGroupCountFact {
                prefix: "faction:".into(),
                excluding_condition_definition_id: Some("core/condition/incapacitated".into()),
                count: 2,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(&control, &facts, &mut rng, EvaluationLimits::default()),
            Ok(false)
        );
    }

    /// sk-ct8.4 / finding #86: `channel_pool` reads a pre-populated fact and
    /// fails closed (never silently 0) when the fact is missing.
    #[test]
    fn channel_pool_reads_explicit_fact_and_missing_fact_errors() {
        let expr = parse_int_expression(r#"channel_pool('core/action_class/primary')"#)
            .expect("channel_pool int expression should parse");
        let mut rng = DeterministicRng::new(vec![]);

        let missing = evaluate_int_expression(
            &expr,
            &EvaluationContext::default(),
            &mut rng,
            EvaluationLimits::default(),
        );
        assert!(
            matches!(missing, Err(EvaluationError::UnknownField { .. })),
            "missing channel_pool fact must fail closed, got {missing:?}"
        );

        let facts = EvaluationContext {
            channel_pools: vec![ChannelPoolFact {
                channel_id: "core/action_class/primary".into(),
                pool_ms_per_round: 1000,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_int_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(1000)
        );
    }

    /// `channel_pool` is usable inside a comparison predicate too (not just a
    /// bare outcome expression) — same "leaf Int op composes anywhere an Int
    /// is expected" contract `implied_by_count`/`participant_tag_group_count`
    /// already carry.
    #[test]
    fn channel_pool_composes_inside_a_predicate_comparison() {
        let expr = parse_predicate(r#"channel_pool('core/action_class/primary') >= 1000"#)
            .expect("channel_pool predicate should parse");
        let facts = EvaluationContext {
            channel_pools: vec![ChannelPoolFact {
                channel_id: "core/action_class/primary".into(),
                pool_ms_per_round: 1000,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );
    }

    #[test]
    fn participant_tag_group_count_guard_blocks_untagged_sessions() {
        let expr = parse(
            r#"participant_tag_group_count("faction:") >= 2 && participant_tag_group_count("faction:", "core/condition/incapacitated") <= 1"#,
        );

        let untagged = EvaluationContext {
            participant_tag_group_counts: vec![
                ParticipantTagGroupCountFact {
                    prefix: "faction:".into(),
                    excluding_condition_definition_id: None,
                    count: 0,
                },
                ParticipantTagGroupCountFact {
                    prefix: "faction:".into(),
                    excluding_condition_definition_id: Some("core/condition/incapacitated".into()),
                    count: 0,
                },
            ],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(&expr, &untagged, &mut rng, EvaluationLimits::default()),
            Ok(false)
        );

        let terminal = EvaluationContext {
            participant_tag_group_counts: vec![
                ParticipantTagGroupCountFact {
                    prefix: "faction:".into(),
                    excluding_condition_definition_id: None,
                    count: 2,
                },
                ParticipantTagGroupCountFact {
                    prefix: "faction:".into(),
                    excluding_condition_definition_id: Some("core/condition/incapacitated".into()),
                    count: 1,
                },
            ],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(&expr, &terminal, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );

        let two_standing = EvaluationContext {
            participant_tag_group_counts: vec![
                ParticipantTagGroupCountFact {
                    prefix: "faction:".into(),
                    excluding_condition_definition_id: None,
                    count: 2,
                },
                ParticipantTagGroupCountFact {
                    prefix: "faction:".into(),
                    excluding_condition_definition_id: Some("core/condition/incapacitated".into()),
                    count: 2,
                },
            ],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(
                &expr,
                &two_standing,
                &mut rng,
                EvaluationLimits::default()
            ),
            Ok(false)
        );
    }

    #[test]
    fn implied_by_count_without_operand_reads_candidate_binding() {
        let expr = parse("implied_by_count() == 1");
        let facts = EvaluationContext {
            implied_by_counts: vec![ImpliedByCountFact {
                condition_definition_id: "core/condition/incapacitated".into(),
                count: 1,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);

        assert_eq!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );
    }

    #[test]
    fn evaluates_canonical_ast_with_facts_and_reported_draws() {
        let expr = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 3,
            nodes: vec![
                node(
                    1,
                    ValueType::Bool,
                    Op::HasCapability(HasCapability {
                        subject: SubjectRef::Actor,
                        capability_id: "core/walk".into(),
                    }),
                ),
                node(
                    2,
                    ValueType::Bool,
                    Op::ReadBool(ReadField {
                        subject: SubjectRef::Target,
                        field_id: "open".into(),
                    }),
                ),
                node(
                    3,
                    ValueType::Bool,
                    Op::And(Operands {
                        operands: vec![1, 2],
                    }),
                ),
            ],
        };
        let facts = EvaluationContext {
            bool_fields: vec![SubjectBoolFact {
                subject: SubjectRef::Target,
                field_id: "open".into(),
                value: true,
            }],
            capabilities: vec![SubjectIdFact {
                subject: SubjectRef::Actor,
                id: "core/walk".into(),
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);

        assert_eq!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );
    }

    #[test]
    // K-5 (ISS1626 / #1626): a natural 20 must replay as a natural 20, not a
    // spurious critical miss. `roll_under_expr(20, 1)` ("roll 1d20 <= 1") is
    // false on a 20 and true on a 1, so the old-vs-new replay paths disagree
    // observably instead of coincidentally matching.
    fn recorded_natural_20_replays_as_natural_20_not_a_1() {
        let expr = roll_under_expr(20, 1);
        let facts = EvaluationContext::default();
        let limits = EvaluationLimits::default();

        // Fresh evaluation: raw entropy 19 maps to face 20 (`(19 % 20) + 1`).
        let mut fresh_rng = DeterministicRng::new(vec![19]);
        let original =
            evaluate_predicate_expression_with_report(&expr, &facts, &mut fresh_rng, limits)
                .expect("fresh eval should succeed");
        assert!(!original.value, "1d20<=1 is false on a natural 20");
        let recorded_value = original.stochastic_draws[0].value;
        assert_eq!(
            recorded_value, 20,
            "the recorded draw is the resolved face, not raw entropy"
        );

        // Old (buggy) replay: the recorded face fed back in as raw entropy —
        // `(20 % 20) + 1 == 1` — flips a natural 20 into a natural 1.
        let mut old_replay_rng = DeterministicRng::new(vec![recorded_value as u32]);
        let old_replay = evaluate_predicate_expression(&expr, &facts, &mut old_replay_rng, limits)
            .expect("old-style replay eval should succeed");
        assert!(
            old_replay,
            "documents the ISS1626 bug this fixture guards against: a raw-entropy \
             replay of a recorded 20 spuriously flips 1d20<=1 to true"
        );

        // New replay mode: the recorded face is returned verbatim, matching
        // the original evaluation.
        let mut replay_rng = DeterministicRng::from_recorded_values(vec![recorded_value as u32]);
        let replayed = evaluate_predicate_expression(&expr, &facts, &mut replay_rng, limits)
            .expect("value-replay eval should succeed");
        assert_eq!(
            replayed, original.value,
            "replay must match the original evaluation"
        );
    }

    #[test]
    fn roll_modifier_bias_stays_in_shared_evaluator() {
        let expr = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 3,
            nodes: vec![
                node(
                    1,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice {
                            count: 1,
                            sides: 20,
                        },
                    }),
                ),
                node(2, ValueType::Int, Op::IntLiteral(IntLiteral { value: 10 })),
                node(
                    3,
                    ValueType::Bool,
                    Op::RollUnder(RollUnder {
                        roll_node: 1,
                        target_node: 2,
                    }),
                ),
            ],
        };
        let facts = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Advantage),
            }),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![15, 5]);
        let result = evaluate_predicate_expression_with_report(
            &expr,
            &facts,
            &mut rng,
            EvaluationLimits::default(),
        )
        .expect("advantage should use the lower roll for roll-under");

        assert!(result.value);
        assert_eq!(result.draws_used, 2);
        assert_eq!(
            result
                .stochastic_draws
                .iter()
                .map(|draw| draw.value)
                .collect::<Vec<_>>(),
            vec![16, 6]
        );
    }

    #[test]
    fn roll_modifier_bias_flat_bonus_shifts_roll_under_result() {
        let predicate = roll_under_expr(20, 10);
        let facts_with_bias = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: -2,
                roll_modifier: None,
            }),
            ..EvaluationContext::default()
        };

        let mut rng = DeterministicRng::new(vec![10]);
        assert_eq!(
            evaluate_predicate_expression(
                &predicate,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(false)
        );

        let mut rng = DeterministicRng::new(vec![10]);
        assert_eq!(
            evaluate_predicate_expression(
                &predicate,
                &facts_with_bias,
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(true)
        );
    }

    #[test]
    fn roll_modifier_bias_disadvantage_takes_higher_of_two_rolls_on_roll_under() {
        let predicate = roll_under_expr(20, 10);
        let facts_disadvantage = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Disadvantage),
            }),
            ..EvaluationContext::default()
        };

        let mut rng = DeterministicRng::new(vec![5]);
        assert_eq!(
            evaluate_predicate_expression(
                &predicate,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(true)
        );

        let mut rng = DeterministicRng::new(vec![5, 15]);
        assert_eq!(
            evaluate_predicate_expression(
                &predicate,
                &facts_disadvantage,
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(false)
        );
    }

    #[test]
    fn roll_modifier_bias_none_is_identity_no_rng_advance() {
        let predicate = roll_under_expr(20, 15);
        let facts_none_bias = EvaluationContext {
            roll_modifier_bias: None,
            ..EvaluationContext::default()
        };

        let mut rng_a = DeterministicRng::new(vec![10]);
        let result_a = evaluate_predicate_expression_with_report(
            &predicate,
            &facts_none_bias,
            &mut rng_a,
            EvaluationLimits::default(),
        )
        .unwrap();

        let mut rng_b = DeterministicRng::new(vec![10]);
        let result_b = evaluate_predicate_expression_with_report(
            &predicate,
            &EvaluationContext::default(),
            &mut rng_b,
            EvaluationLimits::default(),
        )
        .unwrap();

        assert_eq!(result_a, result_b);
        assert_eq!(result_a.draws_used, 1);
    }

    // ── Battle-test finding #23: `Op::Roll` now consults `roll_modifier_bias`
    // for a single-die (`count == 1`) roll, mirroring `Op::RollTable`'s
    // "higher wins" convention (not `Op::RollUnder`'s inverted one) since a
    // bare `d20` term is always consumed by a `>= DC`-shaped comparison. ────

    #[test]
    fn roll_modifier_bias_advantage_takes_higher_of_two_rolls_on_bare_roll() {
        let expr = parse_int_expression("d20").expect("d20 should parse as an int expression");
        let facts_advantage = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Advantage),
            }),
            ..EvaluationContext::default()
        };

        // Unbiased: exactly one draw, the raw face.
        let mut rng = DeterministicRng::new(vec![5]);
        assert_eq!(
            evaluate_int_expression(
                &expr,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(6)
        );

        // Advantage: two draws (6, 16), effective face is the higher one.
        let mut rng = DeterministicRng::new(vec![5, 15]);
        assert_eq!(
            evaluate_int_expression(
                &expr,
                &facts_advantage,
                &mut rng,
                EvaluationLimits::default()
            ),
            Ok(16)
        );
    }

    #[test]
    fn roll_modifier_bias_disadvantage_takes_lower_of_two_rolls_on_bare_roll() {
        let expr = parse_int_expression("d20").expect("d20 should parse as an int expression");
        let facts_disadvantage = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Disadvantage),
            }),
            ..EvaluationContext::default()
        };

        // Disadvantage: two draws (6, 16), effective face is the lower one.
        let mut rng = DeterministicRng::new(vec![5, 15]);
        assert_eq!(
            evaluate_int_expression(
                &expr,
                &facts_disadvantage,
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(6)
        );
    }

    #[test]
    fn roll_modifier_bias_flat_bonus_shifts_bare_roll_result() {
        let expr = parse_int_expression("d20").expect("d20 should parse as an int expression");
        let facts_with_bonus = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 3,
                roll_modifier: None,
            }),
            ..EvaluationContext::default()
        };

        let mut rng = DeterministicRng::new(vec![5]);
        assert_eq!(
            evaluate_int_expression(
                &expr,
                &facts_with_bonus,
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(9)
        );
    }

    #[test]
    fn roll_modifier_bias_none_is_identity_no_rng_advance_on_bare_roll() {
        let expr = parse_int_expression("d20").expect("d20 should parse as an int expression");

        let mut rng_a = DeterministicRng::new(vec![5]);
        let result_a = evaluate_int_expression(
            &expr,
            &EvaluationContext::default(),
            &mut rng_a,
            EvaluationLimits::default(),
        )
        .unwrap();

        let mut rng_b = DeterministicRng::new(vec![5]);
        let facts_none_bias = EvaluationContext {
            roll_modifier_bias: None,
            ..EvaluationContext::default()
        };
        let result_b = evaluate_int_expression(
            &expr,
            &facts_none_bias,
            &mut rng_b,
            EvaluationLimits::default(),
        )
        .unwrap();

        assert_eq!(result_a, result_b);
        assert_eq!(result_a, 6);
        // Only one draw consumed either way: the second `next_u32()` call
        // would return 0 from an exhausted single-entry `DeterministicRng`
        // and flip `exhausted()` — proving no second draw happened.
        assert!(!rng_a.exhausted());
    }

    #[test]
    fn roll_modifier_bias_multi_die_roll_is_unaffected_by_active_bias() {
        // RAW fidelity (Decision 1): Advantage/Disadvantage apply only to a
        // single-die check roll, never to a multi-die damage/effect roll —
        // an active bias must leave a `2d6`-shaped `Op::Roll` byte-identical.
        let expr = parse_int_expression("2d6").expect("2d6 should parse as an int expression");
        let facts_advantage = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 5,
                roll_modifier: Some(RollModifier::Advantage),
            }),
            ..EvaluationContext::default()
        };

        let mut rng_unbiased = DeterministicRng::new(vec![2, 3]);
        let unbiased = evaluate_int_expression(
            &expr,
            &EvaluationContext::default(),
            &mut rng_unbiased,
            EvaluationLimits::default(),
        )
        .unwrap();

        let mut rng_biased = DeterministicRng::new(vec![2, 3]);
        let biased = evaluate_int_expression(
            &expr,
            &facts_advantage,
            &mut rng_biased,
            EvaluationLimits::default(),
        )
        .unwrap();

        assert_eq!(
            unbiased, biased,
            "an active bias must not touch a multi-die roll"
        );
        assert!(
            !rng_biased.exhausted(),
            "a 2d6 roll under bias must still consume exactly 2 draws, not 4"
        );
    }

    #[test]
    fn count_static_draws_and_limits_stay_sound_for_a_biasable_bare_roll() {
        // A single bare `d20` statically counts as 1 draw; the existing x2
        // headroom (already justified by RollUnder/RollTable) covers the
        // worst case where an active Advantage/Disadvantage bias doubles it.
        let expr = parse_int_expression("d20").expect("d20 should parse as an int expression");
        assert_eq!(count_static_draws(&expr), 1);
        assert_eq!(EvaluationLimits::for_expression(&expr).max_draws, 2);

        let facts_advantage = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Advantage),
            }),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![5, 15]);
        assert_eq!(
            evaluate_int_expression(
                &expr,
                &facts_advantage,
                &mut rng,
                EvaluationLimits::for_expression(&expr),
            ),
            Ok(16),
            "a biased bare roll's 2 draws must fit inside the existing x2 budget"
        );
    }

    /// Suppression applies only to the dice term.
    /// `suppress_roll_bias` wraps ONLY `RollUnder`'s own `roll_node`
    /// re-evaluations — `target_node` is deliberately left unsuppressed.
    /// `RollUnder`'s guard exists solely to stop ITS `roll_node` subtree from
    /// double-applying a bias it already owns end-to-end; `target_node` is a
    /// wholly separate subtree RollUnder claims no bias-application
    /// responsibility for, so a bare, single-die `Op::Roll` used there is
    /// free to consult `roll_modifier_bias` on its own, same as any other
    /// top-level bare roll. Documented here as current, deliberate semantics
    /// (a hypothetical "stochastic DC" feels the bias too) — not something
    /// this bucket changes.
    #[test]
    fn roll_modifier_bias_leaves_roll_under_target_node_unsuppressed() {
        let expr = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 3,
            nodes: vec![
                node(
                    1,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice {
                            count: 1,
                            sides: 20,
                        },
                    }),
                ),
                node(
                    2,
                    ValueType::Int,
                    Op::Roll(Roll {
                        dice: Dice {
                            count: 1,
                            sides: 20,
                        },
                    }),
                ),
                node(
                    3,
                    ValueType::Bool,
                    Op::RollUnder(RollUnder {
                        roll_node: 1,
                        target_node: 2,
                    }),
                ),
            ],
        };
        let facts_advantage = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                flat_bonus: 0,
                roll_modifier: Some(RollModifier::Advantage),
            }),
            ..EvaluationContext::default()
        };

        // Draw order: roll_node's own first draw (suppressed, single draw),
        // then target_node's full unsuppressed Op::Roll evaluation (draws
        // TWICE internally under the active bias), then roll_node's own
        // second (suppressed) draw for RollUnder's own re-roll.
        let mut rng = DeterministicRng::new(vec![5, 15, 2, 18]);
        let result = evaluate_predicate_expression_with_report(
            &expr,
            &facts_advantage,
            &mut rng,
            EvaluationLimits::default(),
        )
        .expect("evaluation should succeed");

        // 2 draws from RollUnder's own suppressed roll_node re-roll + 2 MORE
        // from target_node's own unsuppressed Op::Roll bias consultation —
        // the concrete, assertable consequence of leaving target_node out of
        // the suppression scope.
        assert_eq!(result.draws_used, 4);
        let draw_values: Vec<i64> = result.stochastic_draws.iter().map(|d| d.value).collect();
        assert_eq!(draw_values, vec![6, 16, 3, 19]);

        // roll_node (RollUnder's own Advantage: lower of the two wins):
        // min(6, 19) = 6.
        // target_node (Op::Roll's own Advantage: higher of the two wins,
        // drawn independently by the SAME active bias): max(16, 3) = 16.
        // effective_roll(6) <= target(16) -> true.
        assert!(result.value);
    }

    /// #1247: `stat` reads the BASE value and IGNORES the effective overlay
    /// (the ADR-0035 zirkularity cut). Even with an overlay raising AC to 21,
    /// `actor.stat("srd/ac") >= 15` reads the base 10 → false.
    fn ac_facts_with_overlay() -> EvaluationContext {
        EvaluationContext {
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Actor,
                field_id: "srd/ac".into(),
                value: 10,
            }],
            effective_int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Actor,
                field_id: "srd/ac".into(),
                value: 21,
            }],
            ..EvaluationContext::default()
        }
    }

    #[test]
    fn stat_reads_base_and_ignores_effective_overlay() {
        let expr = parse(r#"actor.stat("srd/ac") >= 15"#);
        let facts = ac_facts_with_overlay();
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(false)
        );
    }

    /// #1247: `effective` reads the composed overlay when present, else base.
    #[test]
    fn effective_prefers_overlay_then_falls_back_to_base() {
        let expr = parse(r#"actor.effective("srd/ac") >= 15"#);

        // Overlay (21) present → true.
        let with_overlay = ac_facts_with_overlay();
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(
                &expr,
                &with_overlay,
                &mut rng,
                EvaluationLimits::default(),
            ),
            Ok(true)
        );

        // No overlay → fall back to base (10) → false.
        let base_only = EvaluationContext {
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Actor,
                field_id: "srd/ac".into(),
                value: 10,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_predicate_expression(&expr, &base_only, &mut rng, EvaluationLimits::default()),
            Ok(false)
        );
    }

    /// #30 / sk-ct8.2: `resource_max`/`resource_min` read the per-subject
    /// `resource_bounds` fact — present ⇒ the typed value; absent ⇒
    /// `UnknownField` (ADR-0035 D-4 missing-fact), same as every other read op.
    #[test]
    fn resource_max_and_min_read_the_resource_bounds_fact_and_error_when_absent() {
        let facts = EvaluationContext {
            resource_bounds: vec![SubjectResourceBoundFact {
                subject: SubjectRef::Target,
                field_id: "srd/hp".into(),
                min_value: 5,
                max_value: 15,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);

        let max_expr = parse(r#"target.resource_max("srd/hp") == 15"#);
        assert_eq!(
            evaluate_predicate_expression(&max_expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );
        let min_expr = parse(r#"target.resource_min("srd/hp") == 5"#);
        assert_eq!(
            evaluate_predicate_expression(&min_expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );

        // Wrong subject (Actor, not Target) ⇒ absent ⇒ UnknownField.
        let actor_max_expr = parse(r#"actor.resource_max("srd/hp") == 15"#);
        assert!(matches!(
            evaluate_predicate_expression(
                &actor_max_expr,
                &facts,
                &mut rng,
                EvaluationLimits::default()
            ),
            Err(EvaluationError::UnknownField { .. })
        ));

        // Entity with no resource_bounds facts at all ⇒ UnknownField, never a
        // fabricated zero.
        assert!(matches!(
            evaluate_predicate_expression(
                &max_expr,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default()
            ),
            Err(EvaluationError::UnknownField { .. })
        ));
    }

    #[test]
    fn event_binding_reads_kind_and_detail_fields() {
        let expr = parse(r#"$event.kind == "door_opened" && $event.detail.item_id >= 2"#);
        let facts = EvaluationContext {
            event_kind: Some("door_opened".into()),
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Event(EventRef::Detail {
                    field_id: "item_id".into(),
                }),
                field_id: "item_id".into(),
                value: 3,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);

        assert_eq!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Ok(true)
        );
    }

    #[test]
    fn int_roll_table_validation_names_non_uniform_entry() {
        let expr = parse(r#"roll_table('core/mixed') >= 1"#);
        let entries = vec![
            RollTableEntryFact {
                table_id: "core/mixed".into(),
                entry_id: "core/mixed:1".into(),
                weight: 1,
                result_ref: "1".into(),
            },
            RollTableEntryFact {
                table_id: "core/mixed".into(),
                entry_id: "core/mixed:bad".into(),
                weight: 1,
                result_ref: "core/outcome".into(),
            },
        ];

        assert_eq!(
            validate_int_roll_table_usage(&expr, &entries),
            // node_id 3: the literal `roll_table('core/mixed')` now parses to a
            // TextLiteral node (the table id) + the IntLiteral draw_node before
            // the RollTable node (Option C). Static validation still runs because
            // the table id is a literal.
            Err(RollTableIntValidationError::NonUniformEntry {
                node_id: 3,
                table_id: "core/mixed".into(),
                entry_id: "core/mixed:bad".into(),
                result_ref: "core/outcome".into(),
            })
        );
    }

    #[test]
    fn int_roll_table_runtime_rejects_non_uniform_entry() {
        let expr = parse(r#"roll_table('core/mixed') >= 1"#);
        let facts = EvaluationContext {
            roll_table_entries: vec![RollTableEntryFact {
                table_id: "core/mixed".into(),
                entry_id: "core/mixed:bad".into(),
                weight: 1,
                result_ref: "core/outcome".into(),
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![0]);

        assert!(matches!(
            evaluate_predicate_expression(&expr, &facts, &mut rng, EvaluationLimits::default()),
            Err(EvaluationError::NonUniformRollTableIntEntry { entry_id, .. })
                if entry_id == "core/mixed:bad"
        ));
    }

    // --- G3 (#1254): runtime-resolvable roll_table table-id (Option C) ---------

    #[test]
    fn runtime_roll_table_parses_to_readtext_table_id() {
        let expr = crate::parse_int_expression(
            r#"roll_table(actor.wielded_item.text("srd/damage_profile"))"#,
        )
        .expect("runtime roll_table should parse");
        let root = expr.node(expr.root_node_id).expect("root");
        let Op::RollTable(rt) = &root.op else {
            panic!("expected RollTable root, got {:?}", root.op);
        };
        let id_node = expr.node(rt.table_id_node).expect("table id node");
        assert!(
            matches!(
                &id_node.op,
                Op::ReadText(ReadField { subject: SubjectRef::ActorWieldedItem, field_id })
                    if field_id == "srd/damage_profile"
            ),
            "table id must be ReadText(ActorWieldedItem, ...), got {:?}",
            id_node.op
        );
    }

    #[test]
    fn literal_roll_table_parses_to_textliteral_table_id() {
        let expr = crate::parse_int_expression(r#"roll_table("srd/profile/1d6")"#)
            .expect("literal roll_table should parse");
        let root = expr.node(expr.root_node_id).expect("root");
        let Op::RollTable(rt) = &root.op else {
            panic!("expected RollTable root");
        };
        let id_node = expr.node(rt.table_id_node).expect("table id node");
        assert!(matches!(
            &id_node.op,
            Op::TextLiteral(TextLiteral { value }) if value == "srd/profile/1d6"
        ));
    }

    #[test]
    fn runtime_roll_table_costs_exactly_one_static_draw() {
        // Budget-soundness falsifier vs the rejected literal-dice shape: a runtime
        // roll_table is a static 1 draw regardless of table; the ReadText table-id
        // node adds none (ADR-0059 sound).
        let expr = crate::parse_int_expression(
            r#"roll_table(actor.wielded_item.text("srd/damage_profile"))"#,
        )
        .expect("parse");
        assert_eq!(count_static_draws(&expr), 1);
    }

    #[test]
    fn runtime_roll_table_diverges_by_wielded_profile_pointer() {
        // The acceptance, at eval level: the SAME expression draws from DIFFERENT
        // tables according to the wielded item's profile pointer.
        let expr = crate::parse_int_expression(
            r#"roll_table(actor.wielded_item.text("srd/damage_profile"))"#,
        )
        .expect("parse");
        let facts_for = |profile: &str, table: &str, value: &str| EvaluationContext {
            text_fields: vec![SubjectTextFact {
                subject: SubjectRef::ActorWieldedItem,
                field_id: "srd/damage_profile".into(),
                value: profile.into(),
            }],
            roll_table_entries: vec![RollTableEntryFact {
                table_id: table.into(),
                entry_id: format!("{table}:1"),
                weight: 1,
                result_ref: value.into(),
            }],
            ..EvaluationContext::default()
        };
        let a = facts_for("srd/profile/flat4", "srd/profile/flat4", "4");
        let b = facts_for("srd/profile/flat8", "srd/profile/flat8", "8");
        let mut rng_a = DeterministicRng::new(vec![0]);
        let mut rng_b = DeterministicRng::new(vec![0]);
        assert_eq!(
            evaluate_int_expression(&expr, &a, &mut rng_a, EvaluationLimits::default()),
            Ok(4)
        );
        assert_eq!(
            evaluate_int_expression(&expr, &b, &mut rng_b, EvaluationLimits::default()),
            Ok(8)
        );
    }

    #[test]
    fn runtime_roll_table_fails_closed_when_pointer_missing() {
        // No wielded profile pointer → ReadText UnknownField (fail closed,
        // ADR-0035), never a panic.
        let expr = crate::parse_int_expression(
            r#"roll_table(actor.wielded_item.text("srd/damage_profile"))"#,
        )
        .expect("parse");
        let mut rng = DeterministicRng::new(vec![0]);
        assert!(matches!(
            evaluate_int_expression(
                &expr,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default()
            ),
            Err(EvaluationError::UnknownField { .. })
        ));
    }

    // ── sk-84i.2: `<subject>.source('slot')` (widened G-1) ──────────────

    fn item_source(subject: SubjectRef, slot: &str, source: &str) -> ItemDslSourceFact {
        ItemDslSourceFact {
            subject,
            slot: slot.into(),
            expr: crate::parse_int_expression(source).expect("item DSL source fixture parses"),
        }
    }

    fn wielded_item_source(slot: &str, source: &str) -> ItemDslSourceFact {
        item_source(SubjectRef::ActorWieldedItem, slot, source)
    }

    #[test]
    fn wielded_source_evaluates_the_wielded_items_named_slot() {
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let facts = EvaluationContext {
            item_dsl_sources: vec![wielded_item_source("srd/damage", "2d8")],
            ..EvaluationContext::default()
        };
        // Raw entropy 0 maps to face 1 on a d8 (`(0 % 8) + 1`): 2d8 == 1 + 1.
        let mut rng = DeterministicRng::new(vec![0, 0]);
        let report = evaluate_predicate_expression_with_report(
            &bool_wrap(expr.clone(), 2),
            &facts,
            &mut rng,
            EvaluationLimits::for_expression(&expr),
        );
        // bool_wrap compares the int result to 2 — see helper below.
        assert_eq!(report.map(|r| r.value), Ok(true));
    }

    /// Wraps an int-valued expression's root as the left side of `== rhs`, so
    /// the shared `evaluate_predicate_expression_with_report` entry point
    /// (which asserts on Predicate kind) can exercise an `Op::DslSource`
    /// node without a bespoke int-eval report helper.
    fn bool_wrap(mut expr: Expression, rhs: i64) -> Expression {
        let rhs_id = expr.nodes.iter().map(|n| n.node_id).max().unwrap_or(0) + 1;
        let root_id = rhs_id + 1;
        expr.nodes.push(Node {
            node_id: rhs_id,
            value_type: ValueType::Int,
            op: Op::IntLiteral(IntLiteral { value: rhs }),
            span: crate::SourceSpan::default(),
        });
        expr.nodes.push(Node {
            node_id: root_id,
            value_type: ValueType::Bool,
            op: Op::Compare(Compare {
                op: CompareOp::Eq,
                left: expr.root_node_id,
                right: rhs_id,
            }),
            span: crate::SourceSpan::default(),
        });
        expr.root_node_id = root_id;
        expr.kind = ExpressionKind::Predicate;
        expr
    }

    #[test]
    fn wielded_source_reads_actor_stat_from_the_callers_shared_fact_context() {
        // Design decision (sk-84i.2): the item source evaluates against the
        // SAME `EvaluationContext` as the caller — `actor` inside the item
        // expression is the wielder, because it is literally the SAME
        // `SubjectRef::Actor` fact pool, never re-scoped. This is also the
        // falsifying fixture for the K-5 rationale (canon-warden A-4): a
        // roll-table entry is a static `result_ref` and can NEVER compose
        // with the caller's own facts like this — only a spliced-in
        // evaluated node can.
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let facts = EvaluationContext {
            item_dsl_sources: vec![wielded_item_source(
                "srd/damage",
                "actor.stat('core/str_mod')",
            )],
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Actor,
                field_id: "core/str_mod".into(),
                value: 5,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_int_expression(
                &expr,
                &facts,
                &mut rng,
                EvaluationLimits::for_expression(&expr)
            ),
            Ok(5)
        );
    }

    #[test]
    fn wielded_source_fails_closed_with_no_facts_when_call_site_never_populates() {
        // D-1 split: an EMPTY `item_dsl_sources` pool is the "call site never
        // wires this fact family" shape (e.g. `nothing wielded`, OR a kernel
        // seam that never routes through `manipulation_facts`) —
        // `NoItemDslSourceFacts`, not the misleading old bare
        // `MissingWieldedItemSource` ("nothing wielded" asserted even when
        // the seam simply never populated the pool).
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let mut rng = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_int_expression(
                &expr,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::for_expression(&expr)
            ),
            Err(EvaluationError::NoItemDslSourceFacts { slot, subject, .. })
                if slot == "srd/damage" && subject == SubjectRef::ActorWieldedItem
        ));
    }

    #[test]
    fn wielded_source_fails_closed_when_slot_absent_on_wielded_item() {
        // D-1 split: the pool IS populated (this call site wires it) but
        // carries no match for THIS slot — a genuine missing fact,
        // `MissingItemDslSource`, distinct from `NoItemDslSourceFacts` above.
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let facts = EvaluationContext {
            // The wielded item DOES carry sources — just not this slot.
            item_dsl_sources: vec![wielded_item_source("srd/reach", "5")],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_int_expression(
                &expr,
                &facts,
                &mut rng,
                EvaluationLimits::for_expression(&expr)
            ),
            Err(EvaluationError::MissingItemDslSource { slot, subject, .. })
                if slot == "srd/damage" && subject == SubjectRef::ActorWieldedItem
        ));
    }

    #[test]
    fn wielded_source_fails_closed_when_slot_present_for_a_different_subject() {
        // Subject widening (G-1): a slot authored under one subject does NOT
        // leak to a same-named slot request under a different subject — the
        // fact pool is keyed by `(subject, slot)`, not `slot` alone.
        let expr = crate::parse_int_expression(r#"actor.source('srd/damage')"#).expect("parse");
        let facts = EvaluationContext {
            item_dsl_sources: vec![wielded_item_source("srd/damage", "2d8")],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_int_expression(
                &expr,
                &facts,
                &mut rng,
                EvaluationLimits::for_expression(&expr)
            ),
            Err(EvaluationError::MissingItemDslSource { slot, subject, .. })
                if slot == "srd/damage" && subject == SubjectRef::Actor
        ));
    }

    #[test]
    fn wielded_source_recursion_is_depth_limited_to_one() {
        // The item's OWN source calls `.source(...)` again — fails closed
        // rather than recursing unboundedly.
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let facts = EvaluationContext {
            item_dsl_sources: vec![wielded_item_source(
                "srd/damage",
                "actor.wielded_item.source('srd/damage')",
            )],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_int_expression(
                &expr,
                &facts,
                &mut rng,
                EvaluationLimits::for_expression(&expr)
            ),
            Err(EvaluationError::DslSourceRecursionLimit { slot, .. }) if slot == "srd/damage"
        ));
    }

    #[test]
    fn count_static_draws_charges_the_full_ceiling_for_wielded_source() {
        // ADR-0059: the static walk cannot see into the item's data-driven
        // source, so it charges the full ceiling rather than under-counting.
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        assert_eq!(count_static_draws(&expr), MAX_DRAW_CEILING);
        assert_eq!(
            EvaluationLimits::for_expression(&expr).max_draws,
            MAX_DRAW_CEILING
        );
    }

    #[test]
    fn wielded_source_bomb_expression_trips_the_draw_ceiling_instead_of_running_unbounded() {
        // A "bomb" item source (300 dice > MAX_DRAW_CEILING) is rejected with
        // DrawLimitExceeded rather than drawing 300 times.
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let facts = EvaluationContext {
            item_dsl_sources: vec![wielded_item_source("srd/damage", "300d6")],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![0; 300]);
        assert!(matches!(
            evaluate_int_expression(
                &expr,
                &facts,
                &mut rng,
                EvaluationLimits::for_expression(&expr)
            ),
            Err(EvaluationError::DrawLimitExceeded { .. })
        ));
    }

    #[test]
    fn wielded_source_multi_die_formula_charges_the_shared_draw_budget_and_records_provenance() {
        // Content proof shape (sk-84i.2 item 9): a multi-die item formula
        // (2d8) rolls live and its draws are folded back into the CALLER's
        // provenance — the same `StochasticDraw` record a directly-authored
        // `2d8` in the caller's own expression would produce.
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let facts = EvaluationContext {
            item_dsl_sources: vec![wielded_item_source("srd/damage", "2d8")],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![3, 5]);
        let bool_expr = bool_wrap(expr, 10); // rhs unused; we only need the report
        let report = evaluate_predicate_expression_with_report(
            &bool_expr,
            &facts,
            &mut rng,
            EvaluationLimits::for_expression(&bool_expr),
        )
        .expect("evaluates");
        assert_eq!(report.draws_used, 2);
        assert_eq!(report.stochastic_draws.len(), 2);
        assert_eq!(report.stochastic_draws[0].value, 4); // (3 % 8) + 1
        assert_eq!(report.stochastic_draws[1].value, 6); // (5 % 8) + 1
    }

    #[test]
    fn wielded_source_evaluates_a_commit_cached_macro_expanded_tree() {
        // #2218 twin: act time evaluates the cached tree, not raw source.
        // A `macro(...)` call never appears in this tree — commit already
        // expanded it to ordinary arithmetic / field reads.
        let expr = crate::parse_int_expression(r#"actor.wielded_item.source('srd/damage')"#)
            .expect("parse");
        let cached = crate::compile_int_expression_with_macros(
            "macro('srd/ability_modifier', actor.stat('srd/str'))",
            &[crate::DslMacroDefinition {
                macro_id: "srd/ability_modifier".into(),
                return_type: ValueType::Int,
                parameters: vec![crate::DslMacroParameter {
                    name: "score".into(),
                    value_type: ValueType::Int,
                }],
                body_source: "(score - 10) / 2".into(),
            }],
        )
        .expect("macro expands at commit");
        assert!(
            cached
                .nodes
                .iter()
                .all(|node| node.op.variant_name() != "MacroCall"),
            "cached tree must be ordinary DSL ops, not a leftover macro call"
        );
        let facts = EvaluationContext {
            item_dsl_sources: vec![ItemDslSourceFact {
                subject: SubjectRef::ActorWieldedItem,
                slot: "srd/damage".into(),
                expr: cached,
            }],
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Actor,
                field_id: "srd/str".into(),
                value: 16,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert_eq!(
            evaluate_int_expression(
                &expr,
                &facts,
                &mut rng,
                EvaluationLimits::for_expression(&expr)
            ),
            Ok(3)
        );
    }

    #[test]
    fn let_bound_roll_tables_draw_independently() {
        let expr = parse(
            r#"let a = roll_table('core/d20'); let b = roll_table('core/d20'); min(a, b) == 1"#,
        );
        let facts = EvaluationContext {
            roll_table_entries: d20_entries(),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![1, 0]);
        let result = evaluate_predicate_expression_with_report(
            &expr,
            &facts,
            &mut rng,
            EvaluationLimits::default(),
        )
        .expect("let-bound roll tables should evaluate");

        assert!(result.value);
        assert_eq!(result.draws_used, 2);
        assert_eq!(
            result
                .stochastic_draws
                .iter()
                .map(|draw| draw.value)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
    }

    #[test]
    fn let_reuses_value_without_redrawing() {
        let expr = parse(r#"let a = roll_table('core/d20'); a == a"#);
        let facts = EvaluationContext {
            roll_table_entries: d20_entries(),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![1, 0]);
        let result = evaluate_predicate_expression_with_report(
            &expr,
            &facts,
            &mut rng,
            EvaluationLimits::default(),
        )
        .expect("let-bound roll table should evaluate");

        assert!(result.value);
        assert_eq!(result.draws_used, 1);
        assert_eq!(
            result
                .stochastic_draws
                .iter()
                .map(|draw| draw.value)
                .collect::<Vec<_>>(),
            vec![2]
        );
    }

    #[test]
    fn non_int_roll_table_preserves_unsupported_predicate_refusal() {
        let expr = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 2,
            nodes: vec![
                node(1, ValueType::Int, Op::IntLiteral(IntLiteral { value: 0 })),
                node(
                    2,
                    ValueType::Outcome,
                    Op::RollTable(RollTable {
                        table_id_node: 3,
                        draw_node: 1,
                    }),
                ),
                node(
                    3,
                    ValueType::Text,
                    Op::TextLiteral(TextLiteral {
                        value: "core/cairn_scars".into(),
                    }),
                ),
            ],
        };
        let mut rng = DeterministicRng::new(vec![0]);

        assert_eq!(
            evaluate_predicate_expression(
                &expr,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default(),
            ),
            Err(EvaluationError::UnsupportedInPredicate { node_id: 2 })
        );
    }

    #[test]
    fn runtime_evaluator_enforces_node_depth_draw_and_cycle_bounds() {
        let too_many_nodes = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 1,
            nodes: vec![
                node(
                    1,
                    ValueType::Bool,
                    Op::BoolLiteral(BoolLiteral { value: true }),
                ),
                node(
                    2,
                    ValueType::Bool,
                    Op::BoolLiteral(BoolLiteral { value: true }),
                ),
            ],
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_predicate_expression(
                &too_many_nodes,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits {
                    max_nodes: 1,
                    ..EvaluationLimits::default()
                }
            ),
            Err(EvaluationError::NodeLimitExceeded { .. })
        ));

        let deep = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 2,
            nodes: vec![
                node(
                    1,
                    ValueType::Bool,
                    Op::BoolLiteral(BoolLiteral { value: true }),
                ),
                node(2, ValueType::Bool, Op::Not(UnaryNode { operand: 1 })),
            ],
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_predicate_expression(
                &deep,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits {
                    max_depth: 1,
                    ..EvaluationLimits::default()
                }
            ),
            Err(EvaluationError::DepthLimitExceeded { .. })
        ));

        let roll = parse(r#"roll_under(d20, 10)"#);
        let mut rng = DeterministicRng::new(vec![3]);
        assert!(matches!(
            evaluate_predicate_expression(
                &roll,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits {
                    max_draws: 0,
                    ..EvaluationLimits::default()
                }
            ),
            Err(EvaluationError::DrawLimitExceeded { .. })
        ));

        let cycle = Expression {
            kind: ExpressionKind::Predicate,
            root_node_id: 1,
            nodes: vec![
                node(1, ValueType::Bool, Op::Not(UnaryNode { operand: 2 })),
                node(2, ValueType::Bool, Op::Not(UnaryNode { operand: 1 })),
            ],
        };
        let mut rng = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_predicate_expression(
                &cycle,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default(),
            ),
            Err(EvaluationError::CycleDetected { .. })
        ));
    }

    /// K-5 (ISS1627): an under-provisioned replay stream must fail loud with
    /// `DrawStreamExhausted`, never silently degrade every over-drawn die to
    /// a natural 1 (`DeterministicRng::next_u32`'s old `unwrap_or(0)`).
    #[test]
    fn under_provisioned_draw_stream_fails_loud_instead_of_silent_ones() {
        let two_dice = parse(r#"2d20 >= 1"#);

        let mut starved = DeterministicRng::new(vec![19]);
        assert!(matches!(
            evaluate_predicate_expression(
                &two_dice,
                &EvaluationContext::default(),
                &mut starved,
                EvaluationLimits::default(),
            ),
            Err(EvaluationError::DrawStreamExhausted { attempted: 2, .. })
        ));

        let mut provisioned = DeterministicRng::new(vec![19, 5]);
        let result = evaluate_predicate_expression(
            &two_dice,
            &EvaluationContext::default(),
            &mut provisioned,
            EvaluationLimits::default(),
        )
        .expect("a fully-provisioned draw stream still evaluates");
        assert!(result);
    }

    /// K-5 (ISS1627): the same fail-loud contract for the `roll_table_int`
    /// draw site, which pulls its single ticket via `next_u32` directly
    /// rather than through `roll_die`.
    #[test]
    fn under_provisioned_roll_table_draw_stream_fails_loud() {
        let expr = parse(r#"roll_table('core/d20') >= 1"#);
        let facts = EvaluationContext {
            roll_table_entries: d20_entries(),
            ..EvaluationContext::default()
        };

        let mut starved = DeterministicRng::new(vec![]);
        assert!(matches!(
            evaluate_predicate_expression(&expr, &facts, &mut starved, EvaluationLimits::default(),),
            Err(EvaluationError::DrawStreamExhausted { attempted: 1, .. })
        ));
    }

    #[test]
    fn validation_errors_come_from_shared_ast_validation() {
        let expr = Expression {
            kind: ExpressionKind::Outcome,
            root_node_id: 1,
            nodes: vec![node(
                1,
                ValueType::Bool,
                Op::BoolLiteral(BoolLiteral { value: true }),
            )],
        };
        let mut rng = DeterministicRng::new(vec![]);

        assert!(matches!(
            evaluate_predicate_expression(
                &expr,
                &EvaluationContext::default(),
                &mut rng,
                EvaluationLimits::default()
            ),
            Err(EvaluationError::Validation(
                ValidationError::KindMismatch { .. }
            ))
        ));
    }

    // ── #1891: offer/materialise-time attemptability (`evaluate_offer_predicate_expression`) ──
    //
    // These cases exercise bare and compound stochastic predicates.
    // .toml` affordance), and `roll_under(d20, N)` (`hold-monster.toml:74`).

    #[test]
    fn offer_eval_satisfies_a_bare_stochastic_predicate_with_no_deterministic_conjunct() {
        // `predicate_source = "d20 >= 9"` — no conjunct at all gates this one.
        // A real d20 can obviously roll < 9; offer mode must still say "yes,
        // attemptable" — the die never belongs in the offer decision.
        let expr = parse("d20 >= 9");
        let result = evaluate_offer_predicate_expression(
            &expr,
            &EvaluationContext::default(),
            EvaluationLimits::for_expression(&expr),
        );
        assert_eq!(
            result,
            Ok(true),
            "a bare stochastic predicate must always offer"
        );
    }

    #[test]
    fn offer_eval_satisfies_roll_under_predicate() {
        // `predicate_source = "roll_under(d20, 13)"`.
        let expr = parse("roll_under(d20, 13)");
        let result = evaluate_offer_predicate_expression(
            &expr,
            &EvaluationContext::default(),
            EvaluationLimits::for_expression(&expr),
        );
        assert_eq!(result, Ok(true), "roll_under must always offer");
    }

    #[test]
    fn offer_eval_satisfies_stochastic_comparison_but_still_gates_deterministic_conjunct() {
        // `actor.wielded_item.has_tag('weapon') && target.stat('srd/ac') >= 1 &&
        //  d20 + <mods> >= target.effective('srd/ac')` — simplified to the
        // load-bearing structure: one deterministic capability gate ANDed
        // with one stochastic to-hit comparison. #1891's regression: the
        // candidate must be offered on EVERY recompute when the deterministic
        // gate holds, and gated out when it does not — regardless of the die.
        let expr = parse(r#"actor.has_capability("core/wields_weapon") && d20 + 5 >= 25"#);
        let limits = EvaluationLimits::for_expression(&expr);

        let gated_in = EvaluationContext {
            capabilities: vec![SubjectIdFact {
                subject: SubjectRef::Actor,
                id: "core/wields_weapon".into(),
            }],
            ..EvaluationContext::default()
        };
        assert_eq!(
            evaluate_offer_predicate_expression(&expr, &gated_in, limits),
            Ok(true),
            "deterministic conjunct true ⇒ offered regardless of the (unresolved) d20, \
             even though `d20 + 5 >= 25` could never actually hit on a real d20"
        );

        let gated_out = EvaluationContext::default();
        assert_eq!(
            evaluate_offer_predicate_expression(&expr, &gated_out, limits),
            Ok(false),
            "the deterministic capability gate must still gate offer — attemptability is not \
             unconditional, only the die is short-circuited"
        );
    }

    // Distance in millimeters uses the ordinary target.stat fact lookup.

    #[test]
    fn distance_stat_reads_the_actor_target_distance_fact_and_errors_when_absent() {
        let expr = parse(r#"target.stat("core/distance_to_actor_mm") <= 1524"#);
        let mut rng = DeterministicRng::new(vec![]);

        // Absent (ADR-0035 D-4: cross-graph/incomparable/unresolvable metric
        // ⇒ no fact pushed, never a fabricated zero) ⇒ UnknownField, same as
        // any other missing stat read — the caller's usual `.unwrap_or(false)`
        // then makes the predicate false, never a wrong number.
        let missing = evaluate_predicate_expression(
            &expr,
            &EvaluationContext::default(),
            &mut rng,
            EvaluationLimits::default(),
        );
        assert!(matches!(missing, Err(EvaluationError::UnknownField { .. })));

        // The combat-arena grid boundary (`cell_size_m = 1.524`, 5 ft):
        // orthogonal-adjacent is exactly the gate; diagonal-adjacent
        // (~2155 mm) is just beyond it.
        let orthogonal_adjacent = EvaluationContext {
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Target,
                field_id: "core/distance_to_actor_mm".into(),
                value: 1524,
            }],
            ..EvaluationContext::default()
        };
        assert_eq!(
            evaluate_predicate_expression(
                &expr,
                &orthogonal_adjacent,
                &mut rng,
                EvaluationLimits::default()
            ),
            Ok(true),
            "orthogonal-adjacent (1524 mm) is within the 1524 mm gate"
        );

        let diagonal_adjacent = EvaluationContext {
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Target,
                field_id: "core/distance_to_actor_mm".into(),
                value: 2155,
            }],
            ..EvaluationContext::default()
        };
        assert_eq!(
            evaluate_predicate_expression(
                &expr,
                &diagonal_adjacent,
                &mut rng,
                EvaluationLimits::default()
            ),
            Ok(false),
            "diagonal-adjacent (~2155 mm) is beyond the 1524 mm gate"
        );
    }

    #[test]
    fn offer_eval_gates_on_a_deterministic_distance_conjunct_and_still_satisfies_the_stochastic_die(
    ) {
        // Distance scope:
        // distance is DETERMINISTIC — offer mode must evaluate it for real
        // (gate on it), never short-circuit it like a stochastic term, while
        // the unrelated d20 conjunct still gets the usual offer-mode
        // "satisfied without drawing" treatment. Mirrors the existing
        // `offer_eval_satisfies_stochastic_comparison_but_still_gates_deterministic_conjunct`
        // shape with a distance gate in place of the capability gate.
        let expr = parse(r#"target.stat("core/distance_to_actor_mm") <= 1524 && d20 + 5 >= 25"#);
        let limits = EvaluationLimits::for_expression(&expr);

        let within_range = EvaluationContext {
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Target,
                field_id: "core/distance_to_actor_mm".into(),
                value: 1524,
            }],
            ..EvaluationContext::default()
        };
        assert_eq!(
            evaluate_offer_predicate_expression(&expr, &within_range, limits),
            Ok(true),
            "within range ⇒ offered regardless of the (unresolved) d20, even though \
             `d20 + 5 >= 25` could never actually hit on a real d20"
        );

        let out_of_range = EvaluationContext {
            int_fields: vec![SubjectIntFact {
                subject: SubjectRef::Target,
                field_id: "core/distance_to_actor_mm".into(),
                value: 2155,
            }],
            ..EvaluationContext::default()
        };
        assert_eq!(
            evaluate_offer_predicate_expression(&expr, &out_of_range, limits),
            Ok(false),
            "the deterministic distance gate must still gate offer — only the die is \
             short-circuited, distance is not"
        );

        let absent = EvaluationContext::default();
        assert!(
            matches!(
                evaluate_offer_predicate_expression(&expr, &absent, limits),
                Err(EvaluationError::UnknownField { .. })
            ),
            "an absent distance fact must error the same way in offer mode as in real eval"
        );
    }

    #[test]
    fn offer_eval_matches_full_eval_byte_for_byte_when_predicate_has_no_dice() {
        // A fully deterministic predicate (no Roll/RollTable anywhere) must
        // behave identically under offer mode and the real evaluator — the
        // new entry point changes nothing for the overwhelming majority of
        // authored predicates that were never the bug.
        let expr = parse(
            r#"actor.has_capability("core/pickup") && target.state("core/grounded") == true"#,
        );
        let limits = EvaluationLimits::for_expression(&expr);
        let facts = EvaluationContext {
            capabilities: vec![SubjectIdFact {
                subject: SubjectRef::Actor,
                id: "core/pickup".into(),
            }],
            bool_fields: vec![SubjectBoolFact {
                subject: SubjectRef::Target,
                field_id: "core/grounded".into(),
                value: true,
            }],
            ..EvaluationContext::default()
        };

        let offer_result = evaluate_offer_predicate_expression(&expr, &facts, limits);
        let mut rng = DeterministicRng::new(vec![]);
        let real_result = evaluate_predicate_expression(&expr, &facts, &mut rng, limits);
        assert_eq!(offer_result, real_result);
        assert_eq!(offer_result, Ok(true));
    }

    #[test]
    fn offer_eval_satisfies_stochastic_cond_branch_nested_in_a_compare() {
        // `cond(actor.status('srd/blessed'), d20 + 1d4, d20) >= 21` — the
        // Cond's gate is deterministic, but BOTH branches draw. Proves the
        // recursive `subtree_has_stochastic_draw` walk reaches through Cond,
        // not just a flat top-level Compare operand.
        let expr = parse("cond(actor.status(\"srd/blessed\"), d20 + 1, d20) >= 21");
        let result = evaluate_offer_predicate_expression(
            &expr,
            &EvaluationContext::default(),
            EvaluationLimits::for_expression(&expr),
        );
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn offer_eval_satisfies_a_die_laundered_through_a_let_binding() {
        // The binding name has no AST child edge back to the dice expression.
        let expr = parse("let roll = 1d20 ; roll >= 15");
        let result = evaluate_offer_predicate_expression(
            &expr,
            &EvaluationContext::default(),
            EvaluationLimits::for_expression(&expr),
        );
        assert_eq!(
            result,
            Ok(true),
            "a die laundered through a let-binding must still offer, not panic or error"
        );
    }

    #[test]
    fn offer_eval_propagates_stochastic_bindings_through_a_chained_let() {
        // `let` values are Int-only in this grammar (`parse_let_bool`/
        // `parse_let_int` both route the value through `parse_int_expr`), so
        // the nested case is a second Int let whose OWN value references the
        // first (deferred, never-evaluated) stochastic binding — here via
        // arithmetic, not a bare re-read. Proves `stochastic_bindings`
        // propagates through a chain, not just one hop: the inner `let`
        // must ALSO defer (its value's subtree resolves "roll" as
        // stochastic via the binding-aware walk), so the final Compare over
        // `doubled` is satisfied without ever drawing.
        let expr = parse("let roll = d20 ; let doubled = roll + roll ; doubled >= 30");
        let result = evaluate_offer_predicate_expression(
            &expr,
            &EvaluationContext::default(),
            EvaluationLimits::for_expression(&expr),
        );
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn offer_eval_let_bound_die_still_respects_a_deterministic_conjunct() {
        // Ground shape closest to the real authored predicate: a `let`-bound
        // die feeding a stochastic comparison, ANDed with a deterministic
        // capability gate. The die must never gate offer; the capability
        // gate still must.
        let expr =
            parse(r#"let roll = d20 ; actor.has_capability("core/wields_weapon") && roll >= 15"#);
        let limits = EvaluationLimits::for_expression(&expr);

        let gated_in = EvaluationContext {
            capabilities: vec![SubjectIdFact {
                subject: SubjectRef::Actor,
                id: "core/wields_weapon".into(),
            }],
            ..EvaluationContext::default()
        };
        assert_eq!(
            evaluate_offer_predicate_expression(&expr, &gated_in, limits),
            Ok(true)
        );

        let gated_out = EvaluationContext::default();
        assert_eq!(
            evaluate_offer_predicate_expression(&expr, &gated_out, limits),
            Ok(false)
        );
    }

    #[test]
    fn offer_eval_rejects_oversized_expression_same_as_real_eval() {
        // Structural errors (node-limit, kind-mismatch) still surface —
        // offer mode changes stochastic semantics only, not validation.
        let expr = Expression {
            kind: ExpressionKind::Outcome,
            root_node_id: 1,
            nodes: vec![node(
                1,
                ValueType::Bool,
                Op::BoolLiteral(BoolLiteral { value: true }),
            )],
        };
        assert!(matches!(
            evaluate_offer_predicate_expression(
                &expr,
                &EvaluationContext::default(),
                EvaluationLimits::default()
            ),
            Err(EvaluationError::Validation(
                ValidationError::KindMismatch { .. }
            ))
        ));
    }

    // ── Traced evaluation tests (Tasks 1.1 - 1.5) ──────────────────────────

    #[test]
    fn untraced_state_does_not_construct_trace_metadata() {
        let expr = parse_int_expression("roll_table('core/d20')").unwrap();
        let context = EvaluationContext {
            roll_table_entries: d20_entries(),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![0]);
        let mut state = EvaluationState {
            expr: &expr,
            facts: &context,
            rng: &mut rng,
            limits: EvaluationLimits::for_expression(&expr),
            draws_used: 0,
            stochastic_draws: Vec::new(),
            active_nodes: BTreeSet::new(),
            lexical_bindings: Vec::new(),
            suppress_roll_bias: false,
            offer_satisfiable_mode: false,
            stochastic_bindings: BTreeSet::new(),
            dsl_source_depth: 0,
            trace: None,
            current_scope: Vec::new(),
            pending_roll_table_selections: None,
        };
        let scope = state.nested_trace_scope(&SubjectRef::Actor, "damage");
        assert!(scope.is_empty());
        assert_eq!(scope.capacity(), 0);
        assert_eq!(state.eval_int(expr.root_node_id, 1), Ok(1));
        assert!(state.pending_roll_table_selections.is_none());
        assert!(state.trace.is_none());
        assert_eq!(state.stochastic_draws.len(), 1);
    }

    #[test]
    fn traced_roll_table_marks_the_kept_draw_for_both_biases() {
        let expr = parse_int_expression("roll_table('core/d20')").unwrap();
        for (modifier, expected, kept) in [
            (RollModifier::Advantage, 2, 0),
            (RollModifier::Disadvantage, 1, 1),
        ] {
            let context = EvaluationContext {
                roll_table_entries: d20_entries(),
                roll_modifier_bias: Some(ComposedRollBias {
                    roll_modifier: Some(modifier),
                    flat_bonus: 0,
                }),
                ..EvaluationContext::default()
            };
            let mut rng = DeterministicRng::new(vec![7, 2]);
            let result = evaluate_expression_traced(
                &expr,
                &context,
                &mut rng,
                EvaluationLimits::for_expression(&expr),
            )
            .unwrap();
            assert_eq!(result.value, EvaluationValue::Int(expected));
            let selections = result.trace.last().unwrap().table_entries.as_ref().unwrap();
            assert_eq!(selections.len(), 2);
            assert!(selections[kept].kept);
            assert!(!selections[1 - kept].kept);
        }
    }

    #[test]
    fn traced_eval_false_conjunct_and_short_circuit() {
        let expr = parse("1 == 1 && 1 == 2 && 1 == 1");
        let limits = EvaluationLimits::default();
        let context = EvaluationContext::default();
        let mut rng = DeterministicRng::new(vec![]);

        let result = evaluate_expression_traced(&expr, &context, &mut rng, limits)
            .expect("should evaluate without error");
        assert_eq!(result.value, EvaluationValue::Bool(false));

        assert_eq!(result.trace.last().unwrap().node_id, expr.root_node_id);
        assert_eq!(
            result.trace.last().unwrap().value,
            Some(EvaluationValue::Bool(false))
        );

        let op_names: Vec<&str> = result
            .trace
            .iter()
            .map(|e| e.operator.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(op_names.last(), Some(&"And"));

        // Compare nodes:
        // First operand (1 == 1): produced true
        // Second operand (1 == 2): produced false
        // Third operand (1 == 1): was short-circuited and has NO entries
        let compare_values: Vec<bool> = result
            .trace
            .iter()
            .filter(|e| e.operator.as_deref() == Some("Compare"))
            .filter_map(|e| e.value.as_ref()?.as_bool())
            .collect();
        assert_eq!(compare_values, vec![true, false]);
    }

    #[test]
    fn traced_eval_untaken_cond_branch_has_no_entry() {
        let expr = parse_int_expression("cond(1 == 1, 10, 20)").expect("should parse");
        let limits = EvaluationLimits::default();
        let context = EvaluationContext::default();
        let mut rng = DeterministicRng::new(vec![]);

        let result =
            evaluate_expression_traced(&expr, &context, &mut rng, limits).expect("should evaluate");
        assert_eq!(result.value, EvaluationValue::Int(10));

        let int_values: Vec<i64> = result
            .trace
            .iter()
            .filter_map(|e| e.value.as_ref()?.as_int())
            .collect();
        assert!(int_values.contains(&10));
        assert!(!int_values.contains(&20));
        assert_eq!(result.trace.last().unwrap().node_id, expr.root_node_id);
    }

    #[test]
    fn traced_eval_missing_fact_first_error_and_ancestors() {
        let expr = parse("actor.stat('core/missing') >= 10");
        let limits = EvaluationLimits::default();
        let context = EvaluationContext::default();
        let mut rng = DeterministicRng::new(vec![]);

        let err = evaluate_expression_traced(&expr, &context, &mut rng, limits)
            .expect_err("should fail with missing fact");

        assert!(matches!(err.error, EvaluationError::UnknownField { .. }));
        assert!(!err.trace.is_empty());

        let expected_err_str = err.error.to_string();
        let first_err_idx = err
            .trace
            .iter()
            .position(|e| e.error.is_some())
            .expect("must have an error entry");
        assert_eq!(
            err.trace[first_err_idx].error.as_deref(),
            Some(expected_err_str.as_str())
        );
        assert_eq!(
            err.trace[first_err_idx].operator.as_deref(),
            Some("ReadInt")
        );

        for entry in &err.trace[first_err_idx..] {
            assert_eq!(entry.error.as_deref(), Some(expected_err_str.as_str()));
        }

        assert_eq!(err.trace.last().unwrap().node_id, expr.root_node_id);
    }

    #[test]
    fn traced_eval_depth_limit_entry() {
        let expr = parse("1 == 1 && 1 == 1");
        let limits = EvaluationLimits {
            max_depth: 0,
            max_nodes: 10,
            max_draws: 10,
        };
        let context = EvaluationContext::default();
        let mut rng = DeterministicRng::new(vec![]);

        let err = evaluate_expression_traced(&expr, &context, &mut rng, limits)
            .expect_err("should fail depth limit");
        assert!(matches!(
            err.error,
            EvaluationError::DepthLimitExceeded { .. }
        ));
        assert_eq!(err.trace.len(), 1);
        assert_eq!(err.trace[0].node_id, expr.root_node_id);
        assert_eq!(
            err.trace[0].error.as_deref(),
            Some(err.error.to_string().as_str())
        );
    }

    #[test]
    fn traced_eval_nested_scope_and_overlapping_node_ids() {
        let outer_expr =
            parse_int_expression("actor.source('srd/damage') + 10").expect("parse outer");
        let nested_expr = parse_int_expression("5 + 6").expect("parse nested");

        let context = EvaluationContext {
            item_dsl_sources: vec![ItemDslSourceFact {
                subject: SubjectRef::Actor,
                slot: "srd/damage".into(),
                expr: nested_expr,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        let limits = EvaluationLimits::default();

        let result = evaluate_expression_traced(&outer_expr, &context, &mut rng, limits)
            .expect("should evaluate");
        assert_eq!(result.value, EvaluationValue::Int(21));

        let outer_entries: Vec<_> = result.trace.iter().filter(|e| e.scope.is_empty()).collect();
        assert!(!outer_entries.is_empty());

        let nested_entries: Vec<_> = result
            .trace
            .iter()
            .filter(|e| {
                e.scope.len() == 1
                    && e.scope[0].subject == SubjectRef::Actor
                    && e.scope[0].slot == "srd/damage"
            })
            .collect();
        assert!(!nested_entries.is_empty());

        let node1_entries: Vec<_> = result.trace.iter().filter(|e| e.node_id == 1).collect();
        assert!(node1_entries.len() >= 2);
        assert!(node1_entries.iter().any(|e| e.scope.is_empty()));
        assert!(node1_entries.iter().any(|e| !e.scope.is_empty()));
    }

    #[test]
    fn traced_eval_actor_and_target_same_slot_different_scopes() {
        let outer_expr =
            parse_int_expression("actor.source('srd/damage') + target.source('srd/damage')")
                .expect("parse outer");
        let nested_expr_actor = parse_int_expression("5").expect("parse actor slot");
        let nested_expr_target = parse_int_expression("7").expect("parse target slot");

        let context = EvaluationContext {
            item_dsl_sources: vec![
                ItemDslSourceFact {
                    subject: SubjectRef::Actor,
                    slot: "srd/damage".into(),
                    expr: nested_expr_actor,
                },
                ItemDslSourceFact {
                    subject: SubjectRef::Target,
                    slot: "srd/damage".into(),
                    expr: nested_expr_target,
                },
            ],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        let limits = EvaluationLimits::default();

        let result = evaluate_expression_traced(&outer_expr, &context, &mut rng, limits)
            .expect("should evaluate");
        assert_eq!(result.value, EvaluationValue::Int(12));

        let has_actor_scope = result.trace.iter().any(|e| {
            e.scope.len() == 1
                && e.scope[0].subject == SubjectRef::Actor
                && e.scope[0].slot == "srd/damage"
        });
        let has_target_scope = result.trace.iter().any(|e| {
            e.scope.len() == 1
                && e.scope[0].subject == SubjectRef::Target
                && e.scope[0].slot == "srd/damage"
        });
        assert!(has_actor_scope);
        assert!(has_target_scope);
    }

    #[test]
    fn traced_eval_nested_missing_fact_keeps_nested_entries() {
        let outer_expr =
            parse_int_expression("actor.source('srd/damage') + 10").expect("parse outer");
        let nested_expr =
            parse_int_expression("actor.stat('core/missing') + 1").expect("parse nested");

        let context = EvaluationContext {
            item_dsl_sources: vec![ItemDslSourceFact {
                subject: SubjectRef::Actor,
                slot: "srd/damage".into(),
                expr: nested_expr,
            }],
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::new(vec![]);
        let limits = EvaluationLimits::default();

        let err = evaluate_expression_traced(&outer_expr, &context, &mut rng, limits)
            .expect_err("should fail with missing fact inside nested source");

        let nested_entries: Vec<_> = err.trace.iter().filter(|e| !e.scope.is_empty()).collect();
        assert!(!nested_entries.is_empty());
        assert!(nested_entries.iter().any(|e| e.error.is_some()));

        let outer_source_entry = err
            .trace
            .iter()
            .find(|e| e.operator.as_deref() == Some("DslSource"))
            .expect("outer DslSource node entry exists");
        assert_eq!(outer_source_entry.error, Some(err.error.to_string()));
    }

    #[test]
    fn traced_eval_root_types() {
        let limits = EvaluationLimits::default();
        let context = EvaluationContext::default();
        let mut rng = DeterministicRng::new(vec![]);

        let bool_expr = parse("1 == 1");
        let res_bool = evaluate_expression_traced(&bool_expr, &context, &mut rng, limits).unwrap();
        assert_eq!(res_bool.value, EvaluationValue::Bool(true));

        let int_expr = parse_int_expression("42").unwrap();
        let res_int = evaluate_expression_traced(&int_expr, &context, &mut rng, limits).unwrap();
        assert_eq!(res_int.value, EvaluationValue::Int(42));

        let text_expr = crate::compile_text_expression_with_macros("'hello'", &[]).unwrap();
        let res_text = evaluate_expression_traced(&text_expr, &context, &mut rng, limits).unwrap();
        assert_eq!(res_text.value, EvaluationValue::Text("hello".into()));
    }

    #[test]
    fn traced_eval_node_bound_exceeded_returns_empty_trace() {
        let expr = parse("1 == 1 && 1 == 1 && 1 == 1");
        let limits = EvaluationLimits {
            max_nodes: 2,
            max_depth: 10,
            max_draws: 10,
        };
        let context = EvaluationContext::default();
        let mut rng = DeterministicRng::new(vec![]);

        let err = evaluate_expression_traced(&expr, &context, &mut rng, limits)
            .expect_err("should exceed node bound");
        assert!(matches!(
            err.error,
            EvaluationError::NodeLimitExceeded { .. }
        ));
        assert!(
            err.trace.is_empty(),
            "trace must be empty when rejected before evaluation"
        );
    }

    #[test]
    fn traced_eval_advantage_reroll_shapes() {
        // 1. Roll-under under Advantage:
        let ru_expr = roll_under_expr(20, 15);
        let ru_context = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                roll_modifier: Some(RollModifier::Advantage),
                flat_bonus: 0,
            }),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::from_recorded_values(vec![18, 5]);
        let ru_res = evaluate_expression_traced(
            &ru_expr,
            &ru_context,
            &mut rng,
            EvaluationLimits::default(),
        )
        .unwrap();
        assert_eq!(ru_res.value, EvaluationValue::Bool(true));
        assert_eq!(ru_res.stochastic_draws.len(), 2);
        let roll_operand_entries: Vec<_> = ru_res.trace.iter().filter(|e| e.node_id == 1).collect();
        assert_eq!(roll_operand_entries.len(), 2);
        assert_eq!(
            roll_operand_entries[0].value,
            Some(EvaluationValue::Int(18))
        );
        assert_eq!(roll_operand_entries[1].value, Some(EvaluationValue::Int(5)));

        // 2. Single-die roll under Advantage:
        let roll_expr = parse("1d20 >= 10");
        let roll_context = EvaluationContext {
            roll_modifier_bias: Some(ComposedRollBias {
                roll_modifier: Some(RollModifier::Advantage),
                flat_bonus: 0,
            }),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::from_recorded_values(vec![8, 14]);
        let roll_res = evaluate_expression_traced(
            &roll_expr,
            &roll_context,
            &mut rng,
            EvaluationLimits::default(),
        )
        .unwrap();
        assert_eq!(roll_res.value, EvaluationValue::Bool(true));
        assert_eq!(roll_res.stochastic_draws.len(), 2);
        let roll_entries: Vec<_> = roll_res
            .trace
            .iter()
            .filter(|e| e.operator.as_deref() == Some("Roll"))
            .collect();
        assert_eq!(roll_entries.len(), 1);
        assert_eq!(roll_entries[0].value, Some(EvaluationValue::Int(14)));

        // 3. RollTable under Advantage:
        let rt_expr = parse_int_expression("roll_table('core/d20')").unwrap();
        let rt_context = EvaluationContext {
            roll_table_entries: d20_entries(),
            roll_modifier_bias: Some(ComposedRollBias {
                roll_modifier: Some(RollModifier::Advantage),
                flat_bonus: 0,
            }),
            ..EvaluationContext::default()
        };
        let mut rng = DeterministicRng::from_recorded_values(vec![0, 1]);
        let rt_res = evaluate_expression_traced(
            &rt_expr,
            &rt_context,
            &mut rng,
            EvaluationLimits::default(),
        )
        .unwrap();
        assert_eq!(rt_res.value, EvaluationValue::Int(2));
        assert_eq!(rt_res.stochastic_draws.len(), 2);
        let rt_entries: Vec<_> = rt_res
            .trace
            .iter()
            .filter(|e| e.operator.as_deref() == Some("RollTable"))
            .collect();
        assert_eq!(rt_entries.len(), 1);
        let table_selections = rt_entries[0]
            .table_entries
            .as_ref()
            .expect("must have table_entries");
        assert_eq!(table_selections.len(), 2);
        assert_eq!(table_selections[0].entry_id, "core/d20:1");
        assert!(!table_selections[0].kept);
        assert_eq!(table_selections[1].entry_id, "core/d20:2");
        assert!(table_selections[1].kept);

        // 4. RollTable without modifier:
        let mut rng = DeterministicRng::from_recorded_values(vec![0]);
        let rt_no_mod_context = EvaluationContext {
            roll_table_entries: d20_entries(),
            ..EvaluationContext::default()
        };
        let rt_no_mod_res = evaluate_expression_traced(
            &rt_expr,
            &rt_no_mod_context,
            &mut rng,
            EvaluationLimits::default(),
        )
        .unwrap();
        assert_eq!(rt_no_mod_res.value, EvaluationValue::Int(1));
        assert_eq!(rt_no_mod_res.stochastic_draws.len(), 1);
        let rt_no_mod_entries: Vec<_> = rt_no_mod_res
            .trace
            .iter()
            .filter(|e| e.operator.as_deref() == Some("RollTable"))
            .collect();
        assert_eq!(rt_no_mod_entries.len(), 1);
        let table_selections = rt_no_mod_entries[0]
            .table_entries
            .as_ref()
            .expect("must have table_entries");
        assert_eq!(table_selections.len(), 1);
        assert_eq!(table_selections[0].entry_id, "core/d20:1");
        assert!(table_selections[0].kept);
    }

    #[test]
    fn traced_eval_trace_entry_json_snapshot() {
        let entry = TraceEntry {
            node_id: 42,
            operator: Some("Compare".to_string()),
            depth: 2,
            span: Some(crate::SourceSpan { start: 0, end: 10 }),
            scope: vec![TraceScopeSegment {
                subject: SubjectRef::Actor,
                slot: "srd/damage".to_string(),
            }],
            value: Some(EvaluationValue::Bool(true)),
            error: None,
            table_entries: Some(vec![RollTableSelection {
                entry_id: "core/table:1".to_string(),
                kept: true,
            }]),
        };

        let json = serde_json::to_value(&entry).expect("must serialize");
        let map = json.as_object().expect("must be object");
        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();

        assert_eq!(
            keys,
            vec![
                "depth",
                "error",
                "node_id",
                "operator",
                "scope",
                "span",
                "table_entries",
                "value"
            ]
        );
    }
}
