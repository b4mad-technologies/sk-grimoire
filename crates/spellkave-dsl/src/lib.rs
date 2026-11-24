pub mod eval;
pub mod rng;
pub mod spatial;

pub use eval::{
    count_static_draws, evaluate_expression_traced, evaluate_int_expression,
    evaluate_offer_predicate_expression, evaluate_predicate_expression,
    evaluate_predicate_expression_with_report, validate_int_roll_table_usage, BindingFact,
    ChannelPoolFact, ComposedRollBias, EvaluationContext, EvaluationError, EvaluationLimits,
    EvaluationValue, ImpliedByCountFact, ItemDslSourceFact, OccupantsAtFact,
    ParticipantTagGroupCountFact, PeersNearTargetFact, PredicateEvaluation, RollModifier,
    RollTableEntryFact, RollTableIntValidationError, RollTableSelection, StochasticDraw,
    SubjectBoolFact, SubjectIdFact, SubjectIntFact, SubjectResourceBoundFact, SubjectTextFact,
    TraceEntry, TraceScopeSegment, TracedEvaluation, TracedEvaluationError, MAX_DRAW_CEILING,
};
pub use rng::{DeterministicRng, EvaluationRng};
pub use spatial::{
    validate_h3_cell_value, validate_non_empty_id, validate_place_node_id,
    validate_spatial_reference_for_insert, H3CellRef, LocationDefinitionRef, PlaceEdgeRef,
    PlaceNodeRef, SpatialReference, SpatialReferenceValidationError, TileRef,
};

pub const WORLD_TICK_FIELD_ID: &str = "core/tick";

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub type NodeId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpressionKind {
    Predicate,
    Outcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueType {
    Bool,
    Int,
    Text,
    RowRef,
    Outcome,
}

/// Authoring-facing name for the existing closed DSL value-type family.
///
/// The server's SpacetimeDB mirror uses this name as well.  It is an alias,
/// rather than another enum, so macro definitions cannot create a parallel
/// type vocabulary or a runtime dispatch surface.
pub type DslValueType = ValueType;

/// One ordered, typed input to an authoring macro.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslMacroParameter {
    pub name: String,
    pub value_type: DslValueType,
}

/// A typed source-level macro.  It is compiler input only: expansion produces
/// an ordinary [`Expression`] made exclusively of existing [`Op`] variants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslMacroDefinition {
    pub macro_id: String,
    pub return_type: DslValueType,
    pub parameters: Vec<DslMacroParameter>,
    pub body_source: String,
}

/// Failure from the macro-aware compiler.  Diagnostics retain the authored
/// source span; callers may attach manifest-row provenance around them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroCompileFailure {
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub root_node_id: NodeId,
    pub nodes: Vec<Node>,
}

