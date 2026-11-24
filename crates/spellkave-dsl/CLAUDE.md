# spellkave-dsl — operator reference examples

`dsl_reference()` returns each operator's `examples`. These strings are surfaced to
authors as "this is how you write it", so
they must not drift from the actual grammar.

## Enforced now — the parse gate (ADR-0047 D-5)

`tests/reference_examples.rs` (`every_reference_operator_example_parses`) runs the
real parser over every `authoring: "expression"` operator example and fails CI on
any string the parser rejects. So a documented expression example **cannot** drift
into non-parsing syntax — edit an example to something the grammar doesn't accept
and the test goes red. (This guard is what caught the original ~16-example drift:
`read_int`→`stat`, `roll('1d20')`→`1d20`, `!x`→`x == false`, `let..in`→`let..;`,
infix `+`→`max(a,b)`, `event.detail('x')`→`$event.detail.x`.)

`authoring: "frame_write"` operators (`ModifyResource`, `ApplyStatus`, `EmitTrace`)
are AST nodes with **no parser surface**. Their live authoring form is a structured
`[[consequence_frame_writes]]` row (`EmitTrace` has no frame write of that name — the
trace write is `Inscribe`), so their examples are that row's `frame_write` TOML value
and are deliberately excluded from the parse gate. `OutcomeSequence`,
`InvokeOutcomeGraph` and `Arbitrate` DO parse as outcome DSL text
(`outcome_expression_source`) and are `expression` operators; their eval rows are
`NotEvaluatable` because outcome execution lives in the server
(`outcome_execution.rs`). The classification lives on
`DslOperatorReference::authoring`.

## Enforced now — the eval gate (sk-e0u.3, D5)

`tests/reference_examples_eval.rs` (`every_expression_example_evaluates_to_its_documented_result`)
runs every `authoring: "expression"` operator example through the SAME public
eval entry points production callers use — `parse_predicate`/`parse_int_expression`
+ `evaluate_predicate_expression`/`evaluate_int_expression` — against a
`documented_expectations()` table of `(operator, example) -> expected result`,
and fails CI naming the operator if the evaluated result doesn't match. Zero
new interpreter dispatch arms, no test-only DSL primitives (D5) — the harness
composes existing surface exactly like `eval.rs`'s own unit tests, plus a
handful of shared minimal `EvaluationContext` fixtures local to the test file.

A root-typed-`Text`/`Bool` example (`TextLiteral`, `ReadText`, `BoolLiteral`)
has no public entry point that returns that bare value — `evaluate_int_expression`
rejects a non-Int root, and `parse_predicate` requires a comparison/boolean
connective at the top level, so a bare literal atom doesn't parse as a
Predicate either. These are proven by composing the example into a synthetic
`<example> == <expected>` **Predicate** (real `Op::Compare` grammar) evaluated
through `evaluate_predicate_expression` — still zero new dispatch arms.

`every_documented_expectation_matches_a_live_reference_example` guards the
expectation table itself: a stale row (renamed operator, changed example text)
fails loudly instead of silently never being exercised.

Every predicate/int expression-authored example is eval-proven this way. The
three outcome operators (`OutcomeSequence`, `InvokeOutcomeGraph`, `Arbitrate`)
carry `DslExampleExpect::NotEvaluatable { reason }` rows: this crate has no
outcome evaluator. If another example genuinely cannot be evaluated from this
crate's own public surface, give it such a row rather than skipping it
silently; the gate reports these by name.

Two doc/impl observations surfaced while building the eval gate — both fixed
(sk-e0u.7):
- `BoolLiteral`'s `expression_kind` was documented as `"predicate"`, but the
  bare `true` example does not parse as a top-level Predicate — only via
  `parse_int_expression`'s generic literal-atom fallback (which is tagged
  `ExpressionKind::Outcome`, same as `IntLiteral`/`TextLiteral`). It composes
  into a real Predicate parse only wrapped in a comparison (`true == true`),
  mirroring how `IntLiteral`/`TextLiteral` compose as Compare operands. Fixed
  by relabeling it `"predicate|outcome"`, matching those siblings' existing
  convention, plus a note on how the bare literal actually parses.
- `RollTable`'s reference entry declared `returns: Outcome`, but the only
  parser call site that constructs `Op::RollTable` (the int-atom grammar)
  always produces `ValueType::Int`, and `eval.rs` has no `evaluate_outcome_*`
  entry point or `RuntimeValue::Outcome` variant at all — an Outcome-returning
  `roll_table` form does not exist (the `ValueType::Outcome` + `RollTable`
  combination in `eval.rs` is a hand-built AST fixture proving a defensive
  `UnsupportedInPredicate` guard, not a reachable authoring form). Fixed by
  correcting `returns` to `ValueType::Int`.

## Precedence footgun — `||` binds tighter than `&&`

**Operator precedence is inverted from C:** `||` binds TIGHTER than `&&`
(`parse_and` is built over `parse_or`). So `A && B && C || D` parses as
`A && B && (C || D)`. Avoid the flattened repeated-guard idiom
(`act && niche && C || act && niche && D`) — it mis-parses. Boolean terms cannot
be parenthesised either (`(` groups int expressions only); proof:
`tests/corpus.rs::or_binds_tighter_than_and`. Keep-or-flip is open decision
`sk-jay0.7.105.10`.

When you add or change an operator: keep its example in real parser surface (the
parse gate enforces this for `expression` ops), give it a `documented_expectations()`
row in `tests/reference_examples_eval.rs` (the eval gate fails loudly on a missing
row). Rebuild the validator Wasm when `dsl_reference()` changes.

## Public tokenize (ADR-0131 D-2, sk-jay0.7.105.1)

`tokenize()` is the tooling surface: kind plus byte range, covering the input
exhaustively so concatenating slices reproduces the source. The parser lexer
stays private and still skips whitespace. `tokenize()` wraps it and inserts
`TokenKind::Whitespace` for the gaps. There is no comment syntax; do not add one
here.

**No `Dice` token kind.** `d20` / `2d6` remain a parse recognition via
`try_parse_dice` (`Number` immediately followed by `Ident`, or a bare `Ident`
whose name is `d` + digits). A renderer may apply that same heuristic. A second
recognition path would drift from the compiler (CR-17/LR-17).

## Traced evaluation (ADR-0147, author rule bench)

`evaluate_expression_traced()` is the tooling and bench entry point for offline
rule evaluation (`spellkave_eval` / `sk eval`). It accepts an `Expression`,
`EvaluationContext`, `EvaluationRng` and `EvaluationLimits`, and returns
`Result<TracedEvaluation, TracedEvaluationError>`; the error retains the completed
trace entries alongside the underlying `EvaluationError`.

Key properties:
- **What it is for:** Offline authoring diagnostics, enabling tools to inspect
  node-by-node evaluation, operand values, condition branches, and stochastic
  draws without running a server node.
- **Completion order:** The trace records entries in post-order completion order:
  child operands evaluate and record their trace entries before the enclosing
  operator, with the root node recorded last. If an evaluation error occurs,
  the leaf node that raised the error is recorded first, followed by each
  ancestor propagating that error.
- **Untraced metadata:** Roll-table selection IDs and nested-source scope strings
  are constructed only when the trace sink is enabled. The
  `untraced_state_does_not_construct_trace_metadata` test guards the disabled
  state; ordinary stochastic provenance remains unchanged.
