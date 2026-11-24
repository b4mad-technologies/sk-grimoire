//! Bootstrap vocabularies shared by the server and offline validation.

macro_rules! seed_vocabulary {
    ($seeds:ident: $row:ty, $ids:ident; $(($id:literal, $($field:literal),+ $(,)?)),+ $(,)?) => {
        pub const $seeds: &[$row] = &[$(($id, $($field),+)),+];
        pub const $ids: &[&str] = &[$($id),+];
    };
}

// Scope id and description, in bootstrap order.
seed_vocabulary! {
    CORE_SCOPE_SEEDS: (&str, &str), CORE_SCOPE_IDS;
    ("place_node", "scope: a place_node row"),
    ("place_edge", "scope: a place_edge row"),
    ("place_graph", "scope: a place_graph_definition row"),
    ("trace", "scope: an ambient_traces entry"),
    // ADR-0095 D-1: an `entity` row as a trace anchor — admission is
    // co-location with the anchor's CURRENT position (seams A + B), not
    // addressee-keyed delivery.
    ("entity", "scope: an entity row"),
    ("any", "scope: applies to any addressable row"),
}

// Trace kind id, location kinds, payload schema and description, in seed order.
seed_vocabulary! {
    CORE_TRACE_KIND_SEEDS: (&str, &str, &str, &str), CORE_TRACE_KIND_IDS;
    (
        "scrape_marks",
        "place_node,place_edge",
        "",
        "residual marks left by physical or narrative impact",
    ),
    (
        "scent",
        "place_node,place_edge",
        "",
        "olfactory residue carried through nearby spaces",
    ),
    (
        "smoke",
        "place_node,place_edge",
        "",
        "smoke haze suitable for visibility and tracing fixtures",
    ),
    (
        "handled_item",
        "place_node",
        r#"{"action":"string"}"#,
        "runtime memory that an actor handled an item at a place",
    ),
}

/// The sole effect primitive admitted for ambient autonomy (ADR-0004 D-2).
pub const AUTONOMY_ELIGIBLE_PRIMITIVES: &[&str] = &["InscribeTraceCluster"];

/// Additional effect primitives that require a stored acting subject (ADR-0054).
pub const SUBJECT_BOUND_AUTONOMY_ACTOR_PRIMITIVES: &[&str] = &["Damage", "Heal", "ApplyModifier"];

/// The autonomy frame-write variant that contains a nested body.
pub const AUTONOMY_FRAME_WRITE_CONTAINER: &str = "ForEachActorInRadius";

/// Frame-write variants admitted in subject-bound hazard consequence templates.
pub const AUTONOMY_ELIGIBLE_FRAME_WRITES: &[&str] = &[
    "ApplyStatus",
    AUTONOMY_FRAME_WRITE_CONTAINER,
    "ModifyResource",
];

/// Whether a primitive is admitted for a job with the given subject mode.
pub fn autonomy_eligible_for_subject_mode(primitive: &str, has_subject: bool) -> bool {
    AUTONOMY_ELIGIBLE_PRIMITIVES.contains(&primitive)
        || (has_subject && SUBJECT_BOUND_AUTONOMY_ACTOR_PRIMITIVES.contains(&primitive))
}

/// Whether a frame-write variant is an actor-reaching leaf in an autonomy job.
pub fn is_autonomy_actor_frame_write(variant: &str) -> bool {
    variant != AUTONOMY_FRAME_WRITE_CONTAINER && AUTONOMY_ELIGIBLE_FRAME_WRITES.contains(&variant)
}