impl Expression {
    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        let mut nodes = self.nodes.clone();
        nodes.sort_by_key(|node| node.node_id);
        for node in &mut nodes {
            node.span = SourceSpan::default();
        }
        let canonical = Self {
            kind: self.kind,
            root_node_id: self.root_node_id,
            nodes,
        };
        serde_json::to_string(&canonical)
    }

    pub fn node(&self, node_id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }

    /// The next free node id (max existing id + 1, or 1 if empty).
    ///
    /// Used by [`Expression::graft`] to renumber a foreign sub-expression
    /// onto a disjoint id range when merging it into `self`.
    pub fn next_free_node_id(&self) -> NodeId {
        self.nodes.iter().map(|n| n.node_id).max().unwrap_or(0) + 1
    }

    /// Copy every node of `other` into `self`, shifting each node id (and
    /// every child reference inside each [`Op`]) by `self.next_free_node_id()`.
    /// Returns the new root id of the grafted sub-expression — suitable for
    /// use as a hook node id pointing into the merged expression.
    ///
    /// The merged expression remains structurally normalized: ids are unique,
    /// every child reference resolves, and the original `root_node_id` /
    /// `kind` of `self` are preserved. The grafted subtree's own root is *not*
    /// promoted to `self.root_node_id`; callers (manifest hook authoring)
    /// store the returned id separately on the hook row instead.
    pub fn graft(&mut self, other: &Expression) -> NodeId {
        let offset = self.next_free_node_id();
        for node in &other.nodes {
            self.nodes.push(Node {
                node_id: node.node_id + offset,
                value_type: node.value_type,
                op: node.op.offset_node_ids(offset),
                span: node.span,
            });
        }
        other.root_node_id + offset
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub node_id: NodeId,
    pub value_type: ValueType,
    pub op: Op,
    /// Byte range in the source this node was parsed from. Empty for
    /// hand-built trees. Filled by [`parse_predicate`], [`parse_outcome`],
    /// [`parse_int_expression`], and the compile_* family.
    /// Omitted from [`Expression::canonical_json`] so AST identity stays
    /// source-span-independent.
    #[serde(default, skip_serializing_if = "SourceSpan::is_empty")]
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingRef {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventRef {
    Kind,
    Detail { field_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubjectRef {
    Actor,
    /// The item the actor currently wields — a **relation-named** subject
    /// reached via the chained surface `actor.wielded_item.*` (ADR-0021 D-2: the
    /// kernel reads a relation to an item entity and its generic `object_state`,
    /// never a "weapon"). §17 K-5: growing this closed enum is a K-rule event.
    /// Its license is the equality-filtered fact-pool design — the dispatch lives
    /// in the PARSER (`relation_resubject`), eval stays equality-filtered (the
    /// `shares_tag` precedent), so this variant needs no new eval arm; the wielded
    /// item's facts are populated upstream under this subject (caller's job,
    /// ADR-0035 missing-fact when nothing is wielded).
    ActorWieldedItem,
    Target,
    Location,
    Session,
    World,
    Binding(BindingRef),
    Event(EventRef),
    /// Exact authored actor equipment relation (ADR-0126). Appended to retain
    /// legacy variant ordering; callers populate facts for required slots only.
    ActorEquippedItem {
        slot_id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArithmeticOp {
    Add,
    Sub,
    Mul,
    Div,
    Max,
    Min,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadField {
    pub subject: SubjectRef,
    pub field_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HasCapability {
    pub subject: SubjectRef,
    pub capability_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HasItem {
    pub subject: SubjectRef,
    pub item_def_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HasNiche {
    pub subject: SubjectRef,
    pub niche_id: String,
}

/// `subject.has_tag('tag')` — true if the subject carries the (opaque, authored)
/// tag. The read leg of relational tagging (CR-7 / ADR-0038 D-3): the caller
/// populates the `tags` fact pool by unioning the subject entity's instance
/// `entity_tag` rows with its definition `entity_definition_tag` rows, so a tag
/// authored on either source resolves true. The eval arm is content-neutral —
/// the row-kind dispatch lives at fact population, never here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HasTag {
    pub subject: SubjectRef,
    pub tag: String,
}

/// `shares_tag(subjectA, subjectB, 'prefix')` — true iff `subjectA` and
/// `subjectB` each carry ≥1 tag whose id starts with the authored `prefix`,
/// and those two prefix-filtered tag sets intersect. Set-intersection over the
/// two subjects' already-populated tag pools (CR-7 / ADR-0038 D-3), filtered by
/// an authored string prefix. Content-neutral: the kernel never learns what a
/// prefix family *means* (`actor.language.` is the author's word, not the
/// substrate's) — same neutrality as `HasTag`. Reads only the two named
/// subjects' own tag pools (no world scan), so the locality verdict is `Place`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharesTag {
    pub subject_a: SubjectRef,
    pub subject_b: SubjectRef,
    pub prefix: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HasStatus {
    pub subject: SubjectRef,
    pub condition_definition_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpliedByCount {
    /// `None` means "the candidate child currently bound by the caller".
    /// That keeps one reusable remove policy row usable across all cascades.
    pub condition_definition_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParticipantTagGroupCount {
    pub prefix: String,
    pub excluding_condition_definition_id: Option<String>,
}

/// `peers_near_target(prefix, radius_mm, excluding_condition)` — K-A Fact 2
///, K-5 leaf). Counts actors OTHER than
/// Actor and Target within `radius_mm` of Target that share >=1
/// `prefix`-prefixed tag with Actor and do not carry the excluding
/// condition. All three arguments are authored; the kernel compares ids and
/// integers only and learns no vocabulary (ADR-0021 D-2, same discipline as
/// `SharesTag.prefix` and `ParticipantTagGroupCount`'s two args). Resolved
/// from a kernel-pre-populated fact vector scoped by a typed AST pre-scan
/// walker (`collect_peers_near_target_refs`, server side) — the evaluator
/// never touches the DB or computes geometry itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeersNearTarget {
    pub prefix: String,
    pub radius_mm: i64,
    pub excluding_condition_definition_id: Option<String>,
}

/// `occupants_at('<scope>', '<excluding_condition>')` — ADR-0134 D-3 (K-5
/// leaf). Counts actors OTHER than Actor that hold an `actor_spatial_location`
/// row on the scoped place node and do not carry the excluding condition.
/// `scope` names the node without the `@` sigil the ADR-0101 D-4 fact
/// projections use: `to_node` / `from_node` on the traversal path (`r_move`,
/// the edge's incident nodes) and `target` on the manipulate / resolution
/// path (Target's own `place_node`). The excluding condition is optional
/// (`Option<String>`, mirroring `PeersNearTarget`); omitted, every occupant
/// counts. Both arguments are authored strings the kernel only compares
/// (ADR-0021 D-2). Resolved from a kernel-pre-populated fact vector keyed by
/// the exact `(scope, excluding_condition)` pair (`collect_occupants_at_refs`,
/// server side) — the evaluator never touches the DB, and a scope the
/// current path did not populate is an evaluation error, never a silent zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OccupantsAt {
    pub scope: String,
    pub excluding_condition_definition_id: Option<String>,
}

/// `channel_pool('<channel_id>')` — reads the authored `pool_ms_per_round`
/// constant of an `economy_channel_definitions` row (sk-ct8.4 / finding #86).
/// `channel_id` is an open authored string (ADR-0018), never a closed enum —
/// only the leaf OP is kernel-coupled, not the id it names. Resolved from a
/// kernel-pre-populated fact vector, same discipline as `ImpliedByCount`/
/// `ParticipantTagGroupCount`; the evaluator never reads a database table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelPool {
    pub channel_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracePresent {
    pub subject: SubjectRef,
    pub trace_kind_id: String,
}

/// `actor.knows_entity($bindings.required_entity_id)` — true if the observer has
/// a knowledge row for the given entity id. The `entity_id` field holds a
/// literal entity id (as decimal string) or a binding path. (Item 16)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowsEntity {
    pub subject: SubjectRef,
    /// Decimal string of the entity_id, or a binding path like `$bindings.required_entity_id`.
    pub entity_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operands {
    pub operands: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnaryNode {
    pub operand: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compare {
    pub op: CompareOp,
    pub left: NodeId,
    pub right: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arithmetic {
    pub op: ArithmeticOp,
    pub left: NodeId,
    pub right: NodeId,
}

/// Typed ternary: evaluate `cond` (Bool), then evaluate exactly one of
/// `then_node` / `else_node` (both Int) — the untaken branch is never visited
/// (ADR-0045 D-3). Content-neutral: it selects an Int, knowing nothing of saves
/// or damage. The lazy branch is what no `And`/`Or`/`Let` composition
/// reproduces without re-evaluating a bound roll, which is its K-5 license.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cond {
    pub cond: NodeId,
    pub then_node: NodeId,
    pub else_node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dice {
    pub count: u32,
    pub sides: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoolLiteral {
    pub value: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntLiteral {
    pub value: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextLiteral {
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roll {
    pub dice: Dice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollUnder {
    pub roll_node: NodeId,
    pub target_node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollTable {
    /// The roll-table id as a **text-valued node** (Option C, G3 / #1254): a
    /// `TextLiteral` for a literal `roll_table('lit')` — the static-validatable
    /// case — or a `ReadText` for a runtime-resolved id
    /// (`roll_table(actor.wielded_item.text('…'))`). A `NodeId` argument matches
    /// the AST's existing operator-argument norm (`draw_node`, `delta_node`,
    /// `roll_node`); it grows no closed enum, so this is a grammar shape change,
    /// not a §17 K-5 event.
    pub table_id_node: NodeId,
    pub draw_node: NodeId,
}

/// sk-84i.2 (widened G-1, canon-warden PR #2155): evaluate `subject`'s named
/// DSL-source slot inline, in the caller's own evaluation context
/// (`actor`/`target` unchanged — the item expression sees the SAME facts as
/// the expression that invoked it; the kernel never re-derives a subject for
/// the nested evaluation). `subject` is a normal [`SubjectRef`] — the same
/// closed vocabulary every other fact-reading op (`ReadField`, `HasTag`, …)
/// already parameterises over, reached through the existing chained relation
/// accessor (`actor.wielded_item.source(...)` re-roots via
/// `relation_resubject`, exactly like `actor.wielded_item.stat(...)` does for
/// `ReadInt`). The arm is NOT hardcoded to "wielded" — a natural attack reads
/// `actor.source('slot')` off the actor's own definition, a worn-armor check
/// reads `target.source('slot')`, and so on; the fact pool that backs it
/// (`ItemDslSourceFact`) is keyed by `(subject, slot)` for the same reason.
///
/// `slot_node` reuses the same **text-valued node** shape `RollTable.table_id_node`
/// established (Option C, G3 / #1254): a `TextLiteral` for a literal
/// `actor.wielded_item.source('srd/damage')`, or a `ReadText` for a
/// runtime-resolved slot id. K-5 justification (§17.3): no existing grammar
/// slot resolves a dice/arithmetic expression sourced from a ROW at eval
/// time — `try_parse_dice` only ever parses a literal `XdN` token embedded
/// directly in the CALLING expression's own source text, and the current
/// alternative (`roll_table(actor.wielded_item.text('srd/damage_profile'))`,
/// G3/#1254) resolves only a static `result_ref` per row — it can express a
/// fixed dice distribution as enumerated weighted rows, but a roll-table
/// entry can never COMPOSE with the caller's own facts (an authored
/// `2d8 + actor.stat('core/str_mod')` on the item is unauthorable at any row
/// count; see `wielded_source_reads_actor_stat_from_the_callers_shared_fact_context`
/// in `eval.rs`, the falsifying fixture). So a new evaluated node kind is the
/// only way to splice a data-driven sub-expression into the caller's tree.
/// The item supplies source text, parsed when the expression is evaluated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DslSource {
    pub subject: SubjectRef,
    pub slot_node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Let {
    pub name: String,
    pub value: NodeId,
    pub body: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeSequence {
    pub steps: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvokeOutcomeGraph {
    pub outcome_graph_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifyResource {
    pub subject: SubjectRef,
    pub resource_definition_id: String,
    pub delta_node: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyStatus {
    pub subject: SubjectRef,
    pub condition_definition_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmitTrace {
    pub subject: SubjectRef,
    pub trace_kind_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arbitrate {
    pub request_shape_id: String,
    pub context_nodes: Vec<NodeId>,
    pub allowed_outcome_nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DslReference {
    pub operators: Vec<DslOperatorReference>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DslOperatorReference {
    pub operator: &'static str,
    pub expression_kind: &'static str,
    pub returns: ValueType,
    pub args: Vec<DslArgReference>,
    pub notes: Vec<&'static str>,
    pub examples: Vec<&'static str>,
    /// How the operator is authored: `"expression"` — parseable predicate/outcome
    /// DSL text (the example is real, executable DSL, gated by the
    /// `reference_examples` test, ADR-0047 D-5); `"frame_write"` — an AST node
    /// with no parser surface whose live authoring form is a structured
    /// consequence row (e.g. `[[consequence_frame_writes]] frame_write = { ModifyResource = {…} }`),
    /// NOT a DSL string — its example is the row's `frame_write` TOML value, not DSL.
    pub authoring: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DslArgReference {
    pub name: &'static str,
    pub shape: &'static str,
}

fn dsl_arg(name: &'static str, shape: &'static str) -> DslArgReference {
    DslArgReference { name, shape }
}

fn dsl_operator(
    operator: &'static str,
    expression_kind: &'static str,
    returns: ValueType,
    args: Vec<DslArgReference>,
    notes: Vec<&'static str>,
    examples: Vec<&'static str>,
) -> DslOperatorReference {
    DslOperatorReference {
        operator,
        expression_kind,
        returns,
        args,
        notes,
        examples,
        authoring: "expression",
    }
}

/// A consequence AST node with no parser surface. Its `examples` are the
/// `frame_write` TOML value of the live `[[consequence_frame_writes]]` row
/// (excluded from the parseable-DSL check in `tests/reference_examples.rs`).
fn frame_write_operator(
    operator: &'static str,
    expression_kind: &'static str,
    returns: ValueType,
    args: Vec<DslArgReference>,
    notes: Vec<&'static str>,
    examples: Vec<&'static str>,
) -> DslOperatorReference {
    DslOperatorReference {
        authoring: "frame_write",
        ..dsl_operator(operator, expression_kind, returns, args, notes, examples)
    }
}

pub fn dsl_reference() -> DslReference {
    DslReference {
        operators: vec![
            dsl_operator(
                "BoolLiteral",
                "predicate|outcome",
                ValueType::Bool,
                vec![dsl_arg("value", "boolean literal")],
                vec![
                    "Literal true/false value.",
                    "The bare literal parses only as a root int/outcome expression (`parse_int_expression`); as a Predicate operand it composes into a comparison, e.g. `true == true`.",
                ],
                vec!["true"],
            ),
            dsl_operator(
                "IntLiteral",
                "predicate|outcome",
                ValueType::Int,
                vec![dsl_arg("value", "integer literal")],
                vec!["Literal signed integer value."],
                vec!["10"],
            ),
            dsl_operator(
                "TextLiteral",
                "predicate|outcome",
                ValueType::Text,
                vec![dsl_arg("value", "single-quoted text literal")],
                vec!["Literal text value."],
                vec!["'opened'"],
            ),
            dsl_operator(
                "ReadInt",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|session|world|`bindings.<name>` (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`); `$event.detail.<int field>`",
                    ),
                    dsl_arg("field_id", "authored or core field id"),
                ],
                vec![
                    "Reads an integer fact supplied by the caller: `<subject>.stat('id')`, `bindings.<name>.<field>`, a bare binding name, or an int-typed `$event.detail.<field>`.",
                    "`location` is not a number-read subject — `location.stat(...)` does not parse.",
                ],
                vec!["actor.stat('core/strength')"],
            ),
            dsl_operator(
                "ReadEffectiveInt",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|session|world|`bindings.<name>` (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("field_id", "authored or core field id"),
                ],
                vec![
                    "Reads the EFFECTIVE integer value — base fact plus active modifier composition (e.g. condition-driven modifiers). `stat` reads the base value; `effective` opts in to the composed value.",
                ],
                vec!["actor.effective('homebrew/Momentum')"],
            ),
            dsl_operator(
                "ReadText",
                "predicate|outcome",
                ValueType::Text,
                vec![
                    dsl_arg(
                        "subject",
                        "`<subject>.substrate_id` (actor|target|session|world|`bindings.<name>`); `$event.kind` / `$event.detail.<text field>`; `<subject>.text('id')` (actor|target|location|session|world) only as a `roll_table(...)` or `.source(...)` argument",
                    ),
                    dsl_arg("field_id", "authored or core field id"),
                ],
                vec![
                    "Reads a text fact supplied by the caller. There is no general text read: only the three forms in `subject` produce a ReadText node.",
                    "`.text('id')` is not a free-standing expression — it parses only inside the id argument of `roll_table(...)` or `<subject>.source(...)`.",
                ],
                vec!["$event.detail.speech_act"],
            ),
            dsl_operator(
                "ReadBool",
                "predicate|outcome",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("field_id", "authored or core field id"),
                ],
                vec![
                    "Reads a boolean fact supplied by the caller via `<subject>.state('id')`.",
                    "Inside a macro body, a Bool-typed macro parameter name also reads as a ReadBool.",
                ],
                vec!["target.state('is_open')"],
            ),
            dsl_operator(
                "ReadResourceMax",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|session|world|`bindings.<name>` (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("field_id", "resource_definition_id"),
                ],
                vec![
                    "Reads the EFFECTIVE per-entity MAX bound for a resource — an `entity_resource_bound` override for the subject wins, else the bundle-wide `resource_definition.max_value` (#30 / sk-ct8.2).",
                ],
                vec!["actor.resource_max('srd/hp')"],
            ),
            dsl_operator(
                "ReadResourceMin",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|session|world|`bindings.<name>` (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("field_id", "resource_definition_id"),
                ],
                vec![
                    "Reads the EFFECTIVE per-entity MIN bound for a resource. Sibling of `ReadResourceMax`.",
                ],
                vec!["actor.resource_min('srd/hp')"],
            ),
            dsl_operator(
                "HasCapability",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("capability_id", "capability definition id"),
                ],
                vec!["Checks capability membership in the subject fact pool."],
                vec!["actor.has_capability('core/open')"],
            ),
            dsl_operator(
                "HasItem",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("item_def_id", "item definition id"),
                ],
                vec!["Checks item membership in the subject fact pool."],
                vec!["actor.has_item('core/torch')"],
            ),
            dsl_operator(
                "HasNiche",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("niche_id", "niche definition id"),
                ],
                vec!["Checks niche membership in the subject fact pool."],
                vec!["target.has_niche('core/container')"],
            ),
            dsl_operator(
                "HasTag",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("tag", "opaque authored tag"),
                ],
                vec!["Checks relational instance and definition tags supplied by the caller."],
                vec!["target.has_tag('locked')"],
            ),
            dsl_operator(
                "SharesTag",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg("subject_a", "actor|target|location|session|world"),
                    dsl_arg("subject_b", "actor|target|location|session|world"),
                    dsl_arg("prefix", "opaque authored tag-id prefix"),
                ],
                vec![
                    "True iff the two subjects each carry a tag starting with the prefix and those prefix-filtered tag sets intersect.",
                    "Content-neutral set-intersection over the two subjects' tag pools — an open authored tag family, never a closed column.",
                ],
                vec!["shares_tag(actor, target, 'actor.language.')"],
            ),
            dsl_operator(
                "HasStatus",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("condition_definition_id", "condition definition id"),
                ],
                vec!["Checks active condition/status membership."],
                vec!["actor.status('core/poisoned') == true"],
            ),
            dsl_operator(
                "ImpliedByCount",
                "predicate|outcome",
                ValueType::Int,
                vec![dsl_arg(
                    "condition_definition_id",
                    "optional condition definition id; absent means current candidate",
                )],
                vec![
                    "Reads a kernel-pre-populated count of other active rows whose definitions imply the child condition.",
                    "The evaluator never reads database tables; empty context returns 0.",
                ],
                vec!["implied_by_count('core/condition/incapacitated') == 0"],
            ),
            dsl_operator(
                "ParticipantTagGroupCount",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg("prefix", "opaque authored tag-id prefix"),
                    dsl_arg(
                        "excluding_condition_definition_id",
                        "optional condition definition id",
                    ),
                ],
                vec![
                    "Reads a kernel-pre-populated count of distinct matching tag values among session participants.",
                    "A missing pre-populated fact is an evaluation error, never a silent zero.",
                ],
                vec!["participant_tag_group_count('side:', 'core/condition/unavailable') <= 1"],
            ),
            dsl_operator(
                "PeersNearTarget",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg("prefix", "opaque authored tag-id prefix"),
                    dsl_arg("radius_mm", "authored radius in millimeters"),
                    dsl_arg(
                        "excluding_condition_definition_id",
                        "optional condition definition id",
                    ),
                ],
                vec![
                    "Reads a kernel-pre-populated count of actors other than Actor and Target within radius_mm of Target that share >=1 prefix-prefixed tag with Actor and do not carry the excluding condition.",
                    "The evaluator never reads database tables or computes geometry; a missing pre-populated fact is an evaluation error, never a silent zero.",
                ],
                vec!["peers_near_target('srd.faction.', 1524, 'srd/condition/incapacitated') >= 1"],
            ),
            dsl_operator(
                "OccupantsAt",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg(
                        "scope",
                        "node scope: to_node or from_node on the traversal path, target on the manipulate / resolution path",
                    ),
                    dsl_arg(
                        "excluding_condition_definition_id",
                        "optional condition definition id",
                    ),
                ],
                vec![
                    "Reads a kernel-pre-populated count of actors other than Actor standing on the scoped place node that do not carry the excluding condition (ADR-0134 D-3).",
                    "The evaluator never reads database tables; a scope the current path did not populate is an evaluation error, never a silent zero.",
                ],
                vec!["occupants_at('to_node', 'srd/condition/dead') == 0"],
            ),
            dsl_operator(
                "ChannelPool",
                "predicate|outcome",
                ValueType::Int,
                vec![dsl_arg("channel_id", "economy_channel_definitions.id")],
                vec![
                    "Reads the authored pool_ms_per_round constant of an economy channel.",
                    "A missing kernel-pre-populated fact is an evaluation error, never a silent zero.",
                ],
                vec!["channel_pool('core/action_class/primary')"],
            ),
            dsl_operator(
                "TracePresent",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("trace_kind_id", "trace kind id"),
                ],
                vec!["Checks whether a trace kind is present for the subject."],
                vec!["target.trace('core/opened')"],
            ),
            dsl_operator(
                "KnowsEntity",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg(
                        "subject",
                        "actor|target|location|session|world (chains `actor.wielded_item.…` / `actor.equipped_item('<slot>').…`)",
                    ),
                    dsl_arg("entity_id", "quoted entity-id string or `$bindings.<name>`"),
                ],
                vec![
                    "The entity_id argument is the entity-id-typed binding surface: a single-quoted string or a `$bindings.<name>` path. A bare numeric id does not parse.",
                ],
                vec!["actor.knows_entity($bindings.required_entity_id)"],
            ),
            dsl_operator(
                "And",
                "predicate",
                ValueType::Bool,
                vec![dsl_arg("operands", "two or more bool node ids")],
                vec![
                    "Short-circuits to false when any operand is false.",
                    "Binds LOOSER than `||`: `a && b || c` parses as `a && (b || c)`. Boolean terms cannot be parenthesised (decision `sk-jay0.7.105.10`).",
                ],
                vec!["actor.has_capability('core/open') && target.has_tag('locked')"],
            ),
            dsl_operator(
                "Or",
                "predicate",
                ValueType::Bool,
                vec![dsl_arg("operands", "two or more bool node ids")],
                vec![
                    "Short-circuits to true when any operand is true.",
                    "Binds TIGHTER than `&&`: `a && b || c` parses as `a && (b || c)`. Boolean terms cannot be parenthesised (decision `sk-jay0.7.105.10`).",
                ],
                vec!["target.has_tag('door') || target.has_tag('chest')"],
            ),
            dsl_operator(
                "Not",
                "predicate",
                ValueType::Bool,
                vec![dsl_arg("operand", "bool node id")],
                vec!["Negates one boolean operand."],
                vec!["target.has_tag('sealed') == false"],
            ),
            dsl_operator(
                "Compare",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg("op", "Eq|Ne|Lt|Lte|Gt|Gte"),
                    dsl_arg("left", "bool|int|text node id"),
                    dsl_arg("right", "same value type as left"),
                ],
                vec!["Both operands must have the same comparable value type."],
                vec!["actor.stat('core/strength') >= 10"],
            ),
            dsl_operator(
                "Arithmetic",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg("op", "Add|Sub|Mul|Div|Max|Min"),
                    dsl_arg("left", "int node id"),
                    dsl_arg("right", "int node id"),
                ],
                vec!["Division by zero is rejected at evaluation time."],
                vec!["max(actor.stat('core/strength'), 2)"],
            ),
            dsl_operator(
                "Cond",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg("cond", "bool node id"),
                    dsl_arg("then_node", "int node id"),
                    dsl_arg("else_node", "int node id"),
                ],
                vec!["Evaluates the bool, then exactly one int branch (lazy)."],
                vec!["cond(d20 >= 13, 0, 0 - d6)"],
            ),
            dsl_operator(
                "Roll",
                "predicate|outcome",
                ValueType::Int,
                vec![dsl_arg("dice", "<count>d<sides>")],
                vec![
                    "Uses the caller-provided deterministic evaluation RNG.",
                    "Single-die rolls (count == 1) consult the acting entity's composed roll bias (conditions with modifier delta_kind Advantage/Disadvantage/Plus): Advantage draws the die twice and keeps the max, Disadvantage the min, and BOTH physical faces are recorded individually in evaluation provenance.",
                ],
                vec!["1d20"],
            ),
            dsl_operator(
                "RollUnder",
                "predicate",
                ValueType::Bool,
                vec![
                    dsl_arg("dice", "die token (`d20`, `1d20`, `2d6`) — not an arbitrary int expression"),
                    dsl_arg("target", "int expression"),
                ],
                vec![
                    "Compares a roll result against a target number. The first argument must be a literal die token; the parser synthesizes the Roll node from it.",
                    "Consults the composed roll bias; success is a LOWER roll here, so Advantage keeps the min of two draws (the favorable direction flips relative to Roll).",
                ],
                vec!["roll_under(1d20, actor.stat('core/dexterity'))"],
            ),
            dsl_operator(
                "RollTable",
                "outcome",
                ValueType::Int,
                vec![dsl_arg(
                    "table_id",
                    "quoted roll table id or `<subject>.text('state_id')`",
                )],
                vec![
                    "Takes exactly one argument; the parser synthesizes the internal draw node.",
                    "Draws from an authored roll table; only the int-drawing form is parseable — there is no Outcome-returning form.",
                    "Int-valued draws consult the composed roll bias (Advantage re-draws and keeps the max, Disadvantage the min).",
                ],
                vec!["roll_table('core/random_event')"],
            ),
            dsl_operator(
                "DslSource",
                "predicate|outcome",
                ValueType::Int,
                vec![
                    dsl_arg(
                        "subject",
                        "actor\\|target\\|location\\|session\\|world\\|binding\\|actor.wielded_item\\|actor.equipped_item('<slot>')",
                    ),
                    dsl_arg("slot", "item DSL-source slot id"),
                ],
                vec![
                    "Evaluates `subject`'s named DSL-source slot inline, against the SAME actor/target fact context as the calling expression (sk-84i.2, widened G-1).",
                    "Fails closed when the subject carries no matching item_dsl_source fact for the slot (ADR-0035).",
                    "actor.equipped_item('<slot>') selects exactly one existing item in the actor's authored equipment slot (ADR-0126); missing, dangling, or duplicate occupancy supplies no item facts, with no carried-item fallback.",
                    "The equipment-slot argument is a nonempty literal referencing an admitted equipment slot. This subject supports tags, integer/text state, and source reads; it does not add equipment effective-state, status, or resource-bound reads.",
                    "The slot's own source may not itself call `.source(...)` again — depth-limited to 1, fails closed on violation.",
                    "The inner expression's stochastic draws are charged against the SAME evaluation-wide draw budget as the caller (ADR-0059).",
                ],
                vec![
                    "actor.wielded_item.source('srd/damage')",
                    "actor.equipped_item('demo/main-hand').source('demo/damage')",
                ],
            ),
            dsl_operator(
                "Let",
                "predicate|outcome",
                ValueType::Bool,
                vec![
                    dsl_arg("name", "binding name"),
                    dsl_arg("value", "int expression"),
                    dsl_arg("body", "bool or int expression"),
                ],
                vec![
                    "Introduces a lexical binding for the body node.",
                    "Returns the type of its body: Bool for a predicate body (as in the example), Int for an int body. The bound value is always an int expression.",
                ],
                vec!["let difficulty = 10 ; 1d20 >= difficulty"],
            ),
            dsl_operator(
                "OutcomeSequence",
                "outcome",
                ValueType::Outcome,
                vec![dsl_arg(
                    "steps",
                    "one or more outcome expressions (`sequence`, `invoke_outcome_graph`, `arbitrate`, …)",
                )],
                vec![
                    "Outcome DSL text in `outcome_templates.outcome_expression_source`; the server runs the steps in authored order.",
                ],
                vec!["sequence(invoke_outcome_graph('core/open_container'), invoke_outcome_graph('core/emit_noise'))"],
            ),
            dsl_operator(
                "InvokeOutcomeGraph",
                "outcome",
                ValueType::Outcome,
                vec![dsl_arg("outcome_graph_id", "quoted outcome graph id")],
                vec![
                    "Outcome DSL text in `outcome_templates.outcome_expression_source`; delegates to an authored `[[outcome_graphs]]` graph.",
                ],
                vec!["invoke_outcome_graph('core/open_container')"],
            ),
            frame_write_operator(
                "ModifyResource",
                "outcome",
                ValueType::Outcome,
                vec![
                    dsl_arg("actor", "binding ref of the entity whose resource changes, e.g. `Actor`"),
                    dsl_arg("resource", "resource definition id"),
                    dsl_arg(
                        "delta_source",
                        "DSL int expression evaluated at apply time (`-3`, `0 - d6`, `cond(...)`)",
                    ),
                ],
                vec![
                    "The `Op::ModifyResource` AST node has no parser surface; `modify_resource(...)` in DSL text does not parse.",
                    "Author a `[[consequence_frame_writes]]` row whose `frame_write` is `{ ModifyResource = { actor, resource, delta_source } }`.",
                ],
                vec![
                    "{ ModifyResource = { actor = \"Actor\", resource = \"core/hp\", delta_source = \"-3\" } }",
                ],
            ),
            frame_write_operator(
                "ApplyStatus",
                "outcome",
                ValueType::Outcome,
                vec![
                    dsl_arg("target_kind", "entity\\|item\\|world_object"),
                    dsl_arg("target_ref", "binding ref of the target, e.g. `Actor` or `Target`"),
                    dsl_arg("condition_definition_id", "condition definition id"),
                    dsl_arg(
                        "expires_at_boundary",
                        "optional `{ boundary_id, owner_actor }`",
                    ),
                ],
                vec![
                    "The `Op::ApplyStatus` AST node has no parser surface; `apply_status(...)` in DSL text does not parse.",
                    "Author a `[[consequence_frame_writes]]` row whose `frame_write` is `{ ApplyStatus = { target_kind, target_ref, condition_definition_id } }`.",
                ],
                vec![
                    "{ ApplyStatus = { target_kind = \"entity\", target_ref = \"Target\", condition_definition_id = \"core/poisoned\" } }",
                ],
            ),
            frame_write_operator(
                "EmitTrace",
                "outcome",
                ValueType::Outcome,
                vec![
                    dsl_arg("kind", "trace kind id"),
                    dsl_arg("location_kind", "where the trace lands, e.g. `binding`"),
                    dsl_arg("location_ref", "binding ref of the location, e.g. `ActorSpatialRef`"),
                    dsl_arg("payload_json", "JSON payload string"),
                ],
                vec![
                    "The `Op::EmitTrace` AST node has no parser surface, and there is no `EmitTrace` frame write.",
                    "The trace write is the `Inscribe` frame write: a `[[consequence_frame_writes]]` row whose `frame_write` is `{ Inscribe = { kind, location_kind, location_ref, payload_json } }` (optional `expires_at_tick`, `expires_after_ticks`, `carry_dispatch_text`, `carry_addressee`, `carry_dispatch_options`, `carry_occurrence`, `carry_dispatch_context`).",
                    "`carry_dispatch_options` copies bounded `dispatch_options` labels as `options` and parallel optional `dispatch_option_refs` as `option_definition_refs` (null means no description). References are exact static authored ids, never eligibility-derived.",
                    "`carry_dispatch_context` copies the dispatch's `dispatch_context` (one JSON object sent as text, at most 8 KiB) onto the payload under `context`. The kernel checks only that it is an object within the bound and reads no key of it; it is presentation content for readers of the trace, and no prompt binding, response set or predicate depends on it. A speaker uses it for the machine side of a question (see [guided dialogue](patterns/guided-dialogue.md)).",
                    "`carry_occurrence` copies only the admitted occurrence id into answer history as a decimal string; unbound actions omit it. Response tags `ui.response-slot=<positive integer>` identify syntax without parsing action ids or labels. See [the carriage contracts](interaction-dsl-and-world-authoring.md#recording-an-answered-occurrence-carry_occurrence).",
                ],
                vec![
                    "{ Inscribe = { kind = \"core/opened\", location_kind = \"binding\", location_ref = \"ActorSpatialRef\", payload_json = \"{}\" } }",
                ],
            ),
            dsl_operator(
                "Arbitrate",
                "outcome",
                ValueType::Outcome,
                vec![
                    dsl_arg("request_shape_id", "quoted arbitration request shape id"),
                    dsl_arg("context", "one quoted `fact_id=value` string"),
                    dsl_arg(
                        "allowed_outcomes",
                        "one or more quoted `outcome_id=outcome_graph_id` strings",
                    ),
                ],
                vec![
                    "Outcome DSL text in `outcome_templates.outcome_expression_source`; opens an arbitration request with bounded allowed outcomes.",
                    "Every argument is a single-quoted string; a malformed `fact_id=value` or `outcome_id=outcome_graph_id` pair fails at execution time.",
                ],
                vec![
                    "arbitrate('core/dialogue_choice', 'mood=tense', 'agree=core/dialogue/agree', 'refuse=core/dialogue/refuse')",
                ],
            ),
        ],
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// `EnumCount` is derived only under `cfg(test)` so the variant count
// (`Op::COUNT`) anchors the DSL-reference drift test to the compiler: adding a
// variant changes `COUNT` and fails the test until the variant is both
// represented and described. No production/wasm footprint — strum is a
// dev-dependency.
#[cfg_attr(test, derive(strum::EnumCount))]
pub enum Op {
    BoolLiteral(BoolLiteral),
    IntLiteral(IntLiteral),
    TextLiteral(TextLiteral),
    ReadInt(ReadField),
    /// #1247: like `ReadInt` but reads the EFFECTIVE (composed) value —
    /// base `int_fields` plus active modifier composition folded into the
    /// `effective_int_fields` overlay. `ReadInt` stays strictly base (the
    /// ADR-0035 zirkularity cut); `ReadEffectiveInt` is the explicit author
    /// opt-in to read modifier-composed stats (`actor.effective("x")`).
    ReadEffectiveInt(ReadField),
    ReadText(ReadField),
    ReadBool(ReadField),
    /// #30 / sk-ct8.2: the EFFECTIVE per-entity MAX bound for a resource
    /// (`entity_resource_bound` override else the bundle-wide
    /// `resource_definition.max_value`) — `field_id` names the
    /// `resource_definition_id`. Mirror of `ReadInt`'s shape/precedent; a
    /// dedicated leaf op (not a `stat(field_id)` string convention) because
    /// the lookup is a PARAMETERIZED table-backed constant, not a plain
    /// per-subject fact.
    ReadResourceMax(ReadField),
    /// #30 / sk-ct8.2: the EFFECTIVE per-entity MIN bound. Sibling of
    /// `ReadResourceMax`.
    ReadResourceMin(ReadField),
    HasCapability(HasCapability),
    HasItem(HasItem),
    HasNiche(HasNiche),
    HasTag(HasTag),
    SharesTag(SharesTag),
    HasStatus(HasStatus),
    ImpliedByCount(ImpliedByCount),
    ParticipantTagGroupCount(ParticipantTagGroupCount),
    PeersNearTarget(PeersNearTarget),
    OccupantsAt(OccupantsAt),
    ChannelPool(ChannelPool),
    TracePresent(TracePresent),
    KnowsEntity(KnowsEntity),
    And(Operands),
    Or(Operands),
    Not(UnaryNode),
    Compare(Compare),
    Arithmetic(Arithmetic),
    Cond(Cond),
    Roll(Roll),
    RollUnder(RollUnder),
    RollTable(RollTable),
    DslSource(DslSource),
    Let(Let),
    OutcomeSequence(OutcomeSequence),
    InvokeOutcomeGraph(InvokeOutcomeGraph),
    ModifyResource(ModifyResource),
    ApplyStatus(ApplyStatus),
    EmitTrace(EmitTrace),
    Arbitrate(Arbitrate),
}

impl Op {
    /// Statically named equipment dependencies, including non-integer readers.
    /// This inventories subjects, not support for populating every fact family.
    pub fn equipped_item_slots(&self) -> impl Iterator<Item = &str> {
        let subjects = match self {
            Op::ReadInt(op)
            | Op::ReadEffectiveInt(op)
            | Op::ReadText(op)
            | Op::ReadBool(op)
            | Op::ReadResourceMax(op)
            | Op::ReadResourceMin(op) => [Some(&op.subject), None],
            Op::HasCapability(op) => [Some(&op.subject), None],
            Op::HasItem(op) => [Some(&op.subject), None],
            Op::HasNiche(op) => [Some(&op.subject), None],
            Op::HasTag(op) => [Some(&op.subject), None],
            Op::SharesTag(op) => [Some(&op.subject_a), Some(&op.subject_b)],
            Op::HasStatus(op) => [Some(&op.subject), None],
            Op::TracePresent(op) => [Some(&op.subject), None],
            Op::KnowsEntity(op) => [Some(&op.subject), None],
            Op::DslSource(op) => [Some(&op.subject), None],
            Op::ModifyResource(op) => [Some(&op.subject), None],
            Op::ApplyStatus(op) => [Some(&op.subject), None],
            Op::EmitTrace(op) => [Some(&op.subject), None],
            Op::BoolLiteral(_)
            | Op::IntLiteral(_)
            | Op::TextLiteral(_)
            | Op::ImpliedByCount(_)
            | Op::ParticipantTagGroupCount(_)
            | Op::PeersNearTarget(_)
            | Op::OccupantsAt(_)
            | Op::ChannelPool(_)
            | Op::And(_)
            | Op::Or(_)
            | Op::Not(_)
            | Op::Compare(_)
            | Op::Arithmetic(_)
            | Op::Cond(_)
            | Op::Roll(_)
            | Op::RollUnder(_)
            | Op::RollTable(_)
            | Op::Let(_)
            | Op::OutcomeSequence(_)
            | Op::InvokeOutcomeGraph(_)
            | Op::Arbitrate(_) => [None, None],
        };
        subjects
            .into_iter()
            .flatten()
            .filter_map(|subject| match subject {
                SubjectRef::ActorEquippedItem { slot_id } => Some(slot_id.as_str()),
                _ => None,
            })
    }

    pub fn variant_name(&self) -> &'static str {
        match self {
            Op::BoolLiteral(_) => "BoolLiteral",
            Op::IntLiteral(_) => "IntLiteral",
            Op::TextLiteral(_) => "TextLiteral",
            Op::ReadInt(_) => "ReadInt",
            Op::ReadEffectiveInt(_) => "ReadEffectiveInt",
            Op::ReadText(_) => "ReadText",
            Op::ReadBool(_) => "ReadBool",
            Op::ReadResourceMax(_) => "ReadResourceMax",
            Op::ReadResourceMin(_) => "ReadResourceMin",
            Op::HasCapability(_) => "HasCapability",
            Op::HasItem(_) => "HasItem",
            Op::HasNiche(_) => "HasNiche",
            Op::HasTag(_) => "HasTag",
            Op::SharesTag(_) => "SharesTag",
            Op::HasStatus(_) => "HasStatus",
            Op::ImpliedByCount(_) => "ImpliedByCount",
            Op::ParticipantTagGroupCount(_) => "ParticipantTagGroupCount",
            Op::PeersNearTarget(_) => "PeersNearTarget",
            Op::OccupantsAt(_) => "OccupantsAt",
            Op::ChannelPool(_) => "ChannelPool",
            Op::TracePresent(_) => "TracePresent",
            Op::KnowsEntity(_) => "KnowsEntity",
            Op::And(_) => "And",
            Op::Or(_) => "Or",
            Op::Not(_) => "Not",
            Op::Compare(_) => "Compare",
            Op::Arithmetic(_) => "Arithmetic",
            Op::Cond(_) => "Cond",
            Op::Roll(_) => "Roll",
            Op::RollUnder(_) => "RollUnder",
            Op::RollTable(_) => "RollTable",
            Op::DslSource(_) => "DslSource",
            Op::Let(_) => "Let",
            Op::OutcomeSequence(_) => "OutcomeSequence",
            Op::InvokeOutcomeGraph(_) => "InvokeOutcomeGraph",
            Op::ModifyResource(_) => "ModifyResource",
            Op::ApplyStatus(_) => "ApplyStatus",
            Op::EmitTrace(_) => "EmitTrace",
            Op::Arbitrate(_) => "Arbitrate",
        }
    }

    /// If this op consumes a binding as an ENTITY-ID-typed argument — a value
    /// that must resolve to a numeric `entity_id` (today only
    /// `KnowsEntity.entity_id`) — return the binding name it reads, with the
    /// `$bindings.` prefix stripped. Returns `None` for a literal id, for a
    /// non-binding argument, and for every op whose binding args are def-id /
    /// tag / capability / field strings compared as-is (`HasItem.item_def_id`,
    /// `HasTag.tag`, `ReadInt.field_id`, …) — those values are never numeric
    /// entity ids and must NOT be rewritten.
    ///
    /// Single source of truth for "which bindings are entity refs" (CR-18):
    /// manifest apply reads it to resolve authored entity KEYS → numeric ids
    /// before storage, and the offline validator reads it to reject a key that
    /// resolves to no authored entity (`MF-VAL-244`). One classifier here is
    /// what stops the apply path and the validator from drifting — the
    /// duplicate-grammar failure mode CR-17 fixed for the dice term. A future
    /// entity-id-typed op adds its arm here, and both consumers pick it up.
    pub fn entity_ref_binding(&self) -> Option<&str> {
        let field = match self {
            Op::KnowsEntity(op) => op.entity_id.as_str(),
            _ => return None,
        };
        field
            .strip_prefix("$bindings.")
            .filter(|name| !name.is_empty())
    }

    /// If this op reads a typed field from a binding subject, return the binding
    /// name. This is intentionally separate from `entity_ref_binding`: the field
    /// id stays a normal authored field string, while the subject must be covered
    /// by a row-level binding map.
    pub fn binding_subject_name(&self) -> Option<&str> {
        let subject = match self {
            Op::ReadBool(read)
            | Op::ReadInt(read)
            | Op::ReadEffectiveInt(read)
            | Op::ReadText(read)
            | Op::ReadResourceMax(read)
            | Op::ReadResourceMin(read) => &read.subject,
            _ => return None,
        };
        match subject {
            SubjectRef::Binding(binding) => Some(binding.name.as_str()),
            _ => None,
        }
    }

    pub fn child_nodes(&self) -> Vec<NodeId> {
        match self {
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
            | Op::PeersNearTarget(_)
            | Op::OccupantsAt(_)
            | Op::ChannelPool(_)
            | Op::TracePresent(_)
            | Op::KnowsEntity(_)
            | Op::Roll(_)
            | Op::InvokeOutcomeGraph(_)
            | Op::ApplyStatus(_)
            | Op::EmitTrace(_) => Vec::new(),
            Op::And(op) | Op::Or(op) => op.operands.clone(),
            Op::Not(op) => vec![op.operand],
            Op::Compare(op) => vec![op.left, op.right],
            Op::Arithmetic(op) => vec![op.left, op.right],
            Op::Cond(op) => vec![op.cond, op.then_node, op.else_node],
            Op::RollUnder(op) => vec![op.roll_node, op.target_node],
            Op::RollTable(op) => vec![op.table_id_node, op.draw_node],
            Op::DslSource(op) => vec![op.slot_node],
            Op::Let(op) => vec![op.value, op.body],
            Op::OutcomeSequence(op) => op.steps.clone(),
            Op::ModifyResource(op) => vec![op.delta_node],
            Op::Arbitrate(op) => op
                .context_nodes
                .iter()
                .chain(op.allowed_outcome_nodes.iter())
                .copied()
                .collect(),
        }
    }

    /// Return a copy of this op with every child [`NodeId`] reference shifted
    /// by `offset`. Leaf ops (literals, reads, capability checks, …) are
    /// returned unchanged. Used by [`Expression::graft`] to renumber a
    /// foreign sub-expression onto a disjoint id range.
    pub fn offset_node_ids(&self, offset: NodeId) -> Op {
        match self {
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
            | Op::PeersNearTarget(_)
            | Op::OccupantsAt(_)
            | Op::ChannelPool(_)
            | Op::TracePresent(_)
            | Op::KnowsEntity(_)
            | Op::Roll(_)
            | Op::InvokeOutcomeGraph(_)
            | Op::ApplyStatus(_)
            | Op::EmitTrace(_) => self.clone(),
            Op::And(op) => Op::And(Operands {
                operands: op.operands.iter().map(|n| n + offset).collect(),
            }),
            Op::Or(op) => Op::Or(Operands {
                operands: op.operands.iter().map(|n| n + offset).collect(),
            }),
            Op::Not(op) => Op::Not(UnaryNode {
                operand: op.operand + offset,
            }),
            Op::Compare(op) => Op::Compare(Compare {
                op: op.op,
                left: op.left + offset,
                right: op.right + offset,
            }),
            Op::Arithmetic(op) => Op::Arithmetic(Arithmetic {
                op: op.op,
                left: op.left + offset,
                right: op.right + offset,
            }),
            Op::Cond(op) => Op::Cond(Cond {
                cond: op.cond + offset,
                then_node: op.then_node + offset,
                else_node: op.else_node + offset,
            }),
            Op::RollUnder(op) => Op::RollUnder(RollUnder {
                roll_node: op.roll_node + offset,
                target_node: op.target_node + offset,
            }),
            Op::RollTable(op) => Op::RollTable(RollTable {
                table_id_node: op.table_id_node + offset,
                draw_node: op.draw_node + offset,
            }),
            Op::DslSource(op) => Op::DslSource(DslSource {
                subject: op.subject.clone(),
                slot_node: op.slot_node + offset,
            }),
            Op::Let(op) => Op::Let(Let {
                name: op.name.clone(),
                value: op.value + offset,
                body: op.body + offset,
            }),
            Op::OutcomeSequence(op) => Op::OutcomeSequence(OutcomeSequence {
                steps: op.steps.iter().map(|n| n + offset).collect(),
            }),
            Op::ModifyResource(op) => Op::ModifyResource(ModifyResource {
                subject: op.subject.clone(),
                resource_definition_id: op.resource_definition_id.clone(),
                delta_node: op.delta_node + offset,
            }),
            Op::Arbitrate(op) => Op::Arbitrate(Arbitrate {
                request_shape_id: op.request_shape_id.clone(),
                context_nodes: op.context_nodes.iter().map(|n| n + offset).collect(),
                allowed_outcome_nodes: op
                    .allowed_outcome_nodes
                    .iter()
                    .map(|n| n + offset)
                    .collect(),
            }),
        }
    }

    fn remap_node_ids(&self, remap: &impl Fn(NodeId) -> NodeId) -> Op {
        let mut copy = self.clone();
        match &mut copy {
            Op::And(op) | Op::Or(op) => op.operands.iter_mut().for_each(|id| *id = remap(*id)),
            Op::Not(op) => op.operand = remap(op.operand),
            Op::Compare(op) => {
                op.left = remap(op.left);
                op.right = remap(op.right);
            }
            Op::Arithmetic(op) => {
                op.left = remap(op.left);
                op.right = remap(op.right);
            }
            Op::Cond(op) => {
                op.cond = remap(op.cond);
                op.then_node = remap(op.then_node);
                op.else_node = remap(op.else_node);
            }
            Op::RollUnder(op) => {
                op.roll_node = remap(op.roll_node);
                op.target_node = remap(op.target_node);
            }
            Op::RollTable(op) => {
                op.table_id_node = remap(op.table_id_node);
                op.draw_node = remap(op.draw_node);
            }
            Op::DslSource(op) => op.slot_node = remap(op.slot_node),
            Op::Let(op) => {
                op.value = remap(op.value);
                op.body = remap(op.body);
            }
            Op::OutcomeSequence(op) => op.steps.iter_mut().for_each(|id| *id = remap(*id)),
            Op::ModifyResource(op) => op.delta_node = remap(op.delta_node),
            Op::Arbitrate(op) => {
                op.context_nodes.iter_mut().for_each(|id| *id = remap(*id));
                op.allowed_outcome_nodes
                    .iter_mut()
                    .for_each(|id| *id = remap(*id));
            }
            _ => {}
        }
        copy
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    KindMismatch {
        expected: ExpressionKind,
        actual: ExpressionKind,
    },
    RootMissing {
        root_node_id: NodeId,
    },
    DuplicateNodeId {
        node_id: NodeId,
    },
    MissingChild {
        parent_node_id: NodeId,
        child_node_id: NodeId,
    },
    ShadowedBinding {
        node_id: NodeId,
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventBindingValidationError {
    UnknownDetailField {
        node_id: NodeId,
        event_kind: String,
        field_id: String,
    },
}

pub fn validate_event_binding_refs_for_event(
    expr: &Expression,
    event_kind: &str,
    allowed_detail_fields: &[&str],
) -> Result<(), EventBindingValidationError> {
    for node in &expr.nodes {
        let read = match &node.op {
            Op::ReadBool(read)
            | Op::ReadInt(read)
            | Op::ReadEffectiveInt(read)
            | Op::ReadText(read) => read,
            _ => continue,
        };
        let SubjectRef::Event(EventRef::Detail { field_id }) = &read.subject else {
            continue;
        };
        if !allowed_detail_fields
            .iter()
            .any(|allowed| allowed == field_id)
        {
            return Err(EventBindingValidationError::UnknownDetailField {
                node_id: node.node_id,
                event_kind: event_kind.to_string(),
                field_id: field_id.clone(),
            });
        }
    }

    Ok(())
}

pub fn validate_expression_for_insert(
    expr: &Expression,
    expected_kind: ExpressionKind,
) -> Result<(), ValidationError> {
    if expr.kind != expected_kind {
        return Err(ValidationError::KindMismatch {
            expected: expected_kind,
            actual: expr.kind,
        });
    }

    let ids: BTreeSet<NodeId> = expr.nodes.iter().map(|n| n.node_id).collect();
    if !ids.contains(&expr.root_node_id) {
        return Err(ValidationError::RootMissing {
            root_node_id: expr.root_node_id,
        });
    }
    if ids.len() != expr.nodes.len() {
        let mut seen = BTreeSet::new();
        let duplicate = expr
            .nodes
            .iter()
            .find_map(|n| (!seen.insert(n.node_id)).then_some(n.node_id))
            .unwrap_or(expr.root_node_id);
        return Err(ValidationError::DuplicateNodeId { node_id: duplicate });
    }

    for node in &expr.nodes {
        for child in node.op.child_nodes() {
            if !ids.contains(&child) {
                return Err(ValidationError::MissingChild {
                    parent_node_id: node.node_id,
                    child_node_id: child,
                });
            }
        }
    }

    validate_no_shadowed_lets(
        expr,
        expr.root_node_id,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
    )?;

    Ok(())
}

fn validate_no_shadowed_lets(
    expr: &Expression,
    node_id: NodeId,
    bindings: &mut BTreeSet<String>,
    active_nodes: &mut BTreeSet<NodeId>,
) -> Result<(), ValidationError> {
    if !active_nodes.insert(node_id) {
        return Ok(());
    }
    let Some(node) = expr.node(node_id) else {
        active_nodes.remove(&node_id);
        return Ok(());
    };
    if let Op::Let(op) = &node.op {
        if bindings.contains(&op.name) {
            active_nodes.remove(&node_id);
            return Err(ValidationError::ShadowedBinding {
                node_id,
                name: op.name.clone(),
            });
        }
        validate_no_shadowed_lets(expr, op.value, bindings, active_nodes)?;
        bindings.insert(op.name.clone());
        let body_result = validate_no_shadowed_lets(expr, op.body, bindings, active_nodes);
        bindings.remove(&op.name);
        active_nodes.remove(&node_id);
        return body_result;
    }

    for child in node.op.child_nodes() {
        validate_no_shadowed_lets(expr, child, bindings, active_nodes)?;
    }
    active_nodes.remove(&node_id);
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub fn is_empty(&self) -> bool {
        self.start == 0 && self.end == 0
    }

    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseFailure {
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseFailure {
    fn single(code: &'static str, message: String, span: SourceSpan) -> Self {
        Self {
            diagnostics: vec![Diagnostic {
                code,
                message,
                span,
            }],
        }
    }
}

pub fn parse_predicate(source: &str) -> Result<Expression, ParseFailure> {
    parse_internal(source, ExpressionKind::Predicate)
}

pub fn parse_outcome(source: &str) -> Result<Expression, ParseFailure> {
    parse_internal(source, ExpressionKind::Outcome)
}

/// Parse `source` as a standalone integer expression — a dice roll (`d20`,
/// `2d6`), arithmetic, `min`/`max`, or `roll_table`. The result carries
/// `ExpressionKind::Outcome`, so it round-trips through
/// [`validate_expression_for_insert`] and [`evaluate_int_expression`] — the
/// same convention `modifier_template` uses for int-valued authored
/// expressions, but rooted directly at an int expression (no comparison
/// operator required).
///
/// Used for `turn_scheduler.order_roll_source`: ADR-0037 D-4 evaluates the
/// authored expression once per participant through the shared evaluator,
/// rather than a bespoke dice parser (CR-17 / LR-17).
pub fn parse_int_expression(source: &str) -> Result<Expression, ParseFailure> {
    let tokens = lex(source)?;
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        next_node_id: 1,
        nodes: Vec::new(),
        macro_definitions: None,
        parameter_types: None,
        macro_calls_allowed: true,
    };
    let root_node_id = parser.parse_int_expr()?;
    if !matches!(parser.peek().kind, TokenKind::Eof) {
        let token = parser.peek().clone();
        return Err(ParseFailure::single(
            "MF-DSL-TRAILING",
            "unexpected trailing input".to_string(),
            token.span,
        ));
    }
    Ok(expression_with_spans(
        ExpressionKind::Outcome,
        root_node_id,
        parser.nodes,
        source,
    ))
}

/// Compile `macro('id', ...)` authoring calls away before handing an ordinary
/// AST to the existing validator and evaluator. Macro bodies are deliberately
/// one layer only: a body containing call syntax is rejected.
#[allow(dead_code)]
fn legacy_compile_int_expression_with_macros(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<Expression, MacroCompileFailure> {
    let expanded = expand_macro_calls(source, definitions)?;
    parse_int_expression(&expanded).map_err(Into::into)
}

#[allow(dead_code)]
fn legacy_compile_predicate_with_macros(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<Expression, MacroCompileFailure> {
    let expanded = expand_macro_calls(source, definitions)?;
    parse_predicate(&expanded).map_err(Into::into)
}

#[allow(dead_code)]
fn legacy_compile_text_expression_with_macros(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<Expression, MacroCompileFailure> {
    let expanded = expand_macro_calls(source, definitions)?;
    // Text is represented by the existing literal/read AST.  Parsing it through
    // an Int expression is intentionally avoided: text macros are compiler
    // input for typed authoring surfaces, not a new evaluator entry point.
    let expression = parse_int_expression(&expanded).map_err(MacroCompileFailure::from)?;
    if expression
        .node(expression.root_node_id)
        .map(|node| node.value_type)
        != Some(ValueType::Text)
    {
        return Err(macro_failure(
            "MF-DSL-MACRO-RETURN-TYPE",
            "expected a Text macro expression",
        ));
    }
    Ok(expression)
}

fn macro_failure(code: &'static str, message: impl Into<String>) -> MacroCompileFailure {
    MacroCompileFailure {
        diagnostics: vec![Diagnostic {
            code,
            message: message.into(),
            span: SourceSpan { start: 0, end: 0 },
        }],
    }
}

fn expand_macro_calls(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<String, MacroCompileFailure> {
    let Some(start) = source.find("macro(") else {
        return Ok(source.to_string());
    };
    let open = start + "macro".len();
    let end = matching_paren(source, open)
        .ok_or_else(|| macro_failure("MF-DSL-EXPECTED-RPAREN", "unterminated macro call"))?;
    let before = &source[..start];
    let after = &source[end + 1..];
    let parts = split_macro_arguments(&source[open + 1..end]);
    let Some((id, arguments)) = parts.split_first() else {
        return Err(macro_failure("MF-DSL-MACRO-ID", "expected quoted macro id"));
    };
    let macro_id =
        unquote(id).ok_or_else(|| macro_failure("MF-DSL-MACRO-ID", "expected quoted macro id"))?;
    let definition = definitions
        .iter()
        .find(|definition| definition.macro_id == macro_id)
        .ok_or_else(|| {
            macro_failure(
                "MF-DSL-MACRO-UNKNOWN",
                format!("unknown macro `{macro_id}`"),
            )
        })?;
    if definition.body_source.contains("macro(") {
        return Err(macro_failure(
            "MF-DSL-MACRO-IN-BODY",
            format!("macro `{macro_id}` body invokes a macro"),
        ));
    }
    if arguments.len() != definition.parameters.len() {
        return Err(macro_failure(
            "MF-DSL-MACRO-ARITY",
            format!(
                "macro `{macro_id}` expects {} arguments, got {}",
                definition.parameters.len(),
                arguments.len()
            ),
        ));
    }
    let mut body = definition.body_source.clone();
    for (parameter, argument) in definition.parameters.iter().zip(arguments) {
        let parsed = match parameter.value_type {
            ValueType::Int => parse_int_expression(argument).map(|_| ()),
            ValueType::Bool => parse_predicate(argument).map(|_| ()),
            ValueType::Text => parse_int_expression(argument).and_then(|expr| {
                (expr.node(expr.root_node_id).map(|node| node.value_type) == Some(ValueType::Text))
                    .then_some(())
                    .ok_or_else(|| {
                        ParseFailure::single(
                            "MF-DSL-EXPECTED-TEXT",
                            "expected text expression".to_string(),
                            SourceSpan { start: 0, end: 0 },
                        )
                    })
            }),
            ValueType::RowRef | ValueType::Outcome => {
                return Err(macro_failure(
                    "MF-DSL-MACRO-UNSUPPORTED-TYPE",
                    "macro signatures support Bool, Int, and Text only",
                ))
            }
        };
        if parsed.is_err() {
            return Err(macro_failure(
                "MF-DSL-MACRO-ARG-TYPE",
                format!(
                    "macro `{macro_id}` argument for `{}` has the wrong type",
                    parameter.name
                ),
            ));
        }
        body = replace_identifier(&body, &parameter.name, &format!("({argument})"));
    }
    let expanded = format!("{before}{body}{after}");
    // v1 macro bodies cannot call macros, so each recursive step consumes a
    // call written by the author rather than generating an expansion graph.
    expand_macro_calls(&expanded, definitions)
}

fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    for (index, character) in source[open..].char_indices() {
        let index = open + index;
        if matches!(character, '\'' | '"') {
            quote = if quote == Some(character) {
                None
            } else if quote.is_none() {
                Some(character)
            } else {
                quote
            };
            continue;
        }
        if quote.is_some() {
            continue;
        }
        if character == '(' {
            depth += 1;
        }
        if character == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn split_macro_arguments(source: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote = None;
    for (index, character) in source.char_indices() {
        if matches!(character, '\'' | '"') {
            quote = if quote == Some(character) {
                None
            } else if quote.is_none() {
                Some(character)
            } else {
                quote
            };
            continue;
        }
        if quote.is_some() {
            continue;
        }
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                result.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    result.push(source[start..].trim());
    result
}

fn unquote(value: &str) -> Option<&str> {
    value
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .or_else(|| {
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
}

fn replace_identifier(source: &str, name: &str, replacement: &str) -> String {
    source
        .split_whitespace()
        .map(|word| if word == name { replacement } else { word })
        .collect::<Vec<_>>()
        .join(" ")
}

fn expression_with_spans(
    kind: ExpressionKind,
    root_node_id: NodeId,
    nodes: Vec<Node>,
    source: &str,
) -> Expression {
    let mut expr = Expression {
        kind,
        root_node_id,
        nodes,
    };
    attach_tree_spans(&mut expr, source.len());
    expr
}

fn attach_tree_spans(expr: &mut Expression, source_len: usize) {
    fn union_from(expr: &Expression, node_id: NodeId) -> SourceSpan {
        let Some(node) = expr.node(node_id) else {
            return SourceSpan::default();
        };
        let mut span = node.span;
        for child in node.op.child_nodes() {
            span = span.union(union_from(expr, child));
        }
        span
    }
    if expr.nodes.is_empty() {
        return;
    }
    let mut spans = std::collections::BTreeMap::new();
    for node in &expr.nodes {
        spans.insert(node.node_id, union_from(expr, node.node_id));
    }
    for node in &mut expr.nodes {
        if let Some(span) = spans.get(&node.node_id) {
            node.span = *span;
        }
    }
    if let Some(root) = expr
        .nodes
        .iter_mut()
        .find(|node| node.node_id == expr.root_node_id)
    {
        root.span = root.span.union(SourceSpan {
            start: 0,
            end: source_len,
        });
    }
}

fn parse_internal(source: &str, kind: ExpressionKind) -> Result<Expression, ParseFailure> {
    let tokens = lex(source)?;
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        next_node_id: 1,
        nodes: Vec::new(),
        macro_definitions: None,
        parameter_types: None,
        macro_calls_allowed: true,
    };
    let root_node_id = match kind {
        ExpressionKind::Predicate => parser.parse_let_bool()?,
        ExpressionKind::Outcome => parser.parse_outcome_root()?,
    };
    if !matches!(parser.peek().kind, TokenKind::Eof) {
        let token = parser.peek().clone();
        return Err(ParseFailure::single(
            "MF-DSL-TRAILING",
            "unexpected trailing input".to_string(),
            token.span,
        ));
    }
    Ok(expression_with_spans(
        kind,
        root_node_id,
        parser.nodes,
        source,
    ))
}

fn parse_macro_body(definition: &DslMacroDefinition) -> Result<Expression, ParseFailure> {
    let parameter_types = definition
        .parameters
        .iter()
        .map(|parameter| (parameter.name.clone(), parameter.value_type))
        .collect::<BTreeMap<_, _>>();
    let tokens = lex(&definition.body_source)?;
    let mut parser = Parser {
        source: &definition.body_source,
        tokens,
        pos: 0,
        next_node_id: 1,
        nodes: Vec::new(),
        macro_definitions: None,
        parameter_types: Some(&parameter_types),
        macro_calls_allowed: false,
    };
    let root_node_id = parser.parse_value_expression(definition.return_type)?;
    if !matches!(parser.peek().kind, TokenKind::Eof) {
        let token = parser.peek().clone();
        return Err(ParseFailure::single(
            "MF-DSL-TRAILING",
            "unexpected trailing input".to_string(),
            token.span,
        ));
    }
    Ok(expression_with_spans(
        ExpressionKind::Outcome,
        root_node_id,
        parser.nodes,
        &definition.body_source,
    ))
}

fn macro_parameter_reference(
    node: &Node,
    substitutions: &BTreeMap<&str, NodeId>,
) -> Option<NodeId> {
    let read = match &node.op {
        Op::ReadBool(read) | Op::ReadInt(read) | Op::ReadText(read) => read,
        _ => return None,
    };
    match &read.subject {
        SubjectRef::Binding(binding) if read.field_id == "value" => {
            substitutions.get(binding.name.as_str()).copied()
        }
        _ => None,
    }
}

fn remap_macro_op(op: &Op, ids: &BTreeMap<NodeId, NodeId>) -> Op {
    op.remap_node_ids(&|node_id| *ids.get(&node_id).expect("macro child remapped"))
}

/// Compile an authored expression containing `macro('id', ...)` calls.
///
/// Calls are expanded while parsing, before the returned AST reaches any
/// evaluator or validator.  The result therefore contains no macro-call node
/// and can use every existing expression analysis unchanged.
pub fn compile_expression_with_macros(
    source: &str,
    kind: ExpressionKind,
    definitions: &[DslMacroDefinition],
) -> Result<Expression, MacroCompileFailure> {
    let tokens = lex(source).map_err(MacroCompileFailure::from)?;
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        next_node_id: 1,
        nodes: Vec::new(),
        macro_definitions: Some(definitions),
        parameter_types: None,
        macro_calls_allowed: true,
    };
    let root_node_id = match kind {
        ExpressionKind::Predicate => parser.parse_let_bool(),
        ExpressionKind::Outcome => parser.parse_outcome_root(),
    }
    .map_err(MacroCompileFailure::from)?;
    if !matches!(parser.peek().kind, TokenKind::Eof) {
        let token = parser.peek().clone();
        return Err(MacroCompileFailure {
            diagnostics: vec![Diagnostic {
                code: "MF-DSL-TRAILING",
                message: "unexpected trailing input".to_string(),
                span: token.span,
            }],
        });
    }
    Ok(expression_with_spans(
        kind,
        root_node_id,
        parser.nodes,
        source,
    ))
}

/// Macro-aware counterpart of [`parse_int_expression`].
pub fn compile_int_expression_with_macros(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<Expression, MacroCompileFailure> {
    compile_with_macro_root(source, definitions, DslValueType::Int)
}

/// Macro-aware counterpart of [`parse_predicate`].
pub fn compile_predicate_with_macros(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<Expression, MacroCompileFailure> {
    let mut expression = compile_with_macro_root(source, definitions, DslValueType::Bool)?;
    expression.kind = ExpressionKind::Predicate;
    Ok(expression)
}

/// Compile a Text macro call or expression.  The returned AST is useful to
/// authoring/validation callers even though the current evaluator has no
/// standalone Text-result entry point.
pub fn compile_text_expression_with_macros(
    source: &str,
    definitions: &[DslMacroDefinition],
) -> Result<Expression, MacroCompileFailure> {
    compile_with_macro_root(source, definitions, DslValueType::Text)
}

fn compile_with_macro_root(
    source: &str,
    definitions: &[DslMacroDefinition],
    value_type: DslValueType,
) -> Result<Expression, MacroCompileFailure> {
    let tokens = lex(source).map_err(MacroCompileFailure::from)?;
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        next_node_id: 1,
        nodes: Vec::new(),
        macro_definitions: Some(definitions),
        parameter_types: None,
        macro_calls_allowed: true,
    };
    let root_node_id = parser
        .parse_value_expression(value_type)
        .map_err(MacroCompileFailure::from)?;
    if !matches!(parser.peek().kind, TokenKind::Eof) {
        let token = parser.peek().clone();
        return Err(MacroCompileFailure {
            diagnostics: vec![Diagnostic {
                code: "MF-DSL-TRAILING",
                message: "unexpected trailing input".to_string(),
                span: token.span,
            }],
        });
    }
    Ok(expression_with_spans(
        ExpressionKind::Outcome,
        root_node_id,
        parser.nodes,
        source,
    ))
}

impl From<ParseFailure> for MacroCompileFailure {
    fn from(value: ParseFailure) -> Self {
        Self {
            diagnostics: value.diagnostics,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    StringLit(String),
    Number(i64),
    Bool(bool),
    /// Gap trivia inserted by [`tokenize`], never by the parser lexer.
    Whitespace,
    AndAnd,
    OrOr,
    Dot,
    Dollar,
    LParen,
    RParen,
    Comma,
    Equal,
    Semicolon,
    Minus,
    Plus,
    Star,
    Slash,
    EqEq,
    Ne,
    Lt,
    Lte,
    Gt,
    Ge,
    Eof,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: SourceSpan,
}

fn lex(source: &str) -> Result<Vec<Token>, ParseFailure> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b' ' | b'\n' | b'\r' | b'\t' => i += 1,
            b'&' if bytes.get(i + 1) == Some(&b'&') => {
                tokens.push(Token {
                    kind: TokenKind::AndAnd,
                    span: SourceSpan {
                        start: i,
                        end: i + 2,
                    },
                });
                i += 2;
            }
            b'|' if bytes.get(i + 1) == Some(&b'|') => {
                tokens.push(Token {
                    kind: TokenKind::OrOr,
                    span: SourceSpan {
                        start: i,
                        end: i + 2,
                    },
                });
                i += 2;
            }
            b'=' if bytes.get(i + 1) == Some(&b'=') => {
                tokens.push(Token {
                    kind: TokenKind::EqEq,
                    span: SourceSpan {
                        start: i,
                        end: i + 2,
                    },
                });
                i += 2;
            }
            b'!' if bytes.get(i + 1) == Some(&b'=') => {
                tokens.push(Token {
                    kind: TokenKind::Ne,
                    span: SourceSpan {
                        start: i,
                        end: i + 2,
                    },
                });
                i += 2;
            }
            b'<' if bytes.get(i + 1) == Some(&b'=') => {
                tokens.push(Token {
                    kind: TokenKind::Lte,
                    span: SourceSpan {
                        start: i,
                        end: i + 2,
                    },
                });
                i += 2;
            }
            b'>' if bytes.get(i + 1) == Some(&b'=') => {
                tokens.push(Token {
                    kind: TokenKind::Ge,
                    span: SourceSpan {
                        start: i,
                        end: i + 2,
                    },
                });
                i += 2;
            }
            b'<' => {
                tokens.push(Token {
                    kind: TokenKind::Lt,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'>' => {
                tokens.push(Token {
                    kind: TokenKind::Gt,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'.' => {
                tokens.push(Token {
                    kind: TokenKind::Dot,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'$' => {
                tokens.push(Token {
                    kind: TokenKind::Dollar,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'(' => {
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b')' => {
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b',' => {
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'=' => {
                tokens.push(Token {
                    kind: TokenKind::Equal,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b';' => {
                tokens.push(Token {
                    kind: TokenKind::Semicolon,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'\'' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(ParseFailure::single(
                        "MF-DSL-UNTERMINATED-STRING",
                        "unterminated string literal".to_string(),
                        SourceSpan {
                            start,
                            end: bytes.len(),
                        },
                    ));
                }
                let text = &source[start + 1..i];
                tokens.push(Token {
                    kind: TokenKind::StringLit(text.to_string()),
                    span: SourceSpan { start, end: i + 1 },
                });
                i += 1;
            }
            b'-' => {
                tokens.push(Token {
                    kind: TokenKind::Minus,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'+' => {
                tokens.push(Token {
                    kind: TokenKind::Plus,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'*' => {
                tokens.push(Token {
                    kind: TokenKind::Star,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'/' => {
                tokens.push(Token {
                    kind: TokenKind::Slash,
                    span: SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                });
                i += 1;
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(ParseFailure::single(
                        "MF-DSL-UNTERMINATED-STRING",
                        "unterminated string literal".to_string(),
                        SourceSpan {
                            start,
                            end: bytes.len(),
                        },
                    ));
                }
                let text = &source[start + 1..i];
                tokens.push(Token {
                    kind: TokenKind::StringLit(text.to_string()),
                    span: SourceSpan { start, end: i + 1 },
                });
                i += 1;
            }
            b'0'..=b'9' => {
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let text = &source[start..i];
                let value = text.parse::<i64>().map_err(|_| {
                    ParseFailure::single(
                        "MF-DSL-BAD-NUMBER",
                        format!("invalid integer literal `{text}`"),
                        SourceSpan { start, end: i },
                    )
                })?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    span: SourceSpan { start, end: i },
                });
            }
            b'_' | b'a'..=b'z' | b'A'..=b'Z' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &source[start..i];
                let kind = match ident {
                    "true" => TokenKind::Bool(true),
                    "false" => TokenKind::Bool(false),
                    _ => TokenKind::Ident(ident.to_string()),
                };
                tokens.push(Token {
                    kind,
                    span: SourceSpan { start, end: i },
                });
            }
            _ => {
                return Err(ParseFailure::single(
                    "MF-DSL-UNKNOWN-CHAR",
                    "unknown token".to_string(),
                    SourceSpan {
                        start: i,
                        end: i + 1,
                    },
                ));
            }
        }
    }
    tokens.push(Token {
        kind: TokenKind::Eof,
        span: SourceSpan {
            start: source.len(),
            end: source.len(),
        },
    });
    Ok(tokens)
}

/// Lex `source` into tokens that cover the input exhaustively, including
/// whitespace trivia. Concatenating `source[token.span.start..token.span.end]`
/// for every returned token (Eof is omitted) reproduces `source` byte for byte.
///
/// The parser lexer is unchanged: it still skips trivia and appends Eof.
/// Dice literals (`d20`, `2d6`) stay a parse recognition, not a token kind.
pub fn tokenize(source: &str) -> Result<Vec<Token>, ParseFailure> {
    let lexed = lex(source)?;
    let mut tokens = Vec::new();
    let mut cursor = 0usize;
    for token in lexed {
        if matches!(token.kind, TokenKind::Eof) {
            if cursor < source.len() {
                tokens.push(Token {
                    kind: TokenKind::Whitespace,
                    span: SourceSpan {
                        start: cursor,
                        end: source.len(),
                    },
                });
            }
            break;
        }
        if token.span.start > cursor {
            tokens.push(Token {
                kind: TokenKind::Whitespace,
                span: SourceSpan {
                    start: cursor,
                    end: token.span.start,
                },
            });
        }
        cursor = token.span.end;
        tokens.push(token);
    }
    Ok(tokens)
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    next_node_id: NodeId,
    nodes: Vec<Node>,
    macro_definitions: Option<&'a [DslMacroDefinition]>,
    parameter_types: Option<&'a BTreeMap<String, DslValueType>>,
    macro_calls_allowed: bool,
}

impl Parser<'_> {
    fn parse_value_expression(&mut self, value_type: DslValueType) -> Result<NodeId, ParseFailure> {
        match value_type {
            ValueType::Bool => self.parse_let_bool(),
            ValueType::Int => self.parse_int_expr(),
            ValueType::Text => self.parse_text_expression(),
            ValueType::RowRef | ValueType::Outcome => Err(ParseFailure::single(
                "MF-DSL-MACRO-UNSUPPORTED-TYPE",
                "macros may return only Bool, Int, or Text".to_string(),
                self.peek().span,
            )),
        }
    }

    fn parse_text_expression(&mut self) -> Result<NodeId, ParseFailure> {
        if self.peek_ident("macro") {
            return self.parse_macro_invocation(ValueType::Text);
        }
        let token = self.next();
        match token.kind {
            TokenKind::StringLit(value) => {
                Ok(self.add_node(ValueType::Text, Op::TextLiteral(TextLiteral { value })))
            }
            TokenKind::Ident(name) if self.parameter_type(&name) == Some(ValueType::Text) => {
                Ok(self.add_node(
                    ValueType::Text,
                    Op::ReadText(ReadField {
                        subject: SubjectRef::Binding(BindingRef { name }),
                        field_id: "value".to_string(),
                    }),
                ))
            }
            _ => Err(ParseFailure::single(
                "MF-DSL-EXPECTED-TEXT",
                "expected text expression".to_string(),
                token.span,
            )),
        }
    }

    fn parameter_type(&self, name: &str) -> Option<DslValueType> {
        self.parameter_types
            .and_then(|types| types.get(name).copied())
    }

    fn parse_macro_invocation(&mut self, expected: DslValueType) -> Result<NodeId, ParseFailure> {
        let macro_token = self.next();
        if !self.macro_calls_allowed {
            return Err(ParseFailure::single(
                "MF-DSL-MACRO-IN-BODY",
                "macro calls are not allowed in macro bodies".to_string(),
                macro_token.span,
            ));
        }
        let definitions = self.macro_definitions.ok_or_else(|| {
            ParseFailure::single(
                "MF-DSL-MACRO-UNAVAILABLE",
                "macro calls require the macro-aware compiler".to_string(),
                macro_token.span,
            )
        })?;
        self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
        let id_span = self.peek().span;
        let macro_id = self.expect_string("MF-DSL-MACRO-ID", "expected quoted macro id")?;
        let definition = definitions
            .iter()
            .find(|definition| definition.macro_id == macro_id)
            .cloned()
            .ok_or_else(|| {
                ParseFailure::single(
                    "MF-DSL-MACRO-UNKNOWN",
                    format!("unknown macro `{macro_id}`"),
                    id_span,
                )
            })?;
        if definition.return_type != expected {
            return Err(ParseFailure::single(
                "MF-DSL-MACRO-RETURN-TYPE",
                format!(
                    "macro `{}` returns {:?}, not {:?}",
                    definition.macro_id, definition.return_type, expected
                ),
                id_span,
            ));
        }
        let mut arguments = Vec::with_capacity(definition.parameters.len());
        for (index, parameter) in definition.parameters.iter().enumerate() {
            self.expect_simple(
                TokenKind::Comma,
                "MF-DSL-MACRO-ARITY",
                "expected `,` before macro argument",
            )?;
            let argument = self.parse_value_expression(parameter.value_type)?;
            let actual = self
                .nodes
                .iter()
                .find(|node| node.node_id == argument)
                .map(|node| node.value_type)
                .expect("parsed macro argument node exists");
            if actual != parameter.value_type {
                return Err(ParseFailure::single(
                    "MF-DSL-MACRO-ARG-TYPE",
                    format!(
                        "macro `{}` parameter `{}` expects {:?}, got {:?}",
                        definition.macro_id, parameter.name, parameter.value_type, actual
                    ),
                    self.peek().span,
                ));
            }
            arguments.push(argument);
            if index + 1 == definition.parameters.len()
                && matches!(self.peek().kind, TokenKind::Comma)
            {
                return Err(ParseFailure::single(
                    "MF-DSL-MACRO-ARITY",
                    format!(
                        "macro `{}` received too many arguments",
                        definition.macro_id
                    ),
                    self.peek().span,
                ));
            }
        }
        if definition.parameters.is_empty() && matches!(self.peek().kind, TokenKind::Comma) {
            return Err(ParseFailure::single(
                "MF-DSL-MACRO-ARITY",
                format!(
                    "macro `{}` received too many arguments",
                    definition.macro_id
                ),
                self.peek().span,
            ));
        }
        self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
        let body = parse_macro_body(&definition)?;
        self.graft_macro_body(&body, &definition.parameters, &arguments)
    }

    fn graft_macro_body(
        &mut self,
        body: &Expression,
        parameters: &[DslMacroParameter],
        arguments: &[NodeId],
    ) -> Result<NodeId, ParseFailure> {
        let substitutions: BTreeMap<&str, NodeId> = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.name.as_str(), *argument))
            .collect();
        let mut ids = BTreeMap::new();
        for node in &body.nodes {
            if let Some(argument) = macro_parameter_reference(node, &substitutions) {
                ids.insert(node.node_id, argument);
            }
        }
        for node in &body.nodes {
            if macro_parameter_reference(node, &substitutions).is_none() {
                let id = self.next_node_id;
                self.next_node_id += 1;
                ids.insert(node.node_id, id);
            }
        }
        for node in &body.nodes {
            if macro_parameter_reference(node, &substitutions).is_some() {
                continue;
            }
            self.nodes.push(Node {
                node_id: *ids.get(&node.node_id).expect("macro node id allocated"),
                value_type: node.value_type,
                op: remap_macro_op(&node.op, &ids),
                span: node.span,
            });
        }
        Ok(*ids.get(&body.root_node_id).expect("macro root remapped"))
    }

    fn parse_outcome_root(&mut self) -> Result<NodeId, ParseFailure> {
        self.parse_outcome_expr()
    }

    fn parse_outcome_expr(&mut self) -> Result<NodeId, ParseFailure> {
        if self.peek_ident("sequence") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let mut steps = vec![self.parse_outcome_expr()?];
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.pos += 1;
                steps.push(self.parse_outcome_expr()?);
            }
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Outcome,
                Op::OutcomeSequence(OutcomeSequence { steps }),
            ));
        }

        if self.peek_ident("invoke_outcome_graph") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let outcome_graph_id =
                self.expect_quoted_string("MF-DSL-EXPECTED-STRING", "expected outcome graph id")?;
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Outcome,
                Op::InvokeOutcomeGraph(InvokeOutcomeGraph { outcome_graph_id }),
            ));
        }

        if self.peek_ident("arbitrate") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let request_shape_id = self
                .expect_quoted_string("MF-DSL-EXPECTED-STRING", "expected arbitration shape id")?;
            self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
            let context = self.expect_quoted_string(
                "MF-DSL-EXPECTED-STRING",
                "expected arbitration context fact",
            )?;
            let context_node = self.add_text_literal(context);
            self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
            let first_allowed = self.expect_quoted_string(
                "MF-DSL-EXPECTED-STRING",
                "expected arbitration allowed outcome",
            )?;
            let mut allowed_outcome_nodes = vec![self.add_text_literal(first_allowed)];
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.pos += 1;
                let allowed = self.expect_quoted_string(
                    "MF-DSL-EXPECTED-STRING",
                    "expected arbitration allowed outcome",
                )?;
                allowed_outcome_nodes.push(self.add_text_literal(allowed));
            }
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Outcome,
                Op::Arbitrate(Arbitrate {
                    request_shape_id,
                    context_nodes: vec![context_node],
                    allowed_outcome_nodes,
                }),
            ));
        }

        if self.peek_ident("let") {
            self.parse_let_int()
        } else {
            self.parse_let_bool()
        }
    }

    fn add_text_literal(&mut self, value: String) -> NodeId {
        self.add_node(ValueType::Text, Op::TextLiteral(TextLiteral { value }))
    }

    fn parse_let_bool(&mut self) -> Result<NodeId, ParseFailure> {
        if self.peek_ident("let") {
            self.pos += 1;
            let name = self.expect_ident("MF-DSL-EXPECTED-BINDING", "expected binding name")?;
            self.expect_simple(TokenKind::Equal, "MF-DSL-EXPECTED-EQUAL", "expected `=`")?;
            let value = self.parse_int_expr()?;
            self.expect_simple(
                TokenKind::Semicolon,
                "MF-DSL-EXPECTED-SEMICOLON",
                "expected `;`",
            )?;
            let body = self.parse_let_bool()?;
            let value_type = self
                .nodes
                .iter()
                .find(|node| node.node_id == body)
                .map(|node| node.value_type)
                .unwrap_or(ValueType::Bool);
            return Ok(self.add_node(value_type, Op::Let(Let { name, value, body })));
        }
        self.parse_and()
    }

    fn parse_let_int(&mut self) -> Result<NodeId, ParseFailure> {
        if self.peek_ident("let") {
            self.pos += 1;
            let name = self.expect_ident("MF-DSL-EXPECTED-BINDING", "expected binding name")?;
            self.expect_simple(TokenKind::Equal, "MF-DSL-EXPECTED-EQUAL", "expected `=`")?;
            let value = self.parse_int_expr()?;
            self.expect_simple(
                TokenKind::Semicolon,
                "MF-DSL-EXPECTED-SEMICOLON",
                "expected `;`",
            )?;
            let body = self.parse_let_int()?;
            return Ok(self.add_node(ValueType::Int, Op::Let(Let { name, value, body })));
        }
        self.parse_int_expr()
    }

    fn parse_and(&mut self) -> Result<NodeId, ParseFailure> {
        let mut operands = vec![self.parse_or()?];
        while matches!(self.peek().kind, TokenKind::AndAnd) {
            self.pos += 1;
            operands.push(self.parse_or()?);
        }
        if operands.len() == 1 {
            Ok(operands[0])
        } else {
            Ok(self.add_node(ValueType::Bool, Op::And(Operands { operands })))
        }
    }

    fn parse_or(&mut self) -> Result<NodeId, ParseFailure> {
        let mut operands = vec![self.parse_bool_term()?];
        while matches!(self.peek().kind, TokenKind::OrOr) {
            self.pos += 1;
            operands.push(self.parse_bool_term()?);
        }
        if operands.len() == 1 {
            Ok(operands[0])
        } else {
            Ok(self.add_node(ValueType::Bool, Op::Or(Operands { operands })))
        }
    }

    fn parse_bool_term(&mut self) -> Result<NodeId, ParseFailure> {
        if self.peek_ident("macro") {
            return self.parse_macro_invocation(ValueType::Bool);
        }
        if let TokenKind::Ident(name) = &self.peek().kind {
            if self.parameter_type(name) == Some(ValueType::Bool) {
                let name = name.clone();
                self.next();
                return Ok(self.add_node(
                    ValueType::Bool,
                    Op::ReadBool(ReadField {
                        subject: SubjectRef::Binding(BindingRef { name }),
                        field_id: "value".to_string(),
                    }),
                ));
            }
        }
        if self.peek_ident("roll_under") {
            return self.parse_roll_under();
        }

        if self.peek_ident("shares_tag") {
            return self.parse_shares_tag();
        }

        if self.peek_subject() {
            let mark = self.pos;
            match self.parse_method_bool() {
                Ok(node) => return Ok(node),
                Err(err)
                    if err
                        .diagnostics
                        .first()
                        .map(|d| d.code == "MF-DSL-UNKNOWN-PRIMITIVE")
                        .unwrap_or(false) =>
                {
                    return Err(err);
                }
                Err(_) => {
                    self.pos = mark;
                }
            }
        }

        let left = self.parse_int_expr()?;
        let compare_op = match self.peek().kind {
            TokenKind::Ge => CompareOp::Gte,
            TokenKind::Gt => CompareOp::Gt,
            TokenKind::Lte => CompareOp::Lte,
            TokenKind::Lt => CompareOp::Lt,
            TokenKind::EqEq => CompareOp::Eq,
            TokenKind::Ne => CompareOp::Ne,
            _ => {
                let token = self.peek().clone();
                return Err(ParseFailure::single(
                    "MF-DSL-EXPECTED-COMPARE",
                    "expected comparison operator".to_string(),
                    token.span,
                ));
            }
        };
        self.pos += 1;
        let right = self.parse_int_expr()?;
        Ok(self.add_node(
            ValueType::Bool,
            Op::Compare(Compare {
                op: compare_op,
                left,
                right,
            }),
        ))
    }

    fn parse_method_bool(&mut self) -> Result<NodeId, ParseFailure> {
        let subject = self.parse_subject()?;
        self.expect_simple(TokenKind::Dot, "MF-DSL-EXPECTED-DOT", "expected `.`")?;
        let method = self.expect_ident("MF-DSL-EXPECTED-METHOD", "expected method name")?;
        self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
        let arg = self.expect_string_or_binding_path(
            "MF-DSL-EXPECTED-STRING",
            "expected string literal or binding path",
        )?;
        self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;

        let node_id = match method.as_str() {
            "has_capability" => self.add_node(
                ValueType::Bool,
                Op::HasCapability(HasCapability {
                    subject,
                    capability_id: arg,
                }),
            ),
            "has_item" => self.add_node(
                ValueType::Bool,
                Op::HasItem(HasItem {
                    subject,
                    item_def_id: arg,
                }),
            ),
            "has_niche" => self.add_node(
                ValueType::Bool,
                Op::HasNiche(HasNiche {
                    subject,
                    niche_id: arg,
                }),
            ),
            "has_tag" => self.add_node(ValueType::Bool, Op::HasTag(HasTag { subject, tag: arg })),
            "status" => self.add_node(
                ValueType::Bool,
                Op::HasStatus(HasStatus {
                    subject,
                    condition_definition_id: arg,
                }),
            ),
            "state" => self.add_node(
                ValueType::Bool,
                Op::ReadBool(ReadField {
                    subject,
                    field_id: arg,
                }),
            ),
            "trace" => self.add_node(
                ValueType::Bool,
                Op::TracePresent(TracePresent {
                    subject,
                    trace_kind_id: arg,
                }),
            ),
            "knows_entity" => self.add_node(
                ValueType::Bool,
                Op::KnowsEntity(KnowsEntity {
                    subject,
                    entity_id: arg,
                }),
            ),
            "stat" | "effective" | "armor" | "resource_max" | "resource_min" => {
                // int-typed field access — caller backtracks to parse_int_expr
                return Err(ParseFailure::single(
                    "MF-DSL-INT-FIELD",
                    format!("`{method}` is an int field, not a bool method"),
                    self.slice_span_for_name(&method),
                ));
            }
            _ => {
                return Err(ParseFailure::single(
                    "MF-DSL-UNKNOWN-PRIMITIVE",
                    format!("unknown primitive `{method}`"),
                    self.slice_span_for_name(&method),
                ))
            }
        };

        if matches!(self.peek().kind, TokenKind::EqEq | TokenKind::Ne) {
            let negate = matches!(self.peek().kind, TokenKind::Ne);
            self.pos += 1;
            match self.next().kind {
                TokenKind::Bool(true) => {
                    if negate {
                        Ok(self.add_node(ValueType::Bool, Op::Not(UnaryNode { operand: node_id })))
                    } else {
                        Ok(node_id)
                    }
                }
                TokenKind::Bool(false) => {
                    if negate {
                        Ok(node_id)
                    } else {
                        Ok(self.add_node(ValueType::Bool, Op::Not(UnaryNode { operand: node_id })))
                    }
                }
                _ => Err(ParseFailure::single(
                    "MF-DSL-EXPECTED-BOOL",
                    "expected boolean literal after `==` or `!=`".to_string(),
                    self.peek().span,
                )),
            }
        } else {
            Ok(node_id)
        }
    }

    fn parse_roll_under(&mut self) -> Result<NodeId, ParseFailure> {
        let fn_name = self.expect_ident("MF-DSL-EXPECTED-IDENT", "expected identifier")?;
        if fn_name != "roll_under" {
            return Err(ParseFailure::single(
                "MF-DSL-UNKNOWN-PRIMITIVE",
                format!("unknown primitive `{fn_name}`"),
                self.slice_span_for_name(&fn_name),
            ));
        }
        self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
        let die_span = self.peek().span;
        let dice = self.try_parse_dice().ok_or_else(|| {
            ParseFailure::single(
                "MF-DSL-EXPECTED-DIE",
                "expected die token (e.g. d20, 2d6)".to_string(),
                die_span,
            )
        })?;
        self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
        let target_node = self.parse_int_expr()?;
        self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
        let roll_node = self.add_node(ValueType::Int, Op::Roll(Roll { dice }));
        Ok(self.add_node(
            ValueType::Bool,
            Op::RollUnder(RollUnder {
                roll_node,
                target_node,
            }),
        ))
    }

    /// `shares_tag(a, b, 'prefix')` — a free-function predicate over TWO subjects
    /// and an authored tag-id prefix (mirrors the `roll_under` free-function
    /// shape, not the `subject.method(arg)` shape). Emits `Op::SharesTag`.
    fn parse_shares_tag(&mut self) -> Result<NodeId, ParseFailure> {
        let fn_name = self.expect_ident("MF-DSL-EXPECTED-IDENT", "expected identifier")?;
        if fn_name != "shares_tag" {
            return Err(ParseFailure::single(
                "MF-DSL-UNKNOWN-PRIMITIVE",
                format!("unknown primitive `{fn_name}`"),
                self.slice_span_for_name(&fn_name),
            ));
        }
        self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
        let subject_a = self.parse_subject()?;
        self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
        let subject_b = self.parse_subject()?;
        self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
        let prefix = self.expect_quoted_string(
            "MF-DSL-EXPECTED-STRING",
            "expected tag-id prefix string literal",
        )?;
        self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
        Ok(self.add_node(
            ValueType::Bool,
            Op::SharesTag(SharesTag {
                subject_a,
                subject_b,
                prefix,
            }),
        ))
    }

    /// Recognize a dice atom at the cursor and consume it: `dN` (`Ident("dN")`,
    /// implicit count 1) or `XdN` (`Number(X)` then `Ident("dN")`, count X).
    /// Returns the `Dice` on a match (cursor advanced past it), or `None`
    /// without consuming. The shared dice grammar (CR-17 / LR-17): both the
    /// standalone int atom and `roll_under` route through here, so `roll_under`
    /// loses its hardcoded `d20` special case and `d20` still parses to the
    /// byte-identical `Dice { count: 1, sides: 20 }`.
    fn try_parse_dice(&mut self) -> Option<Dice> {
        let cur = self.tokens.get(self.pos).map(|t| &t.kind);
        let nxt = self.tokens.get(self.pos + 1).map(|t| &t.kind);
        // `XdN`: a positive integer multiplier immediately followed by a `dN` ident.
        if let (Some(TokenKind::Number(count)), Some(TokenKind::Ident(ident))) = (cur, nxt) {
            if let (Ok(count), Some(sides)) = (u32::try_from(*count), dice_sides_from_ident(ident))
            {
                if count >= 1 {
                    self.pos += 2;
                    return Some(Dice { count, sides });
                }
            }
        }
        // `dN`: a bare die ident with implicit count 1.
        if let Some(TokenKind::Ident(ident)) = cur {
            if let Some(sides) = dice_sides_from_ident(ident) {
                self.pos += 1;
                return Some(Dice { count: 1, sides });
            }
        }
        None
    }

    /// Additive layer: `term ((`+`|`-`) term)*`, left-associative. Wires the
    /// existing `Arithmetic::{Add,Sub}` ops to author-facing `+`/`-` (ADR-0045).
    fn parse_int_expr(&mut self) -> Result<NodeId, ParseFailure> {
        let mut left = self.parse_int_term()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => ArithmeticOp::Add,
                TokenKind::Minus => ArithmeticOp::Sub,
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_int_term()?;
            left = self.add_node(
                ValueType::Int,
                Op::Arithmetic(Arithmetic { op, left, right }),
            );
        }
        Ok(left)
    }

    /// Multiplicative layer: `unary ((`*`|`/`) unary)*`, left-associative —
    /// binds tighter than `+`/`-`.
    fn parse_int_term(&mut self) -> Result<NodeId, ParseFailure> {
        let mut left = self.parse_int_unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => ArithmeticOp::Mul,
                TokenKind::Slash => ArithmeticOp::Div,
                _ => break,
            };
            self.pos += 1;
            let right = self.parse_int_unary()?;
            left = self.add_node(
                ValueType::Int,
                Op::Arithmetic(Arithmetic { op, left, right }),
            );
        }
        Ok(left)
    }

    /// Unary minus: `-atom` desugars to `0 - atom` (no negate op — reuses
    /// `Arithmetic::Sub`). Stacks (`--x`) by recursion.
    fn parse_int_unary(&mut self) -> Result<NodeId, ParseFailure> {
        if matches!(self.peek().kind, TokenKind::Minus) {
            self.pos += 1;
            let operand = self.parse_int_unary()?;
            let zero = self.add_node(ValueType::Int, Op::IntLiteral(IntLiteral { value: 0 }));
            return Ok(self.add_node(
                ValueType::Int,
                Op::Arithmetic(Arithmetic {
                    op: ArithmeticOp::Sub,
                    left: zero,
                    right: operand,
                }),
            ));
        }
        self.parse_int_atom()
    }

    fn parse_int_atom(&mut self) -> Result<NodeId, ParseFailure> {
        if self.peek_ident("macro") {
            return self.parse_macro_invocation(ValueType::Int);
        }
        if self.peek_ident("let") {
            return self.parse_let_int();
        }
        if self.peek_ident("max") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let left = self.parse_int_expr()?;
            self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
            let right = self.parse_int_expr()?;
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Int,
                Op::Arithmetic(Arithmetic {
                    op: ArithmeticOp::Max,
                    left,
                    right,
                }),
            ));
        }
        if self.peek_ident("min") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let left = self.parse_int_expr()?;
            self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
            let right = self.parse_int_expr()?;
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Int,
                Op::Arithmetic(Arithmetic {
                    op: ArithmeticOp::Min,
                    left,
                    right,
                }),
            ));
        }
        if self.peek_ident("roll_table") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let table_id_node = self.parse_text_id_or_read_node()?;
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            let draw_node = self.add_node(ValueType::Int, Op::IntLiteral(IntLiteral { value: 0 }));
            return Ok(self.add_node(
                ValueType::Int,
                Op::RollTable(RollTable {
                    table_id_node,
                    draw_node,
                }),
            ));
        }
        if self.peek_ident("cond") {
            return self.parse_cond();
        }
        if self.peek_ident("implied_by_count") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let condition_definition_id =
                if matches!(self.peek().kind, TokenKind::RParen) {
                    None
                } else {
                    Some(self.expect_string(
                        "MF-DSL-EXPECTED-STRING",
                        "expected condition definition id",
                    )?)
                };
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Int,
                Op::ImpliedByCount(ImpliedByCount {
                    condition_definition_id,
                }),
            ));
        }
        if self.peek_ident("participant_tag_group_count") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let prefix = self.expect_string("MF-DSL-EXPECTED-STRING", "expected tag prefix")?;
            let excluding_condition_definition_id =
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.pos += 1;
                    Some(self.expect_string(
                        "MF-DSL-EXPECTED-STRING",
                        "expected condition definition id",
                    )?)
                } else {
                    None
                };
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Int,
                Op::ParticipantTagGroupCount(ParticipantTagGroupCount {
                    prefix,
                    excluding_condition_definition_id,
                }),
            ));
        }
        if self.peek_ident("peers_near_target") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let prefix = self.expect_string("MF-DSL-EXPECTED-STRING", "expected tag prefix")?;
            self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
            let radius_mm = self.expect_int_literal("MF-DSL-EXPECTED-INT", "expected radius_mm")?;
            let excluding_condition_definition_id =
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.pos += 1;
                    Some(self.expect_string(
                        "MF-DSL-EXPECTED-STRING",
                        "expected condition definition id",
                    )?)
                } else {
                    None
                };
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Int,
                Op::PeersNearTarget(PeersNearTarget {
                    prefix,
                    radius_mm,
                    excluding_condition_definition_id,
                }),
            ));
        }
        if self.peek_ident("occupants_at") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let scope = self.expect_string("MF-DSL-EXPECTED-STRING", "expected node scope")?;
            let excluding_condition_definition_id =
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.pos += 1;
                    Some(self.expect_string(
                        "MF-DSL-EXPECTED-STRING",
                        "expected condition definition id",
                    )?)
                } else {
                    None
                };
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(
                ValueType::Int,
                Op::OccupantsAt(OccupantsAt {
                    scope,
                    excluding_condition_definition_id,
                }),
            ));
        }
        if self.peek_ident("channel_pool") {
            self.pos += 1;
            self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
            let channel_id = self.expect_string("MF-DSL-EXPECTED-STRING", "expected channel id")?;
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(self.add_node(ValueType::Int, Op::ChannelPool(ChannelPool { channel_id })));
        }

        // Parenthesized grouping. Reached only when the cursor sits directly on
        // `(` — the function keywords above (`max`/`min`/`roll_table`/`cond`)
        // consume their own ident first, so this never swallows their parens.
        if matches!(self.peek().kind, TokenKind::LParen) {
            self.pos += 1;
            let inner = self.parse_int_expr()?;
            self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
            return Ok(inner);
        }

        // A standalone dice atom (`d20`, `2d6`) is a raw stochastic int term.
        // Intercept BEFORE the binding catch-all below: a bare `dN`/`XdN` ident
        // would otherwise parse as a free-variable read (CR-17 / LR-17). `dNN`-
        // shaped identifiers are therefore reserved for dice in int position.
        if let Some(dice) = self.try_parse_dice() {
            return Ok(self.add_node(ValueType::Int, Op::Roll(Roll { dice })));
        }

        let token = self.next();
        match token.kind {
            TokenKind::Dollar => self.parse_event_binding_ref(token.span),
            TokenKind::Number(value) => {
                Ok(self.add_node(ValueType::Int, Op::IntLiteral(IntLiteral { value })))
            }
            TokenKind::Bool(value) => {
                Ok(self.add_node(ValueType::Bool, Op::BoolLiteral(BoolLiteral { value })))
            }
            TokenKind::StringLit(value) => {
                Ok(self.add_node(ValueType::Text, Op::TextLiteral(TextLiteral { value })))
            }
            TokenKind::Ident(subject)
                if subject == "actor"
                    || subject == "target"
                    || subject == "session"
                    || subject == "world"
                    || subject == "bindings" =>
            {
                self.expect_simple(TokenKind::Dot, "MF-DSL-EXPECTED-DOT", "expected `.`")?;
                let subject_ref = if subject == "bindings" {
                    let name =
                        self.expect_ident("MF-DSL-EXPECTED-BINDING", "expected binding name")?;
                    self.expect_simple(TokenKind::Dot, "MF-DSL-EXPECTED-DOT", "expected `.`")?;
                    SubjectRef::Binding(BindingRef { name })
                } else {
                    subject_from_ident(&subject).map_err(|code| {
                        ParseFailure::single(
                            code,
                            format!("unknown subject `{subject}`"),
                            token.span,
                        )
                    })?
                };
                let field = self.expect_ident("MF-DSL-EXPECTED-FIELD", "expected field name")?;
                // Chained relation accessor: `actor.wielded_item.<field>`. The
                // relation token re-roots the subject; the real field follows the
                // next `.`. Closed K-5 vocab (ADR-0021 D-2); shares the
                // `relation_resubject` source of truth with the bool-method path.
                let relation_subject =
                    if subject_ref == SubjectRef::Actor && field == "equipped_item" {
                        Some(self.parse_equipped_item_subject()?)
                    } else {
                        relation_resubject(&subject_ref, &field)
                    };
                let (subject_ref, field) = match relation_subject {
                    Some(resubject) => {
                        // A relation accessor MUST be followed by `.field`; a bare
                        // `actor.wielded_item` is rejected — it is not itself a value.
                        if !matches!(self.peek().kind, TokenKind::Dot) {
                            return Err(ParseFailure::single(
                                "MF-DSL-DANGLING-RELATION",
                                format!(
                                    "`{field}` is a relation accessor; read a field off it, \
                                     e.g. `actor.{field}.stat('…')`"
                                ),
                                token.span,
                            ));
                        }
                        self.next(); // consume `.`
                        let real_field =
                            self.expect_ident("MF-DSL-EXPECTED-FIELD", "expected field name")?;
                        (resubject, real_field)
                    }
                    None => (subject_ref, field),
                };
                let field = if subject_ref == SubjectRef::World && field == "tick" {
                    WORLD_TICK_FIELD_ID.to_string()
                } else {
                    field
                };
                if field == "stat" {
                    self.expect_simple(
                        TokenKind::LParen,
                        "MF-DSL-EXPECTED-LPAREN",
                        "expected `(`",
                    )?;
                    let stat = self.expect_string("MF-DSL-EXPECTED-STRING", "expected stat id")?;
                    self.expect_simple(
                        TokenKind::RParen,
                        "MF-DSL-EXPECTED-RPAREN",
                        "expected `)`",
                    )?;
                    Ok(self.add_node(
                        ValueType::Int,
                        Op::ReadInt(ReadField {
                            subject: subject_ref,
                            field_id: stat,
                        }),
                    ))
                } else if field == "effective" {
                    // #1247: explicit read of the EFFECTIVE (modifier-composed)
                    // value via the `effective_int_fields` overlay. `stat` stays
                    // strictly base (ADR-0035 zirkularity cut); `effective` is the
                    // author opt-in to gate on composed condition modifiers.
                    self.expect_simple(
                        TokenKind::LParen,
                        "MF-DSL-EXPECTED-LPAREN",
                        "expected `(`",
                    )?;
                    let stat = self.expect_string("MF-DSL-EXPECTED-STRING", "expected stat id")?;
                    self.expect_simple(
                        TokenKind::RParen,
                        "MF-DSL-EXPECTED-RPAREN",
                        "expected `)`",
                    )?;
                    Ok(self.add_node(
                        ValueType::Int,
                        Op::ReadEffectiveInt(ReadField {
                            subject: subject_ref,
                            field_id: stat,
                        }),
                    ))
                } else if field == "resource_max" {
                    // #30 / sk-ct8.2: the EFFECTIVE per-entity MAX bound for a
                    // resource — `entity_resource_bound` override else the
                    // bundle-wide `resource_definition.max_value`. A dedicated
                    // leaf op (not `stat(field_id)`) because the lookup is a
                    // parameterized table-backed constant, not a per-subject
                    // fact string — see the design draft.
                    self.expect_simple(
                        TokenKind::LParen,
                        "MF-DSL-EXPECTED-LPAREN",
                        "expected `(`",
                    )?;
                    let resource_id =
                        self.expect_string("MF-DSL-EXPECTED-STRING", "expected resource id")?;
                    self.expect_simple(
                        TokenKind::RParen,
                        "MF-DSL-EXPECTED-RPAREN",
                        "expected `)`",
                    )?;
                    Ok(self.add_node(
                        ValueType::Int,
                        Op::ReadResourceMax(ReadField {
                            subject: subject_ref,
                            field_id: resource_id,
                        }),
                    ))
                } else if field == "resource_min" {
                    // #30 / sk-ct8.2: sibling of `resource_max` — the EFFECTIVE
                    // per-entity MIN bound.
                    self.expect_simple(
                        TokenKind::LParen,
                        "MF-DSL-EXPECTED-LPAREN",
                        "expected `(`",
                    )?;
                    let resource_id =
                        self.expect_string("MF-DSL-EXPECTED-STRING", "expected resource id")?;
                    self.expect_simple(
                        TokenKind::RParen,
                        "MF-DSL-EXPECTED-RPAREN",
                        "expected `)`",
                    )?;
                    Ok(self.add_node(
                        ValueType::Int,
                        Op::ReadResourceMin(ReadField {
                            subject: subject_ref,
                            field_id: resource_id,
                        }),
                    ))
                } else if field == "substrate_id" {
                    // D3 (Item 24): substrate_id is a text field injected by r_sense
                    // when the actor / target node is resolved to a place_graph substrate.
                    Ok(self.add_node(
                        ValueType::Text,
                        Op::ReadText(ReadField {
                            subject: subject_ref,
                            field_id: field,
                        }),
                    ))
                } else if field == "source" {
                    // sk-84i.2 (widened G-1): `<subject>.source('slot')` reads
                    // `subject`'s named `item_dsl_source` slot inline — the
                    // same subject-parameterised shape every sibling
                    // fact-reading op uses. Shares the roll_table id
                    // argument's text-valued-node grammar
                    // (`parse_text_id_or_read_node`): a quoted literal or a
                    // `<subject>.text('id')` runtime-resolved read.
                    self.expect_simple(
                        TokenKind::LParen,
                        "MF-DSL-EXPECTED-LPAREN",
                        "expected `(`",
                    )?;
                    let slot_node = self.parse_text_id_or_read_node()?;
                    self.expect_simple(
                        TokenKind::RParen,
                        "MF-DSL-EXPECTED-RPAREN",
                        "expected `)`",
                    )?;
                    Ok(self.add_node(
                        ValueType::Int,
                        Op::DslSource(DslSource {
                            subject: subject_ref,
                            slot_node,
                        }),
                    ))
                } else {
                    Ok(self.add_node(
                        ValueType::Int,
                        Op::ReadInt(ReadField {
                            subject: subject_ref,
                            field_id: field,
                        }),
                    ))
                }
            }
            TokenKind::Ident(ident) => Ok(self.add_node(
                ValueType::Int,
                Op::ReadInt(ReadField {
                    subject: SubjectRef::Binding(BindingRef { name: ident }),
                    field_id: "value".into(),
                }),
            )),
            _ => Err(ParseFailure::single(
                "MF-DSL-EXPECTED-INT",
                "expected integer expression".to_string(),
                token.span,
            )),
        }
    }

    /// `cond(<bool>, <int>, <int>)` — typed ternary (ADR-0045 D-3). The gate is
    /// a full Bool expression; both branches are full Int expressions.
    fn parse_cond(&mut self) -> Result<NodeId, ParseFailure> {
        self.pos += 1; // consume `cond`
        self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
        let cond = self.parse_let_bool()?;
        self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
        let then_node = self.parse_int_expr()?;
        self.expect_simple(TokenKind::Comma, "MF-DSL-EXPECTED-COMMA", "expected `,`")?;
        let else_node = self.parse_int_expr()?;
        self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
        Ok(self.add_node(
            ValueType::Int,
            Op::Cond(Cond {
                cond,
                then_node,
                else_node,
            }),
        ))
    }

    fn parse_event_binding_ref(&mut self, dollar_span: SourceSpan) -> Result<NodeId, ParseFailure> {
        let scope = self.expect_ident(
            "MF-DSL-EXPECTED-BINDING-SCOPE",
            "expected binding scope after `$`",
        )?;
        if scope != "event" {
            return Err(ParseFailure::single(
                "MF-DSL-UNKNOWN-BINDING-SCOPE",
                format!("unknown binding scope `{scope}`"),
                dollar_span,
            ));
        }

        self.expect_simple(TokenKind::Dot, "MF-DSL-EXPECTED-DOT", "expected `.`")?;
        let segment = self.expect_ident("MF-DSL-EXPECTED-FIELD", "expected event field")?;
        match segment.as_str() {
            "kind" => Ok(self.add_node(
                ValueType::Text,
                Op::ReadText(ReadField {
                    subject: SubjectRef::Event(EventRef::Kind),
                    field_id: "kind".into(),
                }),
            )),
            "detail" => {
                self.expect_simple(TokenKind::Dot, "MF-DSL-EXPECTED-DOT", "expected `.`")?;
                let field =
                    self.expect_ident("MF-DSL-EXPECTED-FIELD", "expected event detail field")?;
                let subject = SubjectRef::Event(EventRef::Detail {
                    field_id: field.clone(),
                });
                match event_detail_field_type(&field) {
                    ValueType::Int => Ok(self.add_node(
                        ValueType::Int,
                        Op::ReadInt(ReadField {
                            subject,
                            field_id: field,
                        }),
                    )),
                    _ => Ok(self.add_node(
                        ValueType::Text,
                        Op::ReadText(ReadField {
                            subject,
                            field_id: field,
                        }),
                    )),
                }
            }
            other => Err(ParseFailure::single(
                "MF-DSL-UNKNOWN-EVENT-BINDING",
                format!("unknown event binding `$event.{other}`"),
                self.slice_span_for_name(other),
            )),
        }
    }

    fn parse_subject(&mut self) -> Result<SubjectRef, ParseFailure> {
        let token = self.next();
        let TokenKind::Ident(ident) = token.kind else {
            return Err(ParseFailure::single(
                "MF-DSL-EXPECTED-SUBJECT",
                "expected subject identifier".to_string(),
                token.span,
            ));
        };
        let base = subject_from_ident(&ident).map_err(|code| {
            ParseFailure::single(code, format!("unknown subject `{ident}`"), token.span)
        })?;
        // Chained relation accessor: `actor.wielded_item.<method>`. Re-root only
        // when a closed relation token sits between the subject and a further
        // `.` — so a bare `actor.wielded_item` is never itself a subject. The dot
        // before the real method/field is left for the caller to consume. Closed
        // K-5 vocab (ADR-0021 D-2); the eval side stays equality-filtered.
        if matches!(self.peek().kind, TokenKind::Dot) {
            let relation = match &self.peek_at(1).kind {
                TokenKind::Ident(rel) => Some(rel.clone()),
                _ => None,
            };
            if let Some(relation) = relation {
                if base == SubjectRef::Actor && relation == "equipped_item" {
                    self.next(); // consume `.`
                    self.next(); // consume relation name
                    return self.parse_equipped_item_subject();
                }
                if let Some(resubject) = relation_resubject(&base, &relation) {
                    if matches!(self.peek_at(2).kind, TokenKind::Dot) {
                        self.next(); // consume `.`
                        self.next(); // consume the relation token
                        return Ok(resubject);
                    }
                }
            }
        }
        Ok(base)
    }

    fn parse_equipped_item_subject(&mut self) -> Result<SubjectRef, ParseFailure> {
        self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
        let span = self.peek().span;
        let slot_id = self.expect_string(
            "MF-DSL-EQUIPMENT-SLOT-LITERAL",
            "expected a nonempty literal equipment slot id",
        )?;
        if slot_id.trim().is_empty() {
            return Err(ParseFailure::single(
                "MF-DSL-EQUIPMENT-SLOT-LITERAL",
                "equipment slot id must not be empty".to_string(),
                span,
            ));
        }
        self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
        Ok(SubjectRef::ActorEquippedItem { slot_id })
    }

    /// Parse a text-id argument shared by `roll_table(...)` and (sk-84i.2)
    /// `<subject>.source(...)` into a **text-valued node** (Option C, G3 /
    /// #1254): a quoted literal (`roll_table('lit')`,
    /// `actor.wielded_item.source('lit')`)
    /// → a `TextLiteral` node (the static-`MF-VAL-211`-validatable case), or
    /// `<subject>.text('state_id')` → a `ReadText` node that resolves a text
    /// `object_state` at runtime (e.g.
    /// `roll_table(actor.wielded_item.text('srd/damage_profile'))` reads the
    /// wielded weapon's authored profile pointer). Reuses G1's chained
    /// `parse_subject`, so `actor.wielded_item.*` re-roots correctly. Named
    /// generically (not `parse_roll_table_id_node`) because sk-84i.2 gave it
    /// a second caller with no roll-table meaning at all.
    fn parse_text_id_or_read_node(&mut self) -> Result<NodeId, ParseFailure> {
        if matches!(self.peek().kind, TokenKind::StringLit(_)) {
            let value = self.expect_string("MF-DSL-EXPECTED-STRING", "expected a quoted id")?;
            return Ok(self.add_node(ValueType::Text, Op::TextLiteral(TextLiteral { value })));
        }
        let subject = self.parse_subject()?;
        self.expect_simple(TokenKind::Dot, "MF-DSL-EXPECTED-DOT", "expected `.`")?;
        let method = self.expect_ident("MF-DSL-EXPECTED-METHOD", "expected `text`")?;
        if method != "text" {
            return Err(ParseFailure::single(
                "MF-DSL-EXPECTED-TEXT-READ",
                format!(
                    "expected a quoted id or a `<subject>.text('id')` \
                     read, got `.{method}`"
                ),
                self.slice_span_for_name(&method),
            ));
        }
        self.expect_simple(TokenKind::LParen, "MF-DSL-EXPECTED-LPAREN", "expected `(`")?;
        let field_id = self.expect_string("MF-DSL-EXPECTED-STRING", "expected quoted state id")?;
        self.expect_simple(TokenKind::RParen, "MF-DSL-EXPECTED-RPAREN", "expected `)`")?;
        Ok(self.add_node(
            ValueType::Text,
            Op::ReadText(ReadField { subject, field_id }),
        ))
    }

    fn add_node(&mut self, value_type: ValueType, op: Op) -> NodeId {
        let node_id = self.next_node_id;
        self.next_node_id += 1;
        self.nodes.push(Node {
            node_id,
            value_type,
            op,
            span: self.last_consumed_span(),
        });
        node_id
    }

    fn last_consumed_span(&self) -> SourceSpan {
        if self.pos == 0 {
            return SourceSpan::default();
        }
        self.tokens[self.pos - 1].span
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or(self.tokens.last().unwrap())
    }

    /// Lookahead `offset` tokens past the cursor (saturating at EOF). Used by
    /// chained-subject parsing to recognise `actor.wielded_item.<field>` without
    /// consuming, before committing to the re-rooting.
    fn peek_at(&self, offset: usize) -> &Token {
        self.tokens
            .get(self.pos + offset)
            .unwrap_or(self.tokens.last().unwrap())
    }

    fn next(&mut self) -> Token {
        let token = self
            .tokens
            .get(self.pos)
            .cloned()
            .unwrap_or_else(|| self.tokens.last().unwrap().clone());
        self.pos += 1;
        token
    }

    fn peek_ident(&self, expected: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Ident(actual) if actual == expected)
    }

    fn peek_subject(&self) -> bool {
        matches!(&self.peek().kind, TokenKind::Ident(actual) if matches!(actual.as_str(), "actor" | "target" | "location" | "session" | "world"))
    }

    fn expect_simple(
        &mut self,
        expected: TokenKind,
        code: &'static str,
        message: &'static str,
    ) -> Result<(), ParseFailure> {
        let token = self.next();
        if token.kind == expected {
            Ok(())
        } else {
            Err(ParseFailure::single(code, message.to_string(), token.span))
        }
    }

    fn expect_ident(
        &mut self,
        code: &'static str,
        message: &'static str,
    ) -> Result<String, ParseFailure> {
        let token = self.next();
        if let TokenKind::Ident(value) = token.kind {
            Ok(value)
        } else {
            Err(ParseFailure::single(code, message.to_string(), token.span))
        }
    }

    fn expect_string(
        &mut self,
        code: &'static str,
        message: &'static str,
    ) -> Result<String, ParseFailure> {
        let token = self.next();
        if let TokenKind::StringLit(value) = token.kind {
            Ok(value)
        } else {
            Err(ParseFailure::single(code, message.to_string(), token.span))
        }
    }

    /// A bare literal integer argument (`peers_near_target`'s `radius_mm`) —
    /// distinct from `parse_int_expr`'s full int-expression grammar, which
    /// would also accept reads/arithmetic/rolls where this leaf wants a
    /// plain authored constant, same discipline as `expect_string` for a
    /// literal string argument.
    fn expect_int_literal(
        &mut self,
        code: &'static str,
        message: &'static str,
    ) -> Result<i64, ParseFailure> {
        let token = self.next();
        if let TokenKind::Number(value) = token.kind {
            Ok(value)
        } else {
            Err(ParseFailure::single(code, message.to_string(), token.span))
        }
    }

    fn expect_quoted_string(
        &mut self,
        code: &'static str,
        message: &'static str,
    ) -> Result<String, ParseFailure> {
        match self.peek().kind {
            TokenKind::StringLit(_) => self.expect_string(code, message),
            _ => Err(ParseFailure::single(
                code,
                message.to_string(),
                self.peek().span,
            )),
        }
    }

    fn expect_string_or_binding_path(
        &mut self,
        code: &'static str,
        message: &'static str,
    ) -> Result<String, ParseFailure> {
        if matches!(self.peek().kind, TokenKind::Dollar) {
            let dollar = self.next();
            let scope = self.expect_ident(code, message)?;
            if scope != "bindings" {
                return Err(ParseFailure::single(
                    "MF-DSL-UNKNOWN-BINDING-SCOPE",
                    format!("unknown binding scope `{scope}`"),
                    dollar.span,
                ));
            }
            self.expect_simple(TokenKind::Dot, "MF-DSL-EXPECTED-DOT", "expected `.`")?;
            let name = self.expect_ident("MF-DSL-EXPECTED-BINDING", "expected binding name")?;
            return Ok(format!("${scope}.{name}"));
        }
        self.expect_string(code, message)
    }

    fn slice_span_for_name(&self, name: &str) -> SourceSpan {
        if let Some(start) = self.source.find(name) {
            return SourceSpan {
                start,
                end: start + name.len(),
            };
        }
        self.peek().span
    }
}

/// Parse a `dN` die identifier into its side count. Returns `None` unless
/// `ident` is exactly `d` followed by one or more ASCII digits naming a
/// non-zero side count (`d20` → `Some(20)`, `d0`/`hp`/`d2O` → `None`).
/// The single source of truth for dice-token recognition — both the standalone
/// dice atom and `roll_under` route through it, so the grammar cannot drift
/// (CR-17 / LR-17).
fn dice_sides_from_ident(ident: &str) -> Option<u32> {
    let rest = ident.strip_prefix('d')?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let sides: u32 = rest.parse().ok()?;
    (sides != 0).then_some(sides)
}

fn subject_from_ident(ident: &str) -> Result<SubjectRef, &'static str> {
    match ident {
        "actor" => Ok(SubjectRef::Actor),
        "target" => Ok(SubjectRef::Target),
        "location" => Ok(SubjectRef::Location),
        "session" => Ok(SubjectRef::Session),
        "world" => Ok(SubjectRef::World),
        _ => Err("MF-DSL-UNKNOWN-SUBJECT"),
    }
}

/// Re-roots a base subject through a chained **relation accessor** — the
/// `actor.wielded_item.<field>` surface. The relation-accessor vocabulary is a
/// CLOSED, §17 K-5-gated set (ADR-0021 D-2): only authored relation names
/// re-root the subject, and an open `subject.*.*` path grammar is deliberately
/// NOT offered. Returns `None` when `relation` is not a relation accessor for
/// `base` (the token is then an ordinary field/method name). The single source
/// of truth for chained-subject re-rooting — both parse paths route through it
/// so the int-expr and bool-method grammars cannot drift.
fn relation_resubject(base: &SubjectRef, relation: &str) -> Option<SubjectRef> {
    match (base, relation) {
        (SubjectRef::Actor, "wielded_item") => Some(SubjectRef::ActorWieldedItem),
        _ => None,
    }
}

/// One declared `$event.detail.<field>` field, scoped to the `EventKind` path
/// (server `event_kind_path` string, e.g. `"Layer2.ResourceDelta"`) whose
/// detail row carries it.
///
/// Used by the parser to type fields and by validation to check each hook
/// against the fields available for its event kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventDetailField {
    pub event_kind_path: &'static str,
    pub field_id: &'static str,
    pub value_type: ValueType,
}

/// The declared table. Never a widened hardcoded match, never a prefix rule
/// §Classification rule) — every row names
/// one authored field id verbatim.
pub const EVENT_DETAIL_FIELDS: &[EventDetailField] = &[
    // `Layer2.ApplyConsequence` — `event_apply_consequence`'s own detail row,
    // plus the `event_actor_body_swapped` fallback shape
    // `condition_hook_event_context` reads when no `ApplyConsequence` detail
    // row exists for the event id.
    EventDetailField {
        event_kind_path: "Layer2.ApplyConsequence",
        field_id: "template_id",
        value_type: ValueType::Text,
    },
    // Legacy binding, kept typed for back-compat: `item_id` is no longer
    // pushed by the producer (post-§11.18 items are entities; see
    // `condition_hook_event_context`'s doc comment), but old authored
    // predicates and the `dsl_reference()`/eval-gate corpus still reference
    // it, so it stays declared rather than silently re-typing as `Text`.
    EventDetailField {
        event_kind_path: "Layer2.ApplyConsequence",
        field_id: "item_id",
        value_type: ValueType::Int,
    },
    EventDetailField {
        event_kind_path: "Layer2.ApplyConsequence",
        field_id: "target_entity_id",
        value_type: ValueType::Int,
    },
    EventDetailField {
        event_kind_path: "Layer2.ApplyConsequence",
        field_id: "actor_id",
        value_type: ValueType::Int,
    },
    EventDetailField {
        event_kind_path: "Layer2.ApplyConsequence",
        field_id: "previous_body_entity_id",
        value_type: ValueType::Int,
    },
    EventDetailField {
        event_kind_path: "Layer2.ApplyConsequence",
        field_id: "new_body_entity_id",
        value_type: ValueType::Int,
    },
    // `Layer2.ResourceDelta` (#1332 event). K-B Part A1 fix: these fields were
    // already pushed by the producer but had no per-event-kind admission row
    // (validator rejected every `$event.detail.*` reference on this kind) and
    // no Int typing (parser defaulted every one of them to `Text`).
    EventDetailField {
        event_kind_path: "Layer2.ResourceDelta",
        field_id: "template_id",
        value_type: ValueType::Text,
    },
    EventDetailField {
        event_kind_path: "Layer2.ResourceDelta",
        field_id: "resource_id",
        value_type: ValueType::Text,
    },
    EventDetailField {
        event_kind_path: "Layer2.ResourceDelta",
        field_id: "entity_id",
        value_type: ValueType::Int,
    },
    EventDetailField {
        event_kind_path: "Layer2.ResourceDelta",
        field_id: "delta",
        value_type: ValueType::Int,
    },
    EventDetailField {
        event_kind_path: "Layer2.ResourceDelta",
        field_id: "new_value",
        value_type: ValueType::Int,
    },
    // `Layer2.ConditionDelta` (K-B Part B, #1332-precedent sibling event).
    // `applied` rides as Int 0/1 — `$event.detail.<field>` has no `ReadBool`
    // path (see `event_detail_field_type` below), so a native bool cannot be
    // exposed without new parser surface.
    EventDetailField {
        event_kind_path: "Layer2.ConditionDelta",
        field_id: "condition_definition_id",
        value_type: ValueType::Text,
    },
    EventDetailField {
        event_kind_path: "Layer2.ConditionDelta",
        field_id: "template_id",
        value_type: ValueType::Text,
    },
    EventDetailField {
        event_kind_path: "Layer2.ConditionDelta",
        field_id: "entity_id",
        value_type: ValueType::Int,
    },
    EventDetailField {
        event_kind_path: "Layer2.ConditionDelta",
        field_id: "applied",
        value_type: ValueType::Int,
    },
];

/// All fields declared for one `event_kind` path (e.g. `"Layer2.ResourceDelta"`).
/// Empty for an event kind with no declared detail fields.
pub fn event_detail_fields_for_kind(event_kind_path: &str) -> Vec<&'static EventDetailField> {
    EVENT_DETAIL_FIELDS
        .iter()
        .filter(|field| field.event_kind_path == event_kind_path)
        .collect()
}

/// Parser-time type for `$event.detail.<field>` — a flattened projection of
/// [`EVENT_DETAIL_FIELDS`] (parsing a DSL string has no `event_kind` context;
/// see the struct doc above). A field absent from the table defaults to
/// `Text`, matching every current cross-cutting reaction-trigger detail field
/// (`on_exit`, `phase`, `stochastic`, `speech_act`, …) which is Text-only and
/// never declared here — those stay untouched by K-B Part A1.
fn event_detail_field_type(field_id: &str) -> ValueType {
    EVENT_DETAIL_FIELDS
        .iter()
        .find(|field| field.field_id == field_id)
        .map(|field| field.value_type)
        .unwrap_or(ValueType::Text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn subject() -> SubjectRef {
        SubjectRef::Actor
    }

    /// K-B Part A1: `event_detail_field_type` flattens [`EVENT_DETAIL_FIELDS`]
    /// across `event_kind`s because parsing has no per-hook `event_kind`
    /// context. That flattening is only sound if no `field_id` is declared
    /// with two different types under different event kinds — pin it so a
    /// future row addition that violates this fails loudly instead of
    /// silently picking whichever row happens to iterate first.
    #[test]
    fn event_detail_fields_agree_on_type_per_field_id() {
        let mut seen: std::collections::BTreeMap<&str, ValueType> =
            std::collections::BTreeMap::new();
        for field in EVENT_DETAIL_FIELDS {
            if let Some(existing) = seen.insert(field.field_id, field.value_type) {
                assert_eq!(
                    existing, field.value_type,
                    "field id '{}' is declared with two different types across event kinds \
                     ('{:?}' vs '{:?}') — the parser's flattened lookup cannot resolve this",
                    field.field_id, existing, field.value_type
                );
            }
        }
    }

    /// `event_detail_field_type` must resolve every declared field to its
    /// table-declared type (not the `Text` default), and any undeclared
    /// legacy/cross-cutting field (reaction-trigger `on_exit` etc.) must keep
    /// falling back to `Text` — this is the parser-facing surface of the A1
    /// fix, exercised independent of the validator/producer consumers.
    #[test]
    fn event_detail_field_type_matches_declared_table() {
        for field in EVENT_DETAIL_FIELDS {
            assert_eq!(
                event_detail_field_type(field.field_id),
                field.value_type,
                "field '{}' resolved to the wrong type",
                field.field_id
            );
        }
        assert_eq!(event_detail_field_type("on_exit"), ValueType::Text);
        assert_eq!(
            event_detail_field_type("totally_undeclared"),
            ValueType::Text
        );
    }

    fn representative_ops() -> Vec<Op> {
        vec![
            Op::BoolLiteral(BoolLiteral { value: true }),
            Op::IntLiteral(IntLiteral { value: 1 }),
            Op::TextLiteral(TextLiteral {
                value: "text".to_owned(),
            }),
            Op::ReadInt(ReadField {
                subject: subject(),
                field_id: "core/int".to_owned(),
            }),
            Op::ReadEffectiveInt(ReadField {
                subject: subject(),
                field_id: "core/int".to_owned(),
            }),
            Op::ReadText(ReadField {
                subject: subject(),
                field_id: "core/text".to_owned(),
            }),
            Op::ReadBool(ReadField {
                subject: subject(),
                field_id: "core/bool".to_owned(),
            }),
            Op::ReadResourceMax(ReadField {
                subject: subject(),
                field_id: "core/hp".to_owned(),
            }),
            Op::ReadResourceMin(ReadField {
                subject: subject(),
                field_id: "core/hp".to_owned(),
            }),
            Op::HasCapability(HasCapability {
                subject: subject(),
                capability_id: "core/capability".to_owned(),
            }),
            Op::HasItem(HasItem {
                subject: subject(),
                item_def_id: "core/item".to_owned(),
            }),
            Op::HasNiche(HasNiche {
                subject: subject(),
                niche_id: "core/niche".to_owned(),
            }),
            Op::HasTag(HasTag {
                subject: subject(),
                tag: "tag".to_owned(),
            }),
            Op::SharesTag(SharesTag {
                subject_a: subject(),
                subject_b: SubjectRef::Target,
                prefix: "actor.language.".to_owned(),
            }),
            Op::HasStatus(HasStatus {
                subject: subject(),
                condition_definition_id: "core/status".to_owned(),
            }),
            Op::ImpliedByCount(ImpliedByCount {
                condition_definition_id: Some("core/status".to_owned()),
            }),
            Op::ParticipantTagGroupCount(ParticipantTagGroupCount {
                prefix: "side:".to_owned(),
                excluding_condition_definition_id: Some("core/status".to_owned()),
            }),
            Op::PeersNearTarget(PeersNearTarget {
                prefix: "srd.faction.".to_owned(),
                radius_mm: 1524,
                excluding_condition_definition_id: Some("srd/condition/incapacitated".to_owned()),
            }),
            Op::OccupantsAt(OccupantsAt {
                scope: "to_node".to_owned(),
                excluding_condition_definition_id: Some("srd/condition/dead".to_owned()),
            }),
            Op::ChannelPool(ChannelPool {
                channel_id: "core/action_class/primary".to_owned(),
            }),
            Op::TracePresent(TracePresent {
                subject: subject(),
                trace_kind_id: "core/trace".to_owned(),
            }),
            Op::KnowsEntity(KnowsEntity {
                subject: subject(),
                entity_id: "$bindings.entity".to_owned(),
            }),
            Op::And(Operands {
                operands: vec![1, 2],
            }),
            Op::Or(Operands {
                operands: vec![1, 2],
            }),
            Op::Not(UnaryNode { operand: 1 }),
            Op::Compare(Compare {
                op: CompareOp::Eq,
                left: 1,
                right: 2,
            }),
            Op::Arithmetic(Arithmetic {
                op: ArithmeticOp::Add,
                left: 1,
                right: 2,
            }),
            Op::Cond(Cond {
                cond: 1,
                then_node: 2,
                else_node: 3,
            }),
            Op::Roll(Roll {
                dice: Dice {
                    count: 1,
                    sides: 20,
                },
            }),
            Op::RollUnder(RollUnder {
                roll_node: 1,
                target_node: 2,
            }),
            Op::RollTable(RollTable {
                table_id_node: 1,
                draw_node: 1,
            }),
            Op::DslSource(DslSource {
                subject: subject(),
                slot_node: 1,
            }),
            Op::Let(Let {
                name: "x".to_owned(),
                value: 1,
                body: 2,
            }),
            Op::OutcomeSequence(OutcomeSequence { steps: vec![1, 2] }),
            Op::InvokeOutcomeGraph(InvokeOutcomeGraph {
                outcome_graph_id: "core/graph".to_owned(),
            }),
            Op::ModifyResource(ModifyResource {
                subject: subject(),
                resource_definition_id: "core/hp".to_owned(),
                delta_node: 1,
            }),
            Op::ApplyStatus(ApplyStatus {
                subject: subject(),
                condition_definition_id: "core/status".to_owned(),
            }),
            Op::EmitTrace(EmitTrace {
                subject: subject(),
                trace_kind_id: "core/trace".to_owned(),
            }),
            Op::Arbitrate(Arbitrate {
                request_shape_id: "core/request".to_owned(),
                context_nodes: vec![1],
                allowed_outcome_nodes: vec![2],
            }),
        ]
    }

    #[test]
    fn dsl_reference_describes_every_op_variant() {
        use strum::EnumCount;

        let reference_names = dsl_reference()
            .operators
            .into_iter()
            .map(|op| op.operator)
            .collect::<BTreeSet<_>>();
        let op_names = representative_ops()
            .iter()
            .map(Op::variant_name)
            .collect::<BTreeSet<_>>();

        // Anchor to the compiler: `Op::COUNT` tracks the enum, so a newly added
        // variant fails here until `representative_ops()` constructs it — closing
        // the "added a variant but forgot the reference everywhere" gap that two
        // hand-maintained lists alone cannot catch.
        assert_eq!(
            op_names.len(),
            Op::COUNT,
            "representative_ops() must cover every Op variant"
        );

        // Every represented variant has a reference entry and vice versa; with
        // the count anchor above this proves every Op variant is described.
        assert_eq!(
            reference_names, op_names,
            "dsl_reference must describe every Op variant exactly once"
        );
    }

    #[test]
    fn shares_tag_parses_to_two_subject_prefix_op() {
        let expr = parse_predicate(r#"shares_tag(actor, target, "actor.language.")"#)
            .expect("shares_tag should parse as a free-function predicate");
        let root = expr.node(expr.root_node_id).expect("root node exists");
        assert!(
            matches!(
                &root.op,
                Op::SharesTag(SharesTag {
                    subject_a: SubjectRef::Actor,
                    subject_b: SubjectRef::Target,
                    prefix,
                }) if prefix == "actor.language."
            ),
            "got {:?}",
            root.op
        );
    }

    #[test]
    fn shares_tag_composes_inside_a_bool_chain() {
        // The authored communication form: an actor-side capability gate AND the
        // two-subject shared-language gate.
        let expr = parse_predicate(
            r#"actor.has_capability("core/communicate") && shares_tag(actor, target, "actor.language.")"#,
        )
        .expect("shares_tag should compose inside an && chain");
        assert!(expr
            .nodes
            .iter()
            .any(|node| matches!(&node.op, Op::SharesTag(_))));
    }

    #[test]
    fn binding_structured_stat_parses_to_binding_subject_read_int() {
        let expr = parse_int_expression(r#"bindings.party.stat("phandelver/escort_stage")"#)
            .expect("binding stat should parse");
        let root = expr.node(expr.root_node_id).expect("root node exists");
        assert!(matches!(
            &root.op,
            Op::ReadInt(ReadField {
                subject: SubjectRef::Binding(BindingRef { name }),
                field_id,
            }) if name == "party" && field_id == "phandelver/escort_stage"
        ));
        assert_eq!(root.op.binding_subject_name(), Some("party"));
    }

    #[test]
    fn binding_structured_stat_parses_inside_bool_chain() {
        let expr = parse_predicate(
            r#"actor.has_capability("core/walk") && target.has_niche("core/passable_portal") && bindings.party.stat("phandelver/escort_stage") >= 2"#,
        )
        .expect("binding stat comparison should parse in predicate chain");
        assert!(expr.nodes.iter().any(|node| matches!(
            &node.op,
            Op::ReadInt(ReadField {
                subject: SubjectRef::Binding(BindingRef { name }),
                field_id,
            }) if name == "party" && field_id == "phandelver/escort_stage"
        )));
    }

    #[test]
    fn bare_identifier_binding_value_parse_is_unchanged() {
        let expr = parse_int_expression("party").expect("bare binding should parse");
        let root = expr.node(expr.root_node_id).expect("root node exists");
        assert!(matches!(
            &root.op,
            Op::ReadInt(ReadField {
                subject: SubjectRef::Binding(BindingRef { name }),
                field_id,
            }) if name == "party" && field_id == "value"
        ));
    }

    #[test]
    fn parse_outcome_supports_invoke_outcome_graph() {
        let expr = parse_outcome("invoke_outcome_graph('core/open_container')")
            .expect("invoke outcome graph should parse");
        let root = expr.node(expr.root_node_id).expect("root node exists");
        assert!(matches!(
            &root.op,
            Op::InvokeOutcomeGraph(InvokeOutcomeGraph { outcome_graph_id })
                if outcome_graph_id == "core/open_container"
        ));
    }

    #[test]
    fn parse_outcome_supports_arbitrate() {
        let expr = parse_outcome(
            "arbitrate('core/dialogue_choice', 'visible_target=guard', 'talk_down=core/talk_down_guard')",
        )
        .expect("arbitrate should parse");
        let root = expr.node(expr.root_node_id).expect("root node exists");
        let Op::Arbitrate(op) = &root.op else {
            panic!("expected arbitrate root, got {:?}", root.op);
        };
        assert_eq!(op.request_shape_id, "core/dialogue_choice");
        assert_eq!(op.context_nodes.len(), 1);
        assert_eq!(op.allowed_outcome_nodes.len(), 1);
    }

    // --- G1 (#weapon-sourced-attacks Phase 2): the `actor.wielded_item.*`
    // chained-subject surface. §17 K-5 falsifiers for the new `SubjectRef`
    // variant: the chained surface re-roots to `ActorWieldedItem`, the relation
    // vocab is closed (only `actor` has `wielded_item`), and a bare relation
    // accessor is rejected. Eval stays equality-filtered, so no eval arm — the
    // missing-fact path is the existing `has_id`/`ReadInt`-UnknownField semantics.

    #[test]
    fn wielded_item_stat_parses_to_wielded_subject_read_int() {
        let expr = parse_int_expression(r#"actor.wielded_item.stat("srd/reach")"#)
            .expect("wielded-item stat should parse");
        let root = expr.node(expr.root_node_id).expect("root node exists");
        assert!(
            matches!(
                &root.op,
                Op::ReadInt(ReadField {
                    subject: SubjectRef::ActorWieldedItem,
                    field_id,
                }) if field_id == "srd/reach"
            ),
            "expected ReadInt(ActorWieldedItem, 'srd/reach'), got {:?}",
            root.op
        );
    }

    #[test]
    fn wielded_item_stat_parses_inside_bool_comparison() {
        // Routes through the bool-method path's backtrack into parse_int_expr.
        let expr = parse_predicate(r#"actor.wielded_item.stat("srd/reach") >= 1"#)
            .expect("wielded-item stat comparison should parse");
        assert!(expr.nodes.iter().any(|node| matches!(
            &node.op,
            Op::ReadInt(ReadField {
                subject: SubjectRef::ActorWieldedItem,
                field_id,
            }) if field_id == "srd/reach"
        )));
    }

    #[test]
    fn wielded_item_has_tag_parses_to_wielded_subject() {
        let expr = parse_predicate(r#"actor.wielded_item.has_tag("srd/tag/melee")"#)
            .expect("wielded-item has_tag should parse");
        assert!(expr.nodes.iter().any(|node| matches!(
            &node.op,
            Op::HasTag(HasTag {
                subject: SubjectRef::ActorWieldedItem,
                tag,
            }) if tag == "srd/tag/melee"
        )));
    }

    #[test]
    fn bare_wielded_item_relation_accessor_is_rejected() {
        let err = parse_int_expression("actor.wielded_item")
            .expect_err("a bare relation accessor must be rejected, not read as an Actor field");
        assert!(
            err.diagnostics
                .iter()
                .any(|d| d.code == "MF-DSL-DANGLING-RELATION"),
            "expected MF-DSL-DANGLING-RELATION, got {:?}",
            err.diagnostics
        );
    }

    #[test]
    fn wielded_item_relation_is_closed_to_actor_only() {
        // The relation vocab is closed: only `actor` has `wielded_item`. A
        // `target.wielded_item.*` surface must NOT silently re-root — it errors.
        assert!(
            parse_predicate(r#"target.wielded_item.stat("srd/reach") >= 1"#).is_err(),
            "target has no `wielded_item` relation accessor; it must not re-root"
        );
    }
}
