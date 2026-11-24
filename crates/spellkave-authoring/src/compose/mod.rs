//! `clusterRows` — the Rust-owned counterpart of `@spellkave/world-author`'s
//! `compose.ts` (#1968 R7, ADR-0087, task section 4). Ported
//! incrementally, one `ActorBehavior` shape (or cross-cutting piece) per
//! slice (`task` 4.1-4.9); `mod.rs` is only the module wiring — no
//! manifest semantics live here directly until slice 4.10 assembles the full
//! dispatcher.
//!
//! Slice 4.1 lands first: `ids()` (namespace-aware id derivation) and the
//! shared live-wiring emission every shape uses regardless of its own
//! topology. This is deliberately the FIRST slice — `ids()`'s hand-mirror
//! (ADR-0087 Context) is the incident this migration exists to close, so it
//! is ported and proven alone before any shape-specific logic depends on it.

pub mod aoe;
pub mod clock;
pub mod cluster_rows;
pub mod ids;
pub mod live_wiring;
pub mod modifiers;
pub mod outcome_graph;
pub mod scheduled_zone;
pub mod single_target;
pub mod summon;
pub mod zone;
