use schemars::JsonSchema;
use semver::Version;
use serde::{de, Deserialize, Serialize};
use std::collections::BTreeMap;

/// The DSL's existing closed value-type vocabulary. Manifest macro signatures
/// deliberately reuse it rather than introducing a macro-specific type enum.
pub use spellkave_dsl::ValueType as DslValueType;

/// Default fiction milliseconds represented by one world tick.
///
/// The server seeds `Config.fiction_ms_per_tick` from this value and offline
/// manifest validation uses the same default calibration.
pub const DEFAULT_FICTION_MS_PER_TICK: u64 = 6_000;

/// Shared authored player-seat convention; person-driven role subjects use identities.
pub const PLAYER_SEAT_TAG: &str = "srd.player_character";

/// Marks a capability as a metadata audience for service identities only.
pub const SERVICE_ONLY_AUDIENCE_TAG: &str = "core.service_only_audience";

mod core_bootstrap;
pub use core_bootstrap::{CoreExtentKindSeed, CORE_BOOTSTRAP_MANIFEST, CORE_EXTENT_KINDS};
mod condition_extent;
pub use condition_extent::ConditionCapabilityExtentGrantRow;
mod containment_policy;
pub use containment_policy::{
    ContainmentPolicyDefinitionRow, EntityContainmentPolicyRow,
    EntityDefinitionContainmentPolicyRow,
};

mod wire_u64;
pub use wire_u64::{option_u64_flexible, u64_flexible, U64_FIELDS_BY_ROW_KIND};

/// An open, JSON-compatible value authored under a manifest's `[metadata]`
/// table. The deliberately small number range keeps a TOML → wasm JSON → JS
/// → TOML round trip exact: JavaScript cannot represent integers above
/// `2^53 - 1` without loss. TOML datetimes, `nan`/`inf`, and JSON `null` have
/// no lossless representation in that cross-runtime contract and are rejected
/// at the parsing boundary.
#[derive(Debug, Clone, PartialEq, JsonSchema)]
#[serde(untagged)]
pub enum MetadataValue {
    Bool(bool),
    Integer(#[schemars(range(min = -9007199254740991.0, max = 9007199254740991.0))] i64),
    Float(#[schemars(range(min = -9007199254740991.0, max = 9007199254740991.0))] f64),
    String(String),
    Array(Vec<MetadataValue>),
    Object(BTreeMap<String, MetadataValue>),
}

impl MetadataValue {
    pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    fn integer(value: i64) -> Result<Self, String> {
        if value.unsigned_abs() <= Self::MAX_SAFE_INTEGER as u64 {
            Ok(Self::Integer(value))
        } else {
            Err(format!(
                "metadata integers must be within JavaScript's safe range ±{}",
                Self::MAX_SAFE_INTEGER
            ))
        }
    }

    fn float(value: f64) -> Result<Self, String> {
        if value.is_finite() && value.abs() <= Self::MAX_SAFE_INTEGER as f64 {
            Ok(Self::Float(value))
        } else {
            Err(format!(
                "metadata floats must be finite and within JavaScript's safe range ±{}",
                Self::MAX_SAFE_INTEGER
            ))
        }
    }
}

impl Serialize for MetadataValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Integer(value) if value.unsigned_abs() <= Self::MAX_SAFE_INTEGER as u64 => {
                serializer.serialize_i64(*value)
            }
            Self::Integer(_) => Err(serde::ser::Error::custom(format!(
                "metadata integers must be within JavaScript's safe range ±{}",
                Self::MAX_SAFE_INTEGER
            ))),
            Self::Float(value)
                if value.is_finite() && value.abs() <= Self::MAX_SAFE_INTEGER as f64 =>
            {
                serializer.serialize_f64(*value)
            }
            Self::Float(_) => Err(serde::ser::Error::custom(format!(
                "metadata floats must be finite and within JavaScript's safe range ±{}",
                Self::MAX_SAFE_INTEGER
            ))),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(values) => values.serialize(serializer),
            Self::Object(values) => values.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for MetadataValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = MetadataValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a JSON-compatible metadata value (not null, datetime, or non-finite number)",
                )
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(MetadataValue::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                MetadataValue::integer(value).map_err(E::custom)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                i64::try_from(value)
                    .map_err(|_| E::custom("metadata integers must fit signed TOML i64"))
                    .and_then(|value| MetadataValue::integer(value).map_err(E::custom))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                MetadataValue::float(value).map_err(E::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(MetadataValue::String(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(MetadataValue::String(value))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    values.push(value);
                }
                Ok(MetadataValue::Array(values))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    values.insert(key, value);
                }
                Ok(MetadataValue::Object(values))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Err(E::custom("metadata does not support null"))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Err(E::custom("metadata does not support null"))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// Shared action-economy window rules.
pub mod action_economy_window;
pub mod export_round_trip;

pub mod row_identity;
pub mod row_kinds;
pub mod storage_key;
pub use row_kinds::{
    row_kind_composite_key_json, row_kind_natural_key_json, ROW_KIND_COMPOSITE_KEY,
    ROW_KIND_NATURAL_KEY,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringSignificance {
    Opaque,
    RuntimeSignificant,
}

pub fn classify_string_path(path: &str) -> StringSignificance {
    match path {
        "rules.predicate"
        | "rules.outcome"
        | "entity_definitions.slot_compatibility_predicate"
        | "item_definitions.slot_compatibility_predicate"
        | "equipment_slot_definitions.default_compatibility_predicate"
        | "modifier_templates.predicate_source"
        | "modifier_templates.modifier_expression_source"
        | "condition_definitions.gate_predicate_source"
        | "outcome_templates.outcome_expression_source"
        | "outcome_graph_edges.guard_predicate_source"
        | "condition_definitions.hooks.event_kind" => StringSignificance::RuntimeSignificant,
        _ => StringSignificance::Opaque,
    }
}

/// Independently identified manifest rows may carry a presentation description.
/// Outcome descriptions support a closed set of placeholders for visible names.
/// Description text carries no rule logic or authority (ADR-0018).
///
/// One documented, principled EXCLUDED class: pure junction/topology/
/// instruction rows whose entire identity is a composite of foreign keys
/// into other already-described rows (or a structural graph/topology
/// position), carrying no independent authored identity of their own. A
/// description on "actor X holds capability Y" or "graph node 3 of outcome
/// graph Z" would duplicate the description already available on the things
/// being joined. This class covers: every `*_tag` row (relational tag
/// membership), `metadata_attachments` (the generic attachment mechanism
/// itself — `metadata_namespaces` already carries its own optional
/// description), `place_edge_affordance`/`spatial_niche_attachments`
/// (attachment/binding rows), `entity_definition_initial_state` /
/// `_initial_capability` / `_initial_condition` (definition-seed rows),
/// `outcome_graph_nodes` / `outcome_graph_edges` / `consequence_frame_writes`
/// / `roll_table_entries` (graph/sequence topology positions),
/// `target_conditions` / `object_state` / `entity_resource_bound` /
/// `actor_capabilities` / `tag_capability_grant` (a tag × capability grant
/// rule, both legs already described) /
/// `actor_identities` / `actor_spatial_locations` /
/// `spatial_environment_state` / `movement_defaults`
/// (instance fact/grant records), `entity_tag` / `entity_containment` /
/// `actor_equipment_slots` (relational join rows),
/// `retractions` (a deletion tombstone, not authored content), and `include`
/// (a loader directive, resolved away before this struct is ever seen).
///
/// `#[schemars(title = "Manifest")]` pins the JSON Schema `title` to the type
/// name: schemars derives `title` from a struct's doc comment (its first
/// line) when one is present, and this doc comment is long — every consumer
/// of the generated schema (`describeManifestSchema()` in
/// `@spellkave/world-author`, the `manifest_schema_json` wasm export) expects
/// `title === "Manifest"`, not a doc-comment fragment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "Manifest")]
pub struct Manifest {
    /// Guarded subject replacement only; never an admission verdict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binding_preconditions: Vec<BindingPrecondition>,
    pub schema_version: SchemaVersion,
    /// ADR-0145 D-1: the applyset a ROOT manifest belongs to — the key of the
    /// server's applyset memory, named by the author (like kubectl's
    /// `--applyset`). Optional in the schema, required by every applyset apply
    /// (`sk dev seed|up`, `sk apply`); an included fragment must not set it
    /// (the include loaders reject that). The apply reducers ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applyset: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, MetadataValue>,
    #[serde(default)]
    pub metadata_namespaces: Vec<MetadataNamespaceRow>,
    #[serde(default)]
    pub metadata_attachments: Vec<MetadataRow>,
    #[serde(default)]
    pub bundles: Vec<BundleRow>,
    /// Bundle-owned exact tag-value vocabulary. Each row identifies one
    /// complete dotted tag under a declared ref-typed prefix; it carries no
    /// behavior or dispatch payload.
    #[serde(default)]
    pub tag_value_definitions: Vec<TagValueDefinitionRow>,
    #[serde(default)]
    pub place_graphs: Vec<PlaceGraphRow>,
    /// Prepared spatial regions protected from privileged writes (ADR-0151 D-9).
    /// Author seals on a graph whose root is owned by the same nonempty bundle.
    /// Instance seals are copied from their blueprint; instance graphs and
    /// world-owned roots cannot be sealed directly. `broken` and `break_reason`
    /// are hash-visible export-only observations, rejected in authoring input.
    #[serde(default)]
    pub place_seals: Vec<PlaceSealRow>,
    #[serde(default)]
    pub place_graph_instances: Vec<PlaceGraphInstanceRow>,
    #[serde(default)]
    pub place_nodes: Vec<PlaceNodeRow>,
    #[serde(default)]
    pub place_edges: Vec<PlaceEdgeRow>,
    #[serde(default)]
    pub map_layers: Vec<MapLayerRow>,
    #[serde(default)]
    pub place_edge_affordance: Vec<PlaceEdgeAffordanceRow>,
    /// ADR-0101 D-1: bundle- or place-graph-scoped default traversal-mode
    /// rows. Bundle-scoped clear-then-insert at commit (no natural key —
    /// same discipline as `spatial_environment_state`).
    #[serde(default)]
    pub movement_defaults: Vec<MovementDefaultRow>,
    #[serde(default)]
    pub substrate_definitions: Vec<SubstrateDefinitionRow>,
    #[serde(default)]
    pub coordinate_kind_definitions: Vec<CoordinateKindDefinitionRow>,
    #[serde(default)]
    pub anchor_kind_definitions: Vec<AnchorKindDefinitionRow>,
    #[serde(default)]
    pub extent_kind_definitions: Vec<ExtentKindDefinitionRow>,
    /// ADR-0134 D-1 — distance-metric vocabulary rows a `combat_grid_blueprint`
    /// names via `distance_metric_id`. Insert into `distance_metric_definition`.
    #[serde(default)]
    pub distance_metric_definitions: Vec<DistanceMetricDefinitionRow>,
    /// ADR-0142 D-4 — audience vocabulary rows a `prompt_definition` names
    /// via `audience_kind_ref`. Insert into `audience_kind_definition`.
    #[serde(default)]
    pub audience_kind_definitions: Vec<AudienceKindDefinitionRow>,
    /// ADR-0142 D-1 — dialogue prompt contract rows. Insert into
    /// `prompt_definition`.
    #[serde(default)]
    pub prompt_definitions: Vec<PromptDefinitionRow>,
    #[serde(default)]
    pub boundary_definitions: Vec<BoundaryDefinitionRow>,
    #[serde(default)]
    pub location_definitions: Vec<LocationDefinitionRow>,
    #[serde(default)]
    pub place_node_traces: Vec<PlaceNodeTraceRow>,
    #[serde(default)]
    pub location_traces: Vec<LocationTraceRow>,
    #[serde(default)]
    pub entity_definitions: Vec<EntityDefinitionRow>,
    #[serde(default)]
    pub entity_definition_tag: Vec<EntityDefinitionTagRow>,
    /// Definition-level initial state-values (#729). Each row seeds an
    /// `object_state` row onto every placed instance of `entity_def_id`.
    #[serde(default)]
    pub entity_definition_initial_state: Vec<EntityDefinitionInitialStateRow>,
    /// Definition-level innate capability grants (#728). Each row seeds a
    /// `capability` row (entity-anchored) onto every placed instance.
    #[serde(default)]
    pub entity_definition_initial_capability: Vec<EntityDefinitionInitialCapabilityRow>,
    /// Definition-level initial conditions. Seeds a `target_condition` onto
    /// every placed instance; previously a table-only surface unreachable from
    /// authored manifests.
    #[serde(default)]
    pub entity_definition_initial_condition: Vec<EntityDefinitionInitialConditionRow>,
    /// sk-ct8.3 / finding #37 (BL-19): "item is in a slot" → modifier fold on
    /// the WEARER. Each row says "while an entity of `entity_def_id` sits in
    /// ANY of the wearer's `entity_equipment_slot` rows, fold
    /// `modifier_template_id` into the wearer's effective-int overlay." Read
    /// when collecting the wearer’s facts.
    #[serde(default)]
    pub entity_definition_equip_modifier: Vec<EntityDefinitionEquipModifierRow>,
    /// sk-84i.2 (widened G-1): named DSL-source slots authored on an entity
    /// definition (e.g. slot `srd/damage` = `"2d8"`), evaluated inline by
    /// the kernel's `<subject>.source('slot')` primitive.
    #[serde(default)]
    pub entity_definition_item_dsl_source: Vec<EntityDefinitionItemDslSourceRow>,
    #[serde(default)]
    pub containment_policy_definition: Vec<ContainmentPolicyDefinitionRow>,
    #[serde(default)]
    pub entity_definition_containment_policy: Vec<EntityDefinitionContainmentPolicyRow>,
    #[serde(default)]
    pub entity_containment_policy: Vec<EntityContainmentPolicyRow>,
    /// Definition-level initial containment (sk-84i.3). Each row spawns an
    /// instance of `contained_entity_def_id` (recursively applying its own
    /// definition-level seeds) and inserts it loose-carried into every placed
    /// instance of `entity_def_id` — e.g. a bandit definition authoring its
    /// quiver, which itself authors its bolts.
    #[serde(default)]
    pub entity_definition_initial_containment: Vec<EntityDefinitionInitialContainmentRow>,
    /// Definition-level initial equipment (sk-84i.3). Each row spawns an
    /// instance of `equipped_entity_def_id` and equips it into `slot_id` on every
    /// placed instance of `entity_def_id` — the equipped-item sibling of
    /// `entity_definition_initial_containment`.
    #[serde(default)]
    pub entity_definition_initial_equipment: Vec<EntityDefinitionInitialEquipmentRow>,
    #[serde(default)]
    pub object_state_definitions: Vec<ObjectStateDefinitionRow>,
    #[serde(default)]
    pub object_state_definition_tag: Vec<ObjectStateDefinitionTagRow>,
    #[serde(default)]
    pub capability_definitions: Vec<CapabilityDefinitionRow>,
    #[serde(default)]
    pub role_definitions: Vec<RoleDefinitionRow>,
    #[serde(default)]
    pub role_aggregations: Vec<RoleAggregationRow>,
    #[serde(default)]
    pub role_bindings: Vec<RoleBindingRow>,
    #[serde(default)]
    pub capability_definition_tag: Vec<CapabilityDefinitionTagRow>,
    /// The IDENTITY→capability bridge (sk-bwd). Each row says "while an entity
    /// bears `tag`, it may act with `capability_id`" — read live by
    /// `capability_ids_for_actor`, never seeded onto a placed instance.
    #[serde(default)]
    pub tag_capability_grant: Vec<TagCapabilityGrantRow>,
    #[serde(default)]
    pub niche_definitions: Vec<NicheDefinitionRow>,
    #[serde(default)]
    pub niche_definition_tag: Vec<NicheDefinitionTagRow>,
    #[serde(default)]
    pub outcome_templates: Vec<OutcomeTemplateRow>,
    #[serde(default)]
    pub outcome_template_tag: Vec<OutcomeTemplateTagRow>,
    #[serde(default)]
    pub outcome_graphs: Vec<OutcomeGraphRow>,
    #[serde(default)]
    pub outcome_graph_nodes: Vec<OutcomeGraphNodeRow>,
    #[serde(default)]
    pub outcome_graph_edges: Vec<OutcomeGraphEdgeRow>,
    #[serde(default)]
    pub affordance_templates: Vec<AffordanceTemplateRow>,
    /// Optional template-owned participant declarations (ADR-0132).
    #[serde(default)]
    pub affordance_template_participants: Vec<AffordanceTemplateParticipantRow>,
    #[serde(default)]
    pub effect_definitions: Vec<EffectDefinitionRow>,
    #[serde(default)]
    pub consequence_templates: Vec<ConsequenceTemplateRow>,
    #[serde(default)]
    pub consequence_frame_writes: Vec<ConsequenceFrameWriteRow>,
    #[serde(default)]
    pub action_definitions: Vec<ActionDefinitionRow>,
    #[serde(default)]
    pub action_definition_tag: Vec<ActionDefinitionTagRow>,
    #[serde(default)]
    pub resource_definitions: Vec<ResourceDefinitionRow>,
    #[serde(default)]
    pub resource_definition_tag: Vec<ResourceDefinitionTagRow>,
    #[serde(default)]
    pub roll_tables: Vec<RollTableRow>,
    #[serde(default)]
    pub roll_table_tag: Vec<RollTableTagRow>,
    #[serde(default)]
    pub roll_table_entries: Vec<RollTableEntryRow>,
    #[serde(default)]
    pub roll_table_entry_tag: Vec<RollTableEntryTagRow>,
    #[serde(default)]
    pub modifier_target_definitions: Vec<ModifierTargetDefinitionRow>,
    #[serde(default)]
    pub stacking_discipline_definitions: Vec<StackingDisciplineDefinitionRow>,
    #[serde(default)]
    pub condition_remove_policy_definitions: Vec<ConditionRemovePolicyDefinitionRow>,
    #[serde(default)]
    pub modifier_templates: Vec<ModifierTemplateRow>,
    #[serde(default)]
    pub modifier_template_tag: Vec<ModifierTemplateTagRow>,
    #[serde(default)]
    pub condition_definitions: Vec<ConditionDefinitionRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub condition_capability_extent_grants: Vec<ConditionCapabilityExtentGrantRow>,
    #[serde(default)]
    pub condition_definition_tag: Vec<ConditionDefinitionTagRow>,
    /// Authored initial condition rows on a target. Validated at admission,
    /// but the server `r_apply_world_manifest` path does not commit them to
    /// the live `target_condition` table today (open bug `sk-jay0.2.47`) —
    /// use `[[entity_definition_initial_condition]]` to seed a condition
    /// onto placed instances.
    #[serde(default)]
    pub target_conditions: Vec<TargetConditionRow>,
    #[serde(default)]
    pub entities: Vec<EntityRow>,
    /// Relational instance-tag membership (CR-7 write leg / CR-4 relational-only
    /// convention). `entity_key` anchors on an authored `[[entities]]` row;
    /// the apply resolves it to the entity id and inserts an `entity_tag` row.
    #[serde(default)]
    pub entity_tag: Vec<EntityTagRow>,
    #[serde(default)]
    pub entity_containment: Vec<EntityContainmentRow>,
    #[serde(default)]
    pub object_state: Vec<ObjectStateRow>,
    /// Per-instance resource-bound overrides (#30 / sk-ct8.2).
    #[serde(default)]
    pub entity_resource_bound: Vec<EntityResourceBoundRow>,
    #[serde(default)]
    pub actors: Vec<ActorRow>,
    #[serde(default)]
    pub actor_identities: Vec<ActorIdentityRow>,
    #[serde(default)]
    pub actor_spatial_locations: Vec<ActorSpatialLocationRow>,
    #[serde(default)]
    pub actor_capabilities: Vec<ActorCapabilityRow>,
    // Author items and world objects through entities, containment and object state.
    #[serde(default)]
    pub equipment_slot_definitions: Vec<EquipmentSlotDefinitionRow>,
    /// Legacy compatibility overlay for equipment compatibility. Final
    /// authoring folds `slot_compatibility_predicate` into
    /// `entity_definitions`.
    #[serde(default)]
    pub item_definitions: Vec<ItemDefinitionRow>,
    #[serde(default)]
    pub actor_equipment_slots: Vec<ActorEquipmentSlotRow>,
    // machine_port_inventory: folded into entity_containment with non-empty slot_id (Phase 2).
    // Authors who previously used these sections should migrate to autonomy_job rows authored
    // through the spatial_niche_attachments + affordance machinery or direct reducer calls.
    #[serde(default)]
    pub spatial_niche_attachments: Vec<SpatialNicheAttachmentRow>,
    /// Authored spatial ambient-state vocabulary. Commits to the live
    /// `spatial_state_definition` table.
    #[serde(default)]
    pub spatial_state_definitions: Vec<SpatialStateDefinitionRow>,
    /// Authored spatial ambient-state baselines. Commits only the applying
    /// bundle's row for each target and state; play's world-owned override wins
    /// and survives whole-bundle or delta re-apply (ADR-0154 G-3). To reset an
    /// override, author an Inscribe act that writes the desired baseline value,
    /// or reset the world through the operator recovery path. Readers do not
    /// expire values by freshness_rule; authored predicates inspect updated_tick.
    #[serde(default)]
    pub spatial_environment_state: Vec<SpatialEnvironmentStateRow>,
    /// Item 29 — area-shape vocabulary rows (sphere, cone, line, cube, node_ball).
    #[serde(default)]
    pub area_shape_definitions: Vec<AreaShapeDefinitionRow>,
    /// Action-economy: authored per-round time channels. Insert into the live
    /// `economy_channel_definitions` table.
    #[serde(default)]
    pub economy_channel_definitions: Vec<EconomyChannelDefinitionRow>,
    /// Action-economy (batch 2): turn-advance scheduler rows. Insert into the
    /// live `turn_scheduler` table (#433); exactly one `active` per world.
    #[serde(default)]
    pub turn_scheduler: Vec<TurnSchedulerRow>,
    /// ADR-0037: authored session templates. Insert into the live
    /// `session_definition` table. Bundle-scoped idempotent apply (clear-then-insert).
    #[serde(default)]
    pub session_definitions: Vec<SessionDefinitionRow>,
    /// ADR-0052: reusable authored blueprints for session-minted combat grids.
    /// Insert into the live `combat_grid_blueprint` table.
    #[serde(default)]
    pub combat_grid_blueprints: Vec<CombatGridBlueprintRow>,
    /// ADR-0041 / CR-14: authored ambient autonomy jobs. Insert into the live
    /// `autonomy_job` table. Bundle-scoped idempotent apply (clear-then-insert).
    /// ADR-0004 D-2 fence: a job's `dsl_function` may only resolve to an
    /// autonomy-eligible (ambient, non-actor-reaching) effect — enforced by the
    /// validator (`autonomy_jobs.rs`). NO movement/actor effects (no patrols).
    #[serde(default)]
    pub autonomy_jobs: Vec<AutonomyJobRow>,
    /// ADR-0043 / CR-22: authored trace-kind vocabulary. Insert into the live
    /// `trace_kind_definition` table. Bundle-scoped idempotent apply
    /// (clear-then-insert). Opens the closed `TRACE_KINDS` bootstrap list to
    /// modder authoring (e.g. a `phandelver/utterance` speech-trace kind).
    #[serde(default)]
    pub trace_kind_definitions: Vec<TraceKindDefinitionRow>,
    /// Typed, author-time DSL macro definitions. Their bodies are compiled to
    /// the existing DSL AST before admission; no macro-call node is stored.
    #[serde(default)]
    pub dsl_macro_definitions: Vec<DslMacroDefinitionRow>,
    /// ADR-0085 D-6: authored deletion tombstones for the upsert-semantics
    /// `r_apply_manifest_delta` reducer. Upsert cannot express deletion by
    /// omission (an authored key simply absent from a delta says nothing
    /// about whether it should be removed), so a delta MAY carry an explicit
    /// `[[retractions]]` array. Ignored by the whole-bundle
    /// `r_apply_world_manifest` clear-then-insert path (which has its own,
    /// unconditional clear).
    #[serde(default)]
    pub retractions: Vec<RetractionRow>,
    /// `[[include]]` directives are processed and inlined by the manifest
    /// loader before this struct sees the concatenated source. The field
    /// exists so the loader's pre-resolved directives don't trip
    /// `deny_unknown_fields` when the resolved string is re-parsed as a
    /// `Manifest`. Server-side apply logic ignores it.
    #[serde(default, rename = "include")]
    pub includes: Vec<IncludeDirective>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IncludeDirective {
    pub path: String,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            binding_preconditions: Vec::new(),
            schema_version: SchemaVersion::new(1, 0, 0),
            applyset: None,
            metadata: BTreeMap::new(),
            metadata_namespaces: Vec::new(),
            metadata_attachments: Vec::new(),
            bundles: Vec::new(),
            tag_value_definitions: Vec::new(),
            place_graphs: Vec::new(),
            place_seals: Vec::new(),
            place_graph_instances: Vec::new(),
            place_nodes: Vec::new(),
            place_edges: Vec::new(),
            map_layers: Vec::new(),
            place_edge_affordance: Vec::new(),
            movement_defaults: Vec::new(),
            substrate_definitions: Vec::new(),
            coordinate_kind_definitions: Vec::new(),
            anchor_kind_definitions: Vec::new(),
            extent_kind_definitions: Vec::new(),
            distance_metric_definitions: Vec::new(),
            audience_kind_definitions: Vec::new(),
            prompt_definitions: Vec::new(),
            boundary_definitions: Vec::new(),
            location_definitions: Vec::new(),
            place_node_traces: Vec::new(),
            location_traces: Vec::new(),
            entity_definitions: Vec::new(),
            entity_definition_tag: Vec::new(),
            entity_definition_initial_state: Vec::new(),
            entity_definition_initial_capability: Vec::new(),
            entity_definition_initial_condition: Vec::new(),
            entity_definition_equip_modifier: Vec::new(),
            entity_definition_item_dsl_source: Vec::new(),
            containment_policy_definition: Vec::new(),
            entity_definition_containment_policy: Vec::new(),
            entity_containment_policy: Vec::new(),
            entity_definition_initial_containment: Vec::new(),
            entity_definition_initial_equipment: Vec::new(),
            object_state_definitions: Vec::new(),
            object_state_definition_tag: Vec::new(),
            capability_definitions: Vec::new(),
            role_definitions: Vec::new(),
            role_aggregations: Vec::new(),
            role_bindings: Vec::new(),
            capability_definition_tag: Vec::new(),
            tag_capability_grant: Vec::new(),
            niche_definitions: Vec::new(),
            niche_definition_tag: Vec::new(),
            outcome_templates: Vec::new(),
            outcome_template_tag: Vec::new(),
            outcome_graphs: Vec::new(),
            outcome_graph_nodes: Vec::new(),
            outcome_graph_edges: Vec::new(),
            affordance_templates: Vec::new(),
            affordance_template_participants: Vec::new(),
            effect_definitions: Vec::new(),
            consequence_templates: Vec::new(),
            consequence_frame_writes: Vec::new(),
            action_definitions: Vec::new(),
            action_definition_tag: Vec::new(),
            resource_definitions: Vec::new(),
            resource_definition_tag: Vec::new(),
            roll_tables: Vec::new(),
            roll_table_tag: Vec::new(),
            roll_table_entries: Vec::new(),
            roll_table_entry_tag: Vec::new(),
            modifier_target_definitions: Vec::new(),
            stacking_discipline_definitions: Vec::new(),
            condition_remove_policy_definitions: Vec::new(),
            modifier_templates: Vec::new(),
            modifier_template_tag: Vec::new(),
            condition_definitions: Vec::new(),
            condition_capability_extent_grants: Vec::new(),
            condition_definition_tag: Vec::new(),
            target_conditions: Vec::new(),
            entities: Vec::new(),
            entity_tag: Vec::new(),
            entity_containment: Vec::new(),
            object_state: Vec::new(),
            entity_resource_bound: Vec::new(),
            actors: Vec::new(),
            actor_identities: Vec::new(),
            actor_spatial_locations: Vec::new(),
            actor_capabilities: Vec::new(),
            equipment_slot_definitions: Vec::new(),
            item_definitions: Vec::new(),
            actor_equipment_slots: Vec::new(),
            spatial_niche_attachments: Vec::new(),
            spatial_state_definitions: Vec::new(),
            spatial_environment_state: Vec::new(),
            area_shape_definitions: Vec::new(),
            economy_channel_definitions: Vec::new(),
            turn_scheduler: Vec::new(),
            session_definitions: Vec::new(),
            combat_grid_blueprints: Vec::new(),
            autonomy_jobs: Vec::new(),
            trace_kind_definitions: Vec::new(),
            dsl_macro_definitions: Vec::new(),
            retractions: Vec::new(),
            includes: Vec::new(),
        }
    }
}

/// ADR-0085 D-6: one authored deletion tombstone. `row_kind` selects which
/// commit-time cascade resolves `authored_key`; `authored_key` is the exact
/// natural/provenance key the corresponding live table already stores (for
/// `row_kind = "entity"`, this is `manifest_authored_entity.manifest_key`,
/// i.e. `"{bundle_id}:{entity_key}"`. `spellkave-validate` checks the retraction
/// vocabulary (`SUPPORTED_RETRACTION_ROW_KINDS` in
/// `crates/spellkave-validate/src/retractions.rs`) and rejects any other
/// `row_kind` Fatal rather than silently accepting a tombstone the server
/// cannot execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetractionRow {
    pub row_kind: String,
    pub authored_key: String,
}

/// One ordered, typed parameter in a [`DslMacroDefinitionRow`].
///
/// `value_type` is the pre-existing DSL type family. Schemars projects the
/// serde enum as a string until that shared enum owns its JSON-schema derive;
/// deserialization still accepts only the enum's closed values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DslMacroParameterRow {
    pub name: String,
    #[schemars(with = "String")]
    pub value_type: DslValueType,
}

/// A reusable typed DSL expression fragment. The macro-aware compiler owns
/// parsing and expansion; this manifest row is deliberately just the durable,
/// lossless authoring definition it receives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DslMacroDefinitionRow {
    pub macro_id: String,
    pub bundle_id: String,
    #[schemars(with = "String")]
    pub return_type: DslValueType,
    #[serde(default)]
    pub parameters: Vec<DslMacroParameterRow>,
    pub body_source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BundleRow {
    pub bundle_id: String,
    pub display_name: String,
    pub author: String,
    pub version: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub description: String,
    /// Bundle-owned tag-vocabulary namespaces (`tag-vocabulary-declaration`
    /// change, Decision 1). Empty = the bundle declares no namespace of its
    /// own; downstream tooling (validator, `sk` CLI, shared author tools) treats
    /// an undeclared bundle exactly as it did before this field existed.
    #[serde(default)]
    pub tag_prefixes: Vec<TagPrefixDeclaration>,
    /// Opts out of the dot-namespace convention for tags. Defaults to `false`.
    #[serde(default)]
    pub allow_flat_tags: bool,
    /// Bundle-level default extent shape reference (place-node-extent, D-2):
    /// a `place_node` with no authored `extent_shape_id` resolves its extent
    /// from here. Empty = the bundle carries no default (a node with neither
    /// is a well-formedness failure, not a silent absence).
    #[serde(default)]
    pub default_extent_shape_id: String,
    #[serde(default)]
    pub default_extent_params_json: String,
}

/// Wire shape for `TagPrefixDeclaration` deserialized before shape
/// validation (see `TryFrom` below) — kept private so callers only ever see
/// the shape-checked `TagPrefixDeclaration`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TagPrefixDeclarationRaw {
    prefix: String,
    #[serde(default)]
    target_kind: Option<String>,
    #[serde(default)]
    natural_key_field: Option<String>,
}

/// A single bundle-owned tag-vocabulary namespace declaration
/// (`tag-vocabulary-declaration` change, design Decision 1). `prefix` is
/// an open string the validator never enumerates against a Rust-coded
/// legal-value list — this stays inside ADR-0018 D-1's closed-string-enum
/// prohibition without building D-1's `tag_definition` value-vocabulary
/// table; `tag_prefixes` is bundle-level *namespace* metadata, not a
/// per-tag-value row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "TagPrefixDeclarationRaw")]
pub struct TagPrefixDeclaration {
    /// The namespace this bundle owns, e.g. "srd" (matching a dotted prefix
    /// "srd.<name>") or the multi-segment "srd.ability" (matching
    /// "srd.ability.<code>" under the canonical ref-tag convention — see
    /// design Decision 3). MAY itself contain dots; no trailing dot is
    /// stored, the validator appends "." when matching a tag's namespace
    /// boundary.
    pub prefix: String,
    /// Optional ref semantics: if present, every tag under this prefix is
    /// expected to resolve — its suffix (the part after "prefix.") should
    /// match a row's natural-key id in `target_kind`, exactly or as its
    /// final "/"-segment (mirrors `ref-tags.ts`'s existing suffix-match
    /// rule). `target_kind` is an open row-kind name — see design's
    /// "structure, not content" note for what "open" does and does not buy.
    #[serde(default)]
    pub target_kind: Option<String>,
    /// Required when `target_kind` is set: the natural-key field on that
    /// target row kind (mirrors `RefTagFamily.naturalKeyField`). Validated
    /// against the row-kind registry by `spellkave-validate` — see
    /// design's natural-key cross-check note.
    #[serde(default)]
    pub natural_key_field: Option<String>,
}

/// One exact, bundle-owned member of a declared tag vocabulary.
///
/// `tag`, `prefix`, and `bundle_id` establish identity, namespace membership,
/// and ownership respectively. `description` is presentation metadata only:
/// it MUST NOT select behavior, dispatch, affordances, predicates, niches, or
/// runtime rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TagValueDefinitionRow {
    pub tag: String,
    pub prefix: String,
    pub bundle_id: String,
    pub description: String,
}

impl TryFrom<TagPrefixDeclarationRaw> for TagPrefixDeclaration {
    type Error = String;

    /// Shape-level invariant (`world-manifest-authoring` spec: "`TagPrefixDeclaration`
    /// ref semantics are internally consistent"): `target_kind` and
    /// `natural_key_field` are set together or not at all. This is checked
    /// at deserialize time (not left to the semantic validator) so a
    /// malformed entry fails to parse with a message naming both the
    /// entry's `prefix` and the missing field.
    fn try_from(raw: TagPrefixDeclarationRaw) -> Result<Self, Self::Error> {
        match (raw.target_kind.is_some(), raw.natural_key_field.is_some()) {
            (true, false) => Err(format!(
                "tag_prefixes entry '{}' sets target_kind without natural_key_field — both must be set together, or neither",
                raw.prefix
            )),
            (false, true) => Err(format!(
                "tag_prefixes entry '{}' sets natural_key_field without target_kind — both must be set together, or neither",
                raw.prefix
            )),
            _ => Ok(Self {
                prefix: raw.prefix,
                target_kind: raw.target_kind,
                natural_key_field: raw.natural_key_field,
            }),
        }
    }
}

/// Compact spatial anchor used by authoring rows that instantiate world
/// content into a location.
///
/// `kind` is an open string discriminator resolved by
/// `anchor_kind_definition`; `id` carries the address in that namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpatialRef {
    pub kind: String,
    pub id: String,
}

/// Authoring shape for `anchor_kind_definition` (Issue #336).
///
/// Modders add new anchor-kind vocabulary by writing
/// `[[anchor_kind_definitions]]` rows in their bundle TOML. The kernel
/// dispatches by `resolution_shape` (`H3Cell` / `PlaceNode`), not by
/// `kind_id`. Core anchor kinds ship as seeded rows under `bundle_id="core"`
/// — modders should NOT re-author those.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnchorKindDefinitionRow {
    pub kind_id: String,
    pub bundle_id: String,
    /// Parse/resolution handler. Must be one of `"H3Cell"` / `"PlaceNode"`
    /// (the closed `AnchorResolutionShape` enum). The manifest loader rejects
    /// unknown values.
    pub resolution_shape: String,
    #[serde(default)]
    pub description: String,
}

/// Authoring shape for `boundary_definitions` (CR-12 / LR-1; batch-2
/// condition-4 carrier).
///
/// Reset-boundary vocabulary: the named moments at which an action class's
/// budget resets (`economy_channel_definitions.resets_on`, and later slice B's
/// `recharges_on` / `fire_at_boundary`). `id` IS the boundary token
/// (`turn_start`, `short_rest`, a modder's `encounter_end`). Modders extend the
/// vocabulary by authoring a row, never by editing Rust — the legal set is
/// data, not a `const &[&str]` (ADR-0018 D-1).
///
/// Deliberately NOT applied to a server table: the kernel fires only the
/// boundaries it knows from its own clock (slice B's `TurnStart` / `TurnEnd`),
/// so no runtime table consumer exists and a vocabulary table would be a
/// zero-consumer fossil. This row kind is parsed-and-validated only — the
/// table-granularity form of slice A's deferred-field pattern (the four
/// `affordance_template` / `action_definition` fields deserialize but store
/// nowhere yet). The validator builds the legal set from these rows, scoped by
/// `bundle_id` through the same `depends_on` closure every cross-bundle
/// reference uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundaryDefinitionRow {
    pub id: String,
    pub bundle_id: String,
    #[serde(default)]
    pub description: String,
}

/// Compact place-graph instancing row (Item 25 D7).
///
/// Manifest-apply expands this row into concrete `place_graph` + `place_node`
/// + `place_edge` rows by cloning a blueprint graph under a fresh id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceGraphInstanceRow {
    /// Applying bundle that owns every expanded instance row, independent of
    /// blueprint provenance. Omission is compatible with a single applying
    /// bundle or a blueprint authored in this manifest. A foreign blueprint
    /// in a multi-bundle apply requires an explicit owner (MF-VAL-373).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// FK → `place_graph_definition.place_graph_id` (must resolve to a
    /// blueprint row, not an already-instanced graph).
    pub blueprint_id: String,
    /// Optional caller-provided instance graph id. When set, manifest apply uses
    /// it verbatim and rejects duplicates; when omitted, apply keeps the legacy
    /// timestamp-derived `<blueprint_id>#<short_hex>` fallback.
    #[serde(default)]
    pub instance_id: Option<String>,
    /// `None` means this instance is rooted at world level.
    /// `Some(id)` means this instance is a component of `id`.
    #[serde(default)]
    pub parent_graph_id: Option<String>,
    /// Spatial anchor for the instance root (e.g. h3 cell for a world root,
    /// or place-node anchor for nested composition).
    pub anchor: SpatialRef,
    /// Optional clockwise rotation override in degrees.
    /// `None` keeps the blueprint's authored bearing.
    #[serde(default)]
    pub bearing_deg: Option<f32>,
    /// Optional override map.
    /// Known keys today:
    /// - `metres_per_unit`: f32 override for graph-unit→metre ratio
    /// - `node_display_name.<local_id>`: per-node display-name override
    #[serde(default)]
    pub override_params: Option<BTreeMap<String, String>>,
    /// Authored description (#1968) — open string, never load-bearing (no
    /// mechanic reads it; see `Manifest`'s doc comment for the class rule).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

fn default_metres_per_unit() -> f32 {
    1.0
}

/// Item 28 — metric place-graph embedding. `bearing` and `metres_per_unit`
/// are opt-in: non-metric graphs omit them (defaults: 0.0 / 1.0 = identity).
/// A graph is metric iff all its nodes carry `pos_present` — there is no
/// stored flag column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceGraphRow {
    pub place_graph_id: String,
    pub bundle_id: String,
    pub substrate_id: String,
    pub display_name: String,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub origin_cell_res10: u64,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub origin_cell_res7: u64,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub origin_cell_res4: u64,
    #[serde(default)]
    pub description: String,
    /// Clockwise rotation in degrees; 0.0 = no rotation.
    #[serde(default)]
    pub bearing: f32,
    /// Ratio: how many world metres one graph-local unit represents; 1.0 = identity.
    #[serde(default = "default_metres_per_unit")]
    pub metres_per_unit: f32,
    /// Instancing (Item 25 D2): empty = this IS the blueprint;
    /// non-empty = instance stamped from that blueprint.
    #[serde(default)]
    pub blueprint_id: String,
    /// Composition (Item 25 D4): empty = root instance;
    /// non-empty = component of the named parent graph (self-FK).
    #[serde(default)]
    pub parent_graph_id: String,
    /// Anchor node (Brief 26 D3): FK → `place_node.node_id`.
    /// The eject-destination for entities displaced by `Despawn`.
    /// Must be non-empty for every authored graph.
    /// `serde(default)` permits the field to be absent in legacy rows
    /// (produces an empty string, caught by the doctor check at runtime).
    #[serde(default)]
    pub anchor_node_id: String,
    #[serde(default)]
    pub is_ephemeral_instance: bool,
}

/// A seal belongs to the bundle owning the graph's root. Instances inherit copies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceSealRow {
    pub place_graph_id: String,
    pub bundle_id: String,
    #[serde(default)]
    pub description: String,
    /// Export-only runtime overlay; authored inputs carrying it are rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broken: Option<bool>,
    /// Export-only audit reason, retained in the row hash for drift detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceNodeRow {
    pub node_id: String,
    pub bundle_id: String,
    pub place_graph_id: String,
    pub local_id: String,
    pub substrate_id: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cell_res10: u64,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cell_res7: u64,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cell_res4: u64,
    #[serde(default)]
    pub pose_elevation: f32,
    #[serde(default)]
    pub pose_elevation_present: bool,
    #[serde(default)]
    pub pos_x: i32,
    #[serde(default)]
    pub pos_y: i32,
    #[serde(default)]
    pub pos_present: bool,
    /// Authored extent shape reference (`area_shape_definition.shape_id`);
    /// empty = resolve via the owning bundle's default extent
    /// (`BundleRow::default_extent_shape_id`). Replaces the removed
    /// cell-denominated `size_w`/`size_h` (place-node-extent, D-1).
    #[serde(default)]
    pub extent_shape_id: String,
    #[serde(default)]
    pub extent_params_json: String,
    #[serde(default)]
    pub is_ephemeral: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceEdgeRow {
    pub edge_key: String,
    pub bundle_id: String,
    pub place_graph_id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    #[serde(default)]
    pub pose_dt_elevation: f32,
    #[serde(default)]
    pub pose_dt_elevation_present: bool,
    #[serde(default)]
    pub substrate_crossing: bool,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cell_anchor: u64,
    #[serde(default)]
    pub cell_anchor_u: f32,
    #[serde(default)]
    pub cell_anchor_v: f32,
    #[serde(default)]
    pub cell_anchor_present: bool,
    #[serde(default)]
    pub from_anchor_side: String,
    #[serde(default)]
    pub from_anchor_offset: f32,
    #[serde(default)]
    pub to_anchor_side: String,
    #[serde(default)]
    pub to_anchor_offset: f32,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// ADR-0101 D-1/D-2: whether this edge receives the resolved
    /// `movement_default` mode set (edge > graph > bundle). `false`
    /// **replaces** the default entirely — the edge then owns its full mode
    /// set from its own `place_edge_affordance` rows alone (the fly-only
    /// chasm idiom). Defaults to `true` so every legacy edge keeps
    /// inheriting once a bundle/graph default is authored.
    #[serde(default = "default_true")]
    pub inherit_movement_default: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapLayerRow {
    pub map_layer_id: String,
    pub bundle_id: String,
    pub place_graph_id: String,
    pub display_name: String,
    #[serde(default)]
    pub asset_module: String,
    pub asset_path: String,
    #[serde(default)]
    pub source_kind: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub posture: String,
    #[serde(default)]
    pub image_width_px: u32,
    #[serde(default)]
    pub image_height_px: u32,
    #[serde(default)]
    pub grid_size_px: u32,
    #[serde(default)]
    pub grid_distance_m: f32,
    #[serde(default)]
    pub local_width_cells: f32,
    #[serde(default)]
    pub local_height_cells: f32,
    #[serde(default)]
    pub image_origin_x: f32,
    #[serde(default)]
    pub image_origin_y: f32,
    #[serde(default)]
    pub rotation_deg: f32,
    #[serde(default)]
    pub min_zoom: f32,
    #[serde(default)]
    pub max_zoom: f32,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceEdgeAffordanceRow {
    pub edge_key: String,
    pub affordance_template_id: String,
    #[serde(default)]
    pub bindings_json: String,
    /// Open author-chosen traversal-mode id (ADR-0101 D-1). Empty ⇒ the
    /// implicit default mode — every legacy row (authored before modes
    /// existed) keeps its pre-existing AND-chain meaning. Rows sharing one
    /// `(edge_key, mode_id)` compose as AND; the edge is traversable iff at
    /// least one mode passes (OR). Not a closed enum (ADR-0018): an author
    /// invents "swim"/"fly"/"wade" freely.
    #[serde(default)]
    pub mode_id: String,
}

/// `[[movement_defaults]]` — declares the default traversal mode set for a
/// bundle (`place_graph_id` empty) or one place-graph (ADR-0101 D-1/D-2).
/// Resolution per edge is EDGE > GRAPH > BUNDLE: the nearest scope that
/// declares any default row supplies the whole default mode set, which the
/// edge's own `place_edge_affordance` rows then refine (same
/// `(mode_id, affordance_template_id)`) or AND-join (same mode, other
/// template) or extend (new `mode_id`). Not a natural-keyed row kind —
/// bundle-scoped clear-then-insert at commit, same discipline as
/// `spatial_environment_state` (excluded from `ROW_KIND_NATURAL_KEY`, no
/// single-column author identity).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovementDefaultRow {
    pub bundle_id: String,
    /// Empty ⇒ bundle scope; non-empty ⇒ scoped to this one `place_graph_id`.
    #[serde(default)]
    pub place_graph_id: String,
    /// Empty ⇒ the implicit default mode (ADR-0101 D-1).
    #[serde(default)]
    pub mode_id: String,
    pub affordance_template_id: String,
    #[serde(default)]
    pub bindings_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoordinateKindDefinitionRow {
    pub coordinate_kind_id: String,
    pub bundle_id: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubstrateDefinitionRow {
    pub substrate_id: String,
    pub bundle_id: String,
    /// FK → coordinate_kind_definition.coordinate_kind_id.
    pub coordinate_kind_id: String,
    #[serde(default)]
    pub description: String,
}

/// Item 29 — area-shape vocabulary row.
/// `parameter_schema_json` is a JSON object keyed by parameter name, valued
/// by type hint (e.g. `{"radius_m":"f32"}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AreaShapeDefinitionRow {
    pub shape_id: String,
    pub bundle_id: String,
    pub parameter_schema_json: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationDefinitionRow {
    pub location_def_id: String,
    pub bundle_id: String,
    pub display_name: String,
    #[serde(default)]
    pub parent_def: String,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cell_res10: u64,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cell_res7: u64,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cell_res4: u64,
    #[serde(default)]
    pub interior_graph_id: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceNodeTraceRow {
    pub node_id: String,
    pub trace_kind_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocationTraceRow {
    pub location_def_id: String,
    pub trace_kind_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionRow {
    pub entity_def_id: String,
    pub bundle_id: String,
    pub display_name: String,
    #[serde(default)]
    pub lineage_id: String,
    #[serde(default)]
    pub affordance_seed_id: String,
    #[serde(default)]
    pub on_consume_consequence_id: String,
    #[serde(default)]
    pub on_destroy_consequence_id: String,
    /// Capability ids (CSV) granted to the bearer entity that equips this
    /// entity definition (Brief 11 D8 — authoring-side grant source).
    #[serde(default)]
    pub grants_capability_while_equipped: String,
    /// Capability ids (CSV) granted to entities at locations where an entity
    /// of this definition is present (Brief 11 D8).
    #[serde(default)]
    pub grants_capability_while_present_in_location: String,
    /// Closed three-form slot-compatibility grammar (ADR-0118), not
    /// `spellkave-dsl`. Empty/`true` = any slot; `false` = none;
    /// `slot_id == "<id>"` = that slot only. Evaluated by
    /// `spellkave_validate::equip_predicate`.
    #[serde(default)]
    pub slot_compatibility_predicate: String,
    /// sk-ct8.3 / finding #37 (BL-19): `consequence_template_id` executed
    /// (via `apply_consequence_with_bindings`, `Actor` bound to the wearer)
    /// when an entity of this definition is equipped by ANY affordance —
    /// `EquipItem`'s own frame-write arm, not a separate action. Empty
    /// string = no hook.
    #[serde(default)]
    pub on_equip_consequence_id: String,
    /// Same shape as `on_equip_consequence_id`, fired on `UnequipItem` AND
    /// on `unequip_for_entity_cleanup` (drop/transfer/destroy) so a cursed
    /// ring's effect cannot be dodged by destroying the item instead of
    /// unequipping it. Empty string = no hook.
    #[serde(default)]
    pub on_unequip_consequence_id: String,
    /// Whether authored entities of this definition may use `quantity > 1`.
    #[serde(default)]
    pub stackable: bool,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObjectStateDefinitionRow {
    pub state_id: String,
    pub bundle_id: String,
    pub target_kind: String,
    pub display_name: String,
    #[serde(default = "default_public")]
    pub default_visibility: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDefinitionRow {
    pub capability_id: String,
    pub bundle_id: String,
    pub actor_kind: String,
    pub display_name: String,
    pub description: String,
    /// ADR-0120 read-visibility default for capabilities of this vocabulary.
    /// Concrete grants inherit this unless an authored `[[actor_capabilities]]`
    /// row overrides it. Omitted → `"public"`.
    #[serde(default = "default_public")]
    pub default_visibility: String,
}

/// Authored role vocabulary; a definition carries no binding or authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleDefinitionRow {
    pub role_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub description: String,
    pub capability_ids: Vec<String>,
}

/// A bundle-owned contribution to an existing role's capability set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleAggregationRow {
    pub role_id: String,
    pub bundle_id: String,
    pub description: String,
    pub capability_ids: Vec<String>,
}

/// A named role bound to an identity or an authored non-player actor over an extent.
/// Realisation is server state, never an author-controlled field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleBindingRow {
    pub binding_id: String,
    pub bundle_id: String,
    pub subject: String,
    pub role_id: String,
    #[serde(default)]
    pub extent_kind_ref: String,
    pub extent_ref: String,
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub expires_at_tick: Option<u64>,
    pub description: String,
    /// Shared authored briefing, private with the binding; never executable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handover_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtentKindDefinitionRow {
    pub kind_id: String,
    pub resolution_shape: String,
    pub bundle_id: String,
    pub description: String,
}

/// ADR-0142 D-1/D-4 — audience vocabulary row. `resolution_shape` is one of
/// the closed K-5 arms `DirectedTrace` | `Broadcast`; `audience_kind_id`
/// stays an open authored string (ADR-0018 D-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudienceKindDefinitionRow {
    pub audience_kind_id: String,
    pub resolution_shape: String,
    pub bundle_id: String,
    pub description: String,
}

/// A dialogue prompt contract (ADR-0142). The locality fields mirror
/// those on [`AffordanceTemplateRow`] (ADR-0027).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromptDefinitionRow {
    pub prompt_def_id: String,
    pub bundle_id: String,
    pub source_selector: String,
    #[serde(default)]
    pub open_predicate_source: String,
    #[serde(default)]
    pub response_action_ids: Vec<String>,
    pub audience_kind_ref: String,
    #[serde(default)]
    pub trace_window_ticks: u64,
    #[serde(default)]
    pub contract_version: u32,
    /// Modder opt-in acknowledging a `Global`-classified `open_predicate_source`.
    /// Not yet enforced at manifest commit (see struct doc).
    #[serde(default)]
    pub accepts_global_storm: bool,
}

/// The three `distance_metric_definition` rows the `core` bundle seeds
/// (ADR-0134 D-1). The offline validator treats them as always visible when
/// it resolves `combat_grid_blueprints.distance_metric_id`; the server seed
/// list (`spatial::distance_metric::CORE_DISTANCE_METRICS`) pins equality.
pub const CORE_DISTANCE_METRIC_IDS: &[&str] = &[
    "core/metric/euclidean",
    "core/metric/chebyshev",
    "core/metric/manhattan",
];

/// ADR-0134 D-1 — one authored distance metric. `metric_shape` names the
/// closed kernel formula (`Euclidean` | `Chebyshev` | `Manhattan`); the
/// commit path and validator reject any other value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DistanceMetricDefinitionRow {
    pub metric_id: String,
    pub metric_shape: String,
    pub bundle_id: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NicheDefinitionRow {
    pub niche_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub description: String,
    /// DSL predicate source (empty → trivially-true; matches any candidate target).
    /// Added in Brief 11 D2 to replace `target_kind` pre-categorisation.
    #[serde(default)]
    pub predicate_source: String,
    /// actor-carried-verbs D1/D2 (canon-warden amendment): FK into
    /// `niche_provider_definition.provider_id` — WHO provides this niche
    /// (`place` | `entity` | `actor`). Empty resolves to `place`, preserving
    /// today's place-attachment-gated behaviour for every pre-existing niche.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum VisibilityClass {
    Public,
    #[default]
    Local,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutcomeTemplateRow {
    pub outcome_template_id: String,
    pub bundle_id: String,
    pub display_name: String,
    #[serde(default)]
    pub outcome_bool: bool,
    /// DSL outcome source (empty -> `outcome_bool` compatibility carrier).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub outcome_expression_source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manipulation_action_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub communication_speech_act_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub movement_descriptor: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sense_query_kind: String,
    /// Visibility carried by sensing affordances. Existing manifests default Local.
    #[serde(default)]
    pub sense_visibility: VisibilityClass,
    /// Optional consequence applied when this outcome template is committed by
    /// an outcome graph node. Empty means no graph-node effect.
    #[serde(default)]
    pub consequence_template_id: String,
    /// CR-3 / ADR-0038 lift: whether a successful sense via this outcome marks
    /// the target's spatial discovery as *validated* — was the
    /// `sense.discovery=validated` behaviour kv-tag. Absent → `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub discovery_validated: bool,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

fn default_outcome_graph_max_depth() -> u32 {
    8
}

fn default_outcome_graph_max_nodes() -> u32 {
    32
}

fn default_outcome_graph_max_draws() -> u32 {
    8
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutcomeGraphRow {
    pub outcome_graph_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub root_node_id: String,
    #[serde(default = "default_outcome_graph_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_outcome_graph_max_nodes")]
    pub max_nodes: u32,
    #[serde(default = "default_outcome_graph_max_draws")]
    pub max_draws: u32,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutcomeGraphNodeRow {
    pub outcome_graph_node_id: String,
    pub outcome_graph_id: String,
    pub node_id: String,
    #[serde(default)]
    pub outcome_template_id: String,
    #[serde(default)]
    pub terminal: bool,
    #[serde(default)]
    pub authored_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutcomeGraphEdgeRow {
    pub outcome_graph_edge_id: String,
    pub outcome_graph_id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    #[serde(default)]
    pub guard_predicate_source: String,
    #[serde(default)]
    pub authored_order: u32,
}

// NOTE: not `Eq` — `range_m` is an `f32` (floats are not `Eq`), matching the
// other float-bearing manifest rows above.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AffordanceTemplateRow {
    pub template_id: String,
    pub bundle_id: String,
    pub niche_id: String,
    pub capability_id: String,
    pub display_name: String,
    #[serde(default)]
    pub predicate_bool: bool,
    /// DSL source text for this template's predicate (empty → `true`).
    #[serde(default)]
    pub predicate_source: String,
    /// Modder opt-in: acknowledges that this template's predicate classifies
    /// as `Global` and may dirty all actors with the required capability.
    #[serde(default)]
    pub accepts_global_storm: bool,
    /// Validator-assigned locality class (written during manifest validation,
    /// read by the materialiser at runtime). One of: Local, Place, Family, Global.
    #[serde(default)]
    pub locality_class: String,
    /// Validator-computed verdict for whether the predicate reads a
    /// co-located-count / occupancy fact. Runtime arrival scoping reads this
    /// stored value; it never re-walks predicate ASTs at frame time.
    #[serde(default)]
    pub reads_occupancy: bool,
    /// Whether the predicate-dependency graph is cycle-free (stratified).
    /// `true` when the validator finds no SCC of size > 1 involving affordance.
    #[serde(default = "default_true")]
    pub stratification_ok: bool,
    pub outcome_template_id: String,
    /// Metric reach for spatial targets, in place-node embedding units
    /// (ADR-0053). `0.0` preserves touch/same-node reachability.
    #[serde(default)]
    pub range_m: f32,
    /// Action-economy (batch 2): the action class this affordance draws from
    /// the actor's per-round channel pool (FK → `economy_channel_definitions.id`).
    /// Absent → no budget gate fires. The budget gate reads this from the
    /// resolved template, never from the materialised `affordance` cache
    /// (ADR-0011 D-3). Consumed by the slice-C budget gate.
    #[serde(default)]
    pub spends_channel: Option<String>,
    /// Action-economy: recharge boundary (`turn_start` | `turn_end`) after a
    /// successful fire; absent → no recharge clock. Closed set is enforced
    /// validator-side (no Rust enum, ADR-0018). Consumed by the recharge clock.
    #[serde(default)]
    pub recharges_on: Option<String>,
    /// Action-economy: ordered sub-`affordance_template` ids this template
    /// unrolls at resolve time (the Multiattack shape). Absent → not composing.
    /// Validation rejects cycles + dangling sub-ids. Consumed by the slice-D
    /// resolver unroll.
    #[serde(default)]
    pub composes: Option<Vec<String>>,
    /// ADR-0101 D-1/D-4: authored DSL int expression (`Outcome` kind, same
    /// shared crate as `predicate_source`) that multiplies the traversal
    /// edge's authored/metric distance when this template charges movement.
    /// Parsed at manifest commit next to `predicate_source` (FATAL on
    /// parse/type error). Empty ⇒ multiplier 1 (today's static-cost
    /// behaviour, unchanged). The kernel never interprets a state id or
    /// literal as "difficult terrain" — which state counts and how two
    /// difficult endpoints combine is entirely the author's expression.
    #[serde(default)]
    pub movement_cost_expression_source: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Template-owned participant declaration (ADR-0132). Open `role_id` maps to
/// one exact equipped-slot selector. Composite identity is `(template_id, role_id)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AffordanceTemplateParticipantRow {
    pub template_id: String,
    pub role_id: String,
    pub slot_id: String,
    pub bundle_id: String,
}

/// Action-economy (batch 2): a per-turn action budget class. The `core`
/// bootstrap ships `standard`/`bonus`/`reaction`/`free`; modders may author
/// their own (e.g. a "swift" channel) with no Rust change (ADR-0018).
/// Mirrors the live `economy_channel_definitions` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EconomyChannelDefinitionRow {
    pub id: String,
    pub bundle_id: String,
    pub pool_ms_per_round: u64,
    /// When this channel's pool resets. The vocabulary is authored, never a
    /// Rust enum (ADR-0018).
    pub resets_on: String,
    /// An economy channel whose actions are usable OFF-TURN — offered and
    /// resolvable through `r_manipulate` even when it is not the actor's turn in a
    /// scoped session (the reaction shape). Default `false` = turn-gated (the D-6
    /// behaviour; every existing class stays unchanged). Set `true` on
    /// `srd/action_class/reaction`. The budget is still consumed off-turn, so the
    /// per-turn allowance stays metered (ADR-0037 D-6 Postscript, 2026-06-28).
    #[serde(default)]
    pub off_turn_eligible: bool,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Action-economy (batch 2): how `r_advance_turn` selects the next acting
/// actor. The `core` bootstrap ships `core/turn/round_robin_scheduler`;
/// exactly one row with `active = true` must exist per world. Mirrors the
/// live `turn_scheduler` table (landed via #433). ADR-0037: adds
/// `order_roll_source` (DSL expression for `RolledOrder` scheduling).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TurnSchedulerRow {
    pub id: String,
    pub bundle_id: String,
    /// Closed set enforced validator-side; `round_robin` is the value this
    /// change ships. ADR-0037 adds `rolled_order`. The kernel dispatches on the
    /// K-5 `SchedulerKind` enum (ADR-0037 D-3; never a closed String column).
    pub scheduler_kind: String,
    /// DSL expression evaluated once per participant at session open when
    /// `scheduler_kind = "rolled_order"`. Empty for `round_robin`. Validated:
    /// non-empty + parseable when `scheduler_kind = "rolled_order"`.
    #[serde(default)]
    pub order_roll_source: Option<String>,
    pub active: bool,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// ADR-0037 D-H: authored session template. Mirrors the live
/// `session_definition` table. Phase D opaque-string columns (`turn_rule`,
/// `visibility_policy`, etc.) pass through verbatim. New columns added by
/// ADR-0037 appear here; existing columns use `serde(default)` to remain
/// optional when authoring non-combat session types.
// NOTE: not `Eq` — the ADR-0045 grid params are `f64` (floats are not `Eq`),
// mirroring the float-bearing place-graph rows above.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionDefinitionRow {
    pub session_def_id: String,
    pub bundle_id: String,
    #[serde(default)]
    pub participants_pred: String,
    /// Actions satisfying this predicate remain allowed while a participant is busy.
    #[serde(default)]
    pub busy_allow_pred: Option<String>,
    /// Actions satisfying this predicate abandon the session before proceeding.
    #[serde(default)]
    pub busy_break_pred: Option<String>,
    #[serde(default)]
    pub initial_state: String,
    #[serde(default)]
    pub legal_moves_csv: String,
    #[serde(default)]
    pub turn_rule: String,
    #[serde(default)]
    pub visibility_policy: String,
    #[serde(default)]
    pub termination_pred: String,
    #[serde(default)]
    pub on_terminate: String,
    #[serde(default)]
    pub stakes_required: String,
    #[serde(default)]
    pub winner_predicate: String,
    /// FK into `turn_scheduler.id`. Empty = use the world-active scheduler.
    /// ADR-0037 D-3/D-4.
    #[serde(default)]
    pub turn_scheduler_id: String,
    /// Fiction milliseconds to advance per rotation wrap. `0` = unbound.
    /// Positive values must be a whole number of configured world ticks.
    #[serde(default, deserialize_with = "u64_flexible")]
    pub round_duration_ms: u64,
    /// Enroll outsiders that target a participant with an economy action.
    /// Default `false`. ADR-0037 D-7 (COND-12).
    #[serde(default)]
    pub auto_join_on_targeting: bool,
    /// Optional FK into `combat_grid_blueprint.blueprint_id`. Empty = no
    /// automatic grid unless the reducer caller passes explicit grid extent.
    #[serde(default)]
    pub combat_grid_blueprint_id: String,
    /// Affordance template copied onto generated tactical-grid edges.
    /// Empty = the session definition does not author grid traversal.
    #[serde(default)]
    pub grid_edge_affordance_template_id: String,
    /// Optional bindings JSON copied onto each generated tactical-grid edge
    /// affordance. Empty = no bindings.
    #[serde(default)]
    pub grid_edge_affordance_bindings_json: String,
    /// Optional Bool DSL predicate deciding which participants are handed the
    /// turn (ADR-0143 D-1). Evaluated per candidate with that participant as
    /// the DSL `actor` subject when `r_advance_turn` picks the next actor;
    /// a false result skips the participant without removing it. Empty =
    /// every participant is eligible (the unfiltered ADR-0037 rotation).
    /// Only actor-subject reads are admitted (MF-VAL-334). Persisted on the
    /// `session_turn_eligibility` satellite, never on `session_definition`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub turn_eligibility_pred: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Reusable session-grid authoring surface. Runtime opens still mint an
/// ephemeral ADR-0052 place graph; this row only supplies the default geometry
/// and generated-edge policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CombatGridBlueprintRow {
    pub blueprint_id: String,
    pub bundle_id: String,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_grid_topology")]
    pub topology: String,
    #[serde(default = "default_grid_cell_size")]
    pub cell_size_m: f64,
    pub edge_affordance_template_id: String,
    #[serde(default = "default_empty_json_object")]
    pub edge_affordance_bindings_json: String,
    #[serde(default = "default_empty_json_array")]
    pub edge_overrides_json: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// ADR-0134 D-1: `distance_metric_definition.metric_id` the minted grid
    /// measures pairwise node distances under (e.g. `core/metric/chebyshev`
    /// for SRD 5.2's "every square is 5 ft"). Omitted ⇒ Euclidean, the
    /// ADR-0053 D-2 formula. An unknown id is a validation error (MF-VAL-332).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_metric_id: Option<String>,
    /// ADR-0134 D-3: the kernel no longer vetoes a step onto an occupied
    /// cell; occupancy is a clause the edge template authors through
    /// `occupants_at(...)`. A blueprint whose `edge_affordance_template_id`
    /// template reads no `occupants_at` therefore lets actors stack, and the
    /// validator refuses that silently permissive shape (MF-VAL-333) unless
    /// the author states the intent here (the ADR-0027 D-1
    /// `accepts_global_storm` pattern: intent lives in a manifest column,
    /// not in a no-op predicate). Default `false`.
    #[serde(default)]
    pub accepts_stacked_occupancy: bool,
}

fn default_grid_topology() -> String {
    "4".to_owned()
}

/// 1.524 m == 5 ft, the SRD default battle-grid square (sk-yze: this
/// default used to be the bare feet figure `5.0`, which — now that the
/// column is metres — would have been a silent 3.28x-too-small default for
/// any row that omits `cell_size_m`).
///
/// f64, not f32, and that is load-bearing rather than incidental. `1.524` has
/// no exact binary representation, and an f32 default is widened back to f64
/// for the emitted JSON schema by whichever runtime does the printing — native
/// renders `1.5240000486373901`, wasm renders `1.52400004863739`. The
/// multiruntime drift test compares those two schemas byte for byte, so a
/// non-representable f32 default makes the wasm and native builds disagree
/// about a value they both hold identically. The old `5.0` default hid this:
/// it IS exactly representable, so both runtimes printed it the same way.
/// Widening the field is the fix that keeps the default honest; rounding it to
/// something f32-clean (1.5) would have silently redefined a 5-ft square.
fn default_grid_cell_size() -> f64 {
    1.524
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EffectDefinitionRow {
    pub effect_def_id: String,
    pub bundle_id: String,
    pub primitives_csv: String,
    #[serde(default = "default_empty_json_object")]
    pub parameters_schema_json: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Authored ambient autonomy job (ADR-0041). `dsl_function` names an
/// effect definition.
/// (the bridge); the validator fences it to autonomy-eligible (ambient) effects
/// per ADR-0004 D-2 — a clock-fire has no acting subject, so it never targets an
/// actor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutonomyJobRow {
    /// Stable namespaced identifier (PK in `autonomy_job`).
    pub job_id: String,
    /// Owning mod bundle (cleared/reinserted on re-apply).
    pub bundle_id: String,
    /// Absolute jobs recur every N fiction milliseconds. Across both firing
    /// modes, `0` = one-shot (fire once, then delete); a non-zero
    /// boundary-armed job rearms for its next matching owner boundary.
    #[serde(deserialize_with = "u64_flexible")]
    pub frequency_ms: u64,
    /// Absolute fiction time at which this job fires. Present only for the
    /// clock-driven mode; boundary-armed jobs leave it absent.
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub next_fire_at_ms: Option<u64>,
    /// Named boundary that fires this job. Present with `owner_actor` only for
    /// the boundary-relative mode; absolute jobs leave it absent.
    #[serde(default)]
    pub fire_at_boundary: String,
    /// Actor owning `fire_at_boundary`. The boundary engine dispatches only
    /// rows matching this owner and boundary.
    #[serde(default, deserialize_with = "u64_flexible")]
    pub owner_actor: u64,
    /// Bounded-recurrence counter. Omitted / `None` = unbounded.
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub repeats_remaining: Option<u64>,
    /// Optional responsible actor for subject-bound autonomy fires. The value is
    /// an `entity.entity_id` (normally the caster captured at schedule time).
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub subject: Option<u64>,
    /// Manifest-authoring alternative to `subject` (authored-hazard-pulses
    /// D-3): the `entity_key` of an `[[entities]]` row in the same manifest,
    /// resolved to its numeric `entity_id` at commit through the same-apply
    /// `entity_ids` map (the idiom `actor_equipment_slots.actor_entity_key` /
    /// `actor_capabilities.actor_entity_key` already use). Empty string =
    /// unused. A row that is unresolvable, or that sets both `subject` and a
    /// non-empty `subject_entity_key`, is rejected at apply. No manifest
    /// author can hardcode a fresh entity's runtime id, so this is the only
    /// way to author a subject-bound job (e.g. a hazard pulse) whose subject
    /// is spawned by the same manifest rather than captured by a cast.
    #[serde(default)]
    pub subject_entity_key: String,
    /// Optional captured spatial anchor for subject-bound autonomy fires. The
    /// value is a `place_node.node_id` captured when the job is scheduled.
    #[serde(default)]
    pub anchor_place_node_id: Option<String>,
    /// When true, the dispatcher cancels this job if `subject` is absent or no
    /// longer alive at fire time.
    #[serde(default)]
    pub requires_subject_alive: bool,
    /// DSL handle — resolves to an `effect_definition.effect_def_id` at fire time.
    pub dsl_function: String,
    /// JSON parameters forwarded to the effect on each fire. `""` if parameterless.
    #[serde(default)]
    pub payload_json: String,
    /// Dispatch order when several jobs share a tick (lower = earlier).
    #[serde(default)]
    pub priority: i32,
    /// When `false`, the dispatcher skips this job.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Origin for cancel-by-source lookup. `"manifest"` for authored rows.
    #[serde(default = "default_manifest_source")]
    pub source: String,
    /// Human-readable description for operator tooling.
    #[serde(default)]
    pub description: String,
    /// Author-set curation bit — a successful fire stamps `world_event.notable`.
    #[serde(default)]
    pub notable: bool,
}

fn default_manifest_source() -> String {
    "manifest".to_owned()
}

/// Authored trace-kind vocabulary (ADR-0043). `kind_id` is an open
/// string with no Rust match (ADR-0018 compliant); `location_kinds` is validated
/// against the registered scope set (`spellkave-validate::trace_kinds`).
///
/// Field names mirror the table columns 1:1 (deliberate mapping, not drift —
/// per LR-51(c)).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceKindDefinitionRow {
    /// Stable namespaced identifier (PK in `trace_kind_definition`).
    pub kind_id: String,
    /// Owning mod bundle (cleared/reinserted on re-apply).
    pub bundle_id: String,
    /// Comma-joined registered scopes the trace can attach to (e.g.
    /// `"place_node,place_edge"`). Each entry must be a registered scope id.
    pub location_kinds: String,
    /// Canonical JSON schema for the trace payload: a flat object mapping a
    /// required payload key to its type name (`{"text":"string"}`), a `?`
    /// suffix marking the key optional (`{"addressee":"integer?"}`). `""` if
    /// the kind carries no structured payload. Enforced at inscription time by
    /// `inscribe::validate_payload_against_schema` — the author declares what
    /// must hold, the kernel makes it hold.
    #[serde(default)]
    pub schema_json: String,
    /// How many ticks after inscription this kind of trace stays admissible to
    /// an ARRIVING observer (ADR-0040 seam B, `observer_knowledge::
    /// learn_at_node`). Absent = unbounded: the trace is a standing mark that
    /// anyone who later walks in can find — footprints, a slime trail, a
    /// scorch. `0` = perceptible only at the moment it is inscribed (seam A
    /// alone): an EVENT, not a mark — a sound, heard by whoever was present
    /// and by nobody who arrives afterwards (bead sk-n16).
    ///
    /// This is NOT a lifetime. The trace row survives regardless — the room
    /// keeps its history for an elevated/DM read-back; what expires is only
    /// the chance to perceive it by walking in. Deleting the row instead
    /// (`Inscribe.expires_at_tick`) would un-hear listeners who already heard,
    /// because the player read path resolves remembered traces back through
    /// the `trace` table and skips dangling ids.
    #[serde(default)]
    pub perceptible_for_ticks: Option<u64>,
    /// Human-readable description for authors and operator tooling.
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsequenceTemplateRow {
    pub template_id: String,
    pub bundle_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub effect_refs_csv: String,
    #[serde(
        default = "default_empty_json_array",
        skip_serializing_if = "is_default_empty_json_array"
    )]
    pub traces_json: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsequenceFrameWriteRow {
    pub template_id: String,
    pub ordinal: u32,
    /// Externally tagged frame operation; the unit BreakPlaceSeal is a JSON/TOML string.
    pub frame_write: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionDefinitionRow {
    pub action_id: String,
    pub bundle_id: String,
    pub required_capability: String,
    pub on_success: String,
    #[serde(default)]
    pub on_failure: String,
    #[serde(default)]
    pub resolution_mode: ActionResolutionModeRow,
    /// Authored Spellkave-DSL boolean expression evaluated once at the
    /// Stochastic resolution seam (replaces dormant resolution_check/dc).
    #[serde(default)]
    pub resolution_predicate_source: String,
    /// Open authored scope for actor-side resolution modifiers. Empty means
    /// the action has no narrower scope beyond the universal actor axis.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resolution_scope: String,
    /// Explicit author assertion that this action's target writes are
    /// non-steering and therefore do not require a target claim. False keeps
    /// the conservative runtime classifier. Marking a hostile effect true is
    /// a content-authoring bug, not a kernel inference.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub non_steering: bool,
    pub display_name: String,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub cost_time_ms: u64,
    #[serde(default)]
    pub session_def_id: String,
    /// Interrupted progress may be credited on the next matching promotion.
    #[serde(default)]
    pub resumable: bool,
    /// Action-economy (batch 2, slice E): the §8.4 WHO-may-react axis — a DSL
    /// predicate over a candidate reactor's own fields/capabilities. Absent →
    /// not a reaction candidate. Consumed by the deferred reaction producer
    /// (slice E). Warden cond. 3 retired a MISPLACED affordance-cache field that
    /// was named `reaction_trigger`; the §8.4 `reaction_trigger` axis below is a
    /// distinct, correctly-placed `action_definition` field (slice F-impl).
    #[serde(default)]
    pub reaction_eligibility: Option<String>,
    /// Action-economy: how long the reaction window stays open, in ms. Absent
    /// with a non-null `reaction_eligibility` → an implementation default.
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub reaction_window_ms: Option<u64>,
    /// Action-economy (batch 2, slice F-impl): the §8.4 WHAT-provokes axis — a
    /// DSL predicate over the TRIGGERING event's generic facts (`$event.kind`,
    /// `$event.detail.<field>`), distinct from `reaction_eligibility` (WHO).
    /// Absent / empty → provoke on every co-located trigger (lenient, slice-E
    /// behaviour preserved). The producer evaluates it ONCE per event against
    /// the populated intent facts; a Global-locality read (`world.*`/`session.*`)
    /// is rejected at validation (MF-VAL-239 — `$event.*` is bounded, not Global).
    #[serde(default)]
    pub reaction_trigger: Option<String>,
    /// Whether this reaction may include the acting actor as its own reactor.
    #[serde(default)]
    pub self_eligible: bool,
    /// ADR-0075 (amends ADR-0060 D-3): this manipulate action targets ONLY the
    /// acting actor — the materialiser fanout and the `r_manipulate` act-gate
    /// both exclude every distinct co-located entity (and every place/world
    /// target). Implies self-admission (no need to also set `self_eligible`).
    /// Absent/false = unchanged ADR-0060 behaviour; additive, existing rows
    /// omit it and stay byte-identical.
    #[serde(default)]
    pub self_only: bool,
    /// Action-economy (P2, ADR-0037 D-6 second Postscript): an open FK into
    /// `economy_channel_definitions.id` — the channel a triggered (`r_react`)
    /// reaction consumes. Absent → unmetered (byte-identical Tier-1 producer
    /// behaviour). When present and resolving to an off-turn-eligible class, the
    /// reaction producer opens a `pending_resolution` window only if the reactor
    /// has remaining budget this round, and `commit_pending_resolution` consumes
    /// the budget (capacity-guarded). Mirrors the content-neutral
    /// `AffordanceTemplateRow.spends_channel`; the kernel reads the id +
    /// the channel `off_turn_eligible` bool, never the literal "reaction".
    #[serde(default)]
    pub spends_channel: Option<String>,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub enum ActionResolutionModeRow {
    #[default]
    Deterministic,
    Stochastic,
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceDefinitionRow {
    pub resource_definition_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub value_kind: String,
    pub min_value: i64,
    pub max_value: i64,
    pub recovery_rule: String,
    /// Value used to seed a fresh `object_state` row when `ModifyResource`
    /// fires against an `(entity, resource)` pair that has no live row yet.
    /// `None` → fall back to `min_value` (today's semantics for charges /
    /// gold / temp-HP-shaped resources). `Some(v)` → seed at `v` (HP-shaped
    /// resources author `Some(max_value)`).
    #[serde(default)]
    pub initial_value: Option<i64>,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RollTableRow {
    pub roll_table_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub result_kind: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RollTableEntryRow {
    pub roll_table_entry_id: String,
    pub roll_table_id: String,
    pub authored_order: u32,
    pub weight: u32,
    pub result_ref: String,
}

/// Authoring shape for `modifier_target_definition` (Item 15 / Issue #314).
///
/// Modders add new composition-axis vocabulary by writing
/// `[[modifier_target_definitions]]` rows in their bundle TOML. The kernel
/// composes by `shape` (`Numeric` / `Mode` / `Reference`), not by
/// `target_id`. Core targets ship as seeded rows under `bundle_id="core"`
/// — modders should NOT re-author those.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModifierTargetDefinitionRow {
    pub target_id: String,
    pub bundle_id: String,
    /// Fold algebra. Must be one of `"Numeric"` / `"Mode"` / `"Reference"`
    /// (the closed `ModifierTargetShape` enum). The manifest loader rejects
    /// unknown values.
    pub shape: String,
    pub display_name: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// `[[stacking_discipline_definitions]]` rows in a bundle TOML. A condition's
/// `stacking_rule` resolves to one of these rows; the kernel evaluates the
/// DSL outcomes instead of branching on the discipline id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StackingDisciplineDefinitionRow {
    pub discipline_id: String,
    pub bundle_id: String,
    pub display_name: String,
    /// Int-valued DSL expression over the existing row state. Available facts:
    /// `stack_count`, `expires_at_tick`, and `world_tick`.
    pub stack_count_outcome_source: String,
    /// Int-valued DSL expression over the existing row state. `-1` means
    /// "leave the existing expiry unchanged"; non-negative values set a tick.
    pub expires_at_tick_outcome_source: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// `[[condition_remove_policy_definitions]]` rows in a bundle TOML. A
/// condition's `remove_time_policy` resolves to one of these rows; the removal
/// path evaluates `remove_predicate` instead of branching on policy ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConditionRemovePolicyDefinitionRow {
    pub policy_id: String,
    pub bundle_id: String,
    pub display_name: String,
    pub remove_predicate_source: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

fn default_condition_stacking_rule() -> String {
    // The no-op discipline: one active row, never stacks. Symmetric with
    // `default_condition_remove_time_policy` so both fields are optional in
    // authored TOML and a condition that cares about neither stays terse.
    "core/stacking/single".to_string()
}

fn default_condition_remove_time_policy() -> String {
    "core/remove_policy/independent_lifetime".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModifierTemplateRow {
    pub modifier_template_id: String,
    pub bundle_id: String,
    pub applies_to: String,
    pub display_name: String,
    pub predicate_source: String,
    pub modifier_expression_source: String,
    #[serde(default)]
    pub priority: i32,
    /// CR-3 / ADR-0038 lift: the `ModifierDelta` shape discriminant — was the
    /// `delta_kind=<X>` behaviour kv-tag. Closed value-set (`Advantage`,
    /// `Disadvantage`, `Plus`, `Times`, `FlatSet`, `Replace`); the validator
    /// rejects any other value (MF-VAL-243). Absent → `Plus`.
    #[serde(default = "default_delta_kind")]
    pub delta_kind: String,
    /// CR-3 / ADR-0038 lift: for `delta_kind = "Replace"`, the modifier_template
    /// id whose shape replaces this one — was the `replace_template=<id>` kv-tag.
    /// Open ref (absent → unset). Mirror of `affordance_template`'s action-economy
    /// `Option<String>` authoring fields.
    #[serde(default)]
    pub replace_template_id: Option<String>,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Default `modifier_template.delta_kind`: the kernel's `ModifierDeltaTag::Plus`
/// arm, preserving the pre-CR-3 `tag_value(...).unwrap_or("Plus")` behaviour.
fn default_delta_kind() -> String {
    "Plus".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConditionDefinitionRow {
    pub condition_definition_id: String,
    pub bundle_id: String,
    pub display_name: String,
    #[serde(default = "default_condition_stacking_rule")]
    pub stacking_rule: String,
    #[serde(default = "default_condition_remove_time_policy")]
    pub remove_time_policy: String,
    pub gate_predicate_source: String,
    #[serde(default)]
    pub modifier_template_id: String,
    #[serde(default)]
    pub hooks: Vec<ConditionHookRow>,
    #[serde(default = "default_true")]
    pub observable: bool,
    /// Capability ids granted to the bearer while it carries an active
    /// instance of this condition (Brief 11 D8).
    ///
    /// sk-syo made this field REAL. It shipped as dead metadata — written by
    /// the manifest committer, read by nothing — which is worse than absent,
    /// because an author trusts a field that validates and seeds cleanly and
    /// then silently does nothing. It also shipped as a CSV-in-a-`String`;
    /// it is now the list it always was.
    ///
    /// Why this is not redundant with the `ApplyStatus` + `actor.status(...)`
    /// idiom: an `affordance_template` carries a STRUCTURAL `capability_id`
    /// gate — without the capability
    /// the affordance is never offered at all. A status is only a fact that
    /// each predicate must remember to read. A ring that grants a spell is
    /// therefore expressible here and NOT expressible as a status without
    /// hand-wiring the gate into every affordance.
    #[serde(default)]
    pub grants_capability_while_active: Vec<String>,
    /// Capability ids SUPPRESSED on the bearer while it carries an active
    /// instance of this condition — the negative twin of
    /// `grants_capability_while_active`, and the reason sk-syo exists.
    ///
    /// The two are not interchangeable: granting ADDS to an open vocabulary,
    /// suppressing SUBTRACTS from it. Expressing a grant as a suppression
    /// would mean first giving every entity every conceivable capability and
    /// then masking, which an open vocabulary cannot do.
    ///
    /// Applies to the bearer AS AN ACTOR only. A condition that stops a
    /// creature acting must not stop it being acted UPON: "Speak with Dead"
    /// is the caster's affordance targeting a corpse that is itself
    /// Incapacitated, and must stay authorable. `capability_ids_for_actor`
    /// enforces that split — the Target-subject fact pool never applies it.
    #[serde(default)]
    pub suppresses_capability_while_active: Vec<String>,
    /// Condition cascade — SRD-canonical "implies" edge. When this condition
    /// is applied to a target via `ApplyStatus`, each id listed here is also
    /// applied to the same target (recursively, with cycle + duplicate
    /// guards). Empty `Vec` = no cascade.
    ///
    /// Probe motivation: SRD §490 ff. — Paralyzed implies Incapacitated;
    /// Unconscious implies Incapacitated + Prone; Petrified implies
    /// Incapacitated. The implementing slice (srd-conformance-probe Phase
    /// 24 / Direction R) lands the field, the manifest wiring, and the
    /// cascading insert in `apply_status::execute`. The remove path and
    /// reference-counting semantics are deferred to a follow-up
    /// (issue #290 Axis 3 open design question).
    #[serde(default)]
    pub implies: Vec<String>,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

fn default_tag_source() -> String {
    "manifest".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionTagRow {
    pub entity_def_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObjectStateDefinitionTagRow {
    pub state_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDefinitionTagRow {
    pub capability_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NicheDefinitionTagRow {
    pub niche_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutcomeTemplateTagRow {
    pub outcome_template_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionDefinitionTagRow {
    pub action_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResourceDefinitionTagRow {
    pub resource_definition_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RollTableTagRow {
    pub roll_table_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RollTableEntryTagRow {
    pub roll_table_entry_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModifierTemplateTagRow {
    pub modifier_template_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConditionDefinitionTagRow {
    pub condition_definition_id: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

/// Authoring shape of a relational INSTANCE tag (`[[entity_tag]]`).
///
/// The instance sibling of the `*_definition_tag` rows above: those anchor on a
/// definition id, this anchors on an authored entity instance via `entity_key`
/// (an `[[entities]]` row in the same manifest). At apply the key resolves to
/// the entity id and an `entity_tag` row is inserted. This replaces the
/// removed inline `world_objects.tags` CSV form (CR-4, predating the #1968
/// B-10 retirement of the `[[world_objects]]` row kind itself) and is the
/// CR-7 write leg; the `has_tag` DSL read leg lands in a later slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityTagRow {
    pub entity_key: String,
    pub tag: String,
    pub bundle_id: String,
    #[serde(default = "default_tag_source")]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataNamespaceRow {
    pub namespace: String,
    /// Who may read this namespace (ADR-0130 D-3): a `VisibilityClass` token,
    /// the strict owner token `owned`, or a `capability_definition` id.
    pub audience: String,
    pub bundle_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Reference content delivered only on request (ADR-0130 D-4). Absent → `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub on_demand: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataRow {
    pub target_kind: String,
    pub target_ref: String,
    pub namespace: String,
    pub payload_json: String,
    pub bundle_id: String,
    #[serde(default = "default_manifest_source")]
    pub source: String,
}

/// Authoring shape of a single hook on a `condition_definition`.
///
/// Two shapes are admitted (issue #257, srd-conformance-probe Direction B):
///
/// * **Inline-DSL (preferred).** Author the detail filter and modifier
///   expression as standalone DSL source strings via `detail_filter_source`
///   and `expression_source`. At manifest admission the strings are parsed
///   to sub-`Expression`s and grafted onto the parent
///   `gate_predicate_source`'s node id space (see
///   `spellkave_dsl::Expression::graft`); their resolved root ids become
///   the `detail_filter` / `expression` node ids that the runtime evaluator
///   walks. Authors never see the numeric ids.
///
/// * **Numeric (legacy).** Provide `detail_filter` / `expression` as raw
///   `DslNodeId` offsets into the parsed `gate_predicate_source` AST. This
///   shape exists for in-crate unit tests that build hook rows
///   programmatically; production fixtures should use the inline-DSL form.
///
/// Exactly one of `expression` / `expression_source` must be set, and at most
/// one of `detail_filter` / `detail_filter_source`. The validator and the
/// manifest loader reject all other combinations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConditionHookRow {
    pub event_kind: String,
    #[serde(default)]
    pub detail_filter: Option<u32>,
    #[serde(default)]
    pub detail_filter_source: Option<String>,
    #[serde(default)]
    pub expression: Option<u32>,
    #[serde(default)]
    pub expression_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetConditionRow {
    pub target_kind: String,
    #[serde(deserialize_with = "u64_flexible")]
    pub target_id: u64,
    pub condition_definition_id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default = "default_stack_count")]
    pub stack_count: u32,
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub expires_at_tick: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityRow {
    pub entity_key: String,
    pub bundle_id: String,
    #[serde(default)]
    pub entity_def_id: String,
    pub name: String,
    pub kind: EntityKindRow,
    pub place_node_id: String,
    #[serde(default = "default_quantity", deserialize_with = "u64_flexible")]
    pub quantity: u64,
    #[serde(default)]
    pub owned_by_entity_key: String,
    /// Authored description (#1968) — e.g. "the guard with the limp". Open
    /// string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityContainmentRow {
    pub entity_key: String,
    pub container_entity_key: String,
    #[serde(default)]
    pub slot_id: String,
    #[serde(default)]
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObjectStateRow {
    pub entity_key: String,
    pub state_id: String,
    /// ADR-0120 read-visibility override for this concrete instance row.
    /// `None` (field omitted) resolves at manifest commit to the owning
    /// `object_state_definition.default_visibility` (falling back further to
    /// `Public` if no definition row exists) — see
    /// `object_state_default_visibility` (#1935). `Some(_)` always wins.
    #[serde(default)]
    pub visibility: Option<String>,
    /// Boolean carrier — legacy form. Default `true` preserves pre-§12.5b
    /// authoring where `[[object_state]]` rows were always flags.
    #[serde(default = "default_true")]
    pub value_bool: bool,
    /// Numeric carrier (§12.5b — Brief 09 Slicing Step 1 lift to DslValue).
    /// When `Some(_)`, takes precedence over `value_bool` at manifest commit.
    /// Enables modder authoring of `value_int = 10` for HP / mana / charges.
    #[serde(default)]
    pub value_int: Option<i64>,
    /// String carrier (§12.5b). When `Some(_)` and `value_int` is `None`,
    /// commits as `DslValue::Text(_)`. Enables authoring of `value_text = "running"`
    /// for machine status etc. (Brief 12 machine-state-dissolution Example).
    #[serde(default)]
    pub value_text: Option<String>,
    pub source_template_id: String,
    pub bundle_id: String,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub expires_at_tick: u64,
}

/// Per-instance resource-bound OVERRIDE (#30 / sk-ct8.2 "per-creature HP
/// maximum"). Row absent for an `(entity, resource_definition)` pair ⇒ the
/// bundle-wide `resource_definitions.{min_value,max_value}` bounds apply
/// unchanged (inheritance); row present ⇒ `ModifyResource`'s clamp AND the
/// DSL `Op::ReadResourceMax`/`Op::ReadResourceMin` leaf ops
/// (`actor.resource_max("<id>")`/`actor.resource_min("<id>")`) consult this
/// row instead. Mirrors [`ObjectStateRow`]'s per-instance shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityResourceBoundRow {
    pub entity_key: String,
    pub resource_definition_id: String,
    pub min_value: i64,
    pub max_value: i64,
    pub bundle_id: String,
}

/// Definition-level initial state-value seed (#729). Each row binds an initial
/// state-value to an `entity_definition`; at admission/spawn every placed
/// instance inherits an `object_state` row. Mirrors `ObjectStateRow`'s typed
/// carriers (no `value_kind` string enum — ADR-0018). Carrier precedence at
/// commit: `value_int` > `value_text` > `value_bool`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionInitialStateRow {
    pub entity_def_id: String,
    pub state_id: String,
    #[serde(default = "default_true")]
    pub value_bool: bool,
    #[serde(default)]
    pub value_int: Option<i64>,
    #[serde(default)]
    pub value_text: Option<String>,
    pub bundle_id: String,
}

/// Definition-level innate capability grant (#728). Each row grants an
/// unconditional capability to an `entity_definition`; at admission/spawn every
/// placed instance is born holding a `capability` row keyed on its `entity_id`
/// (entity-anchored per ADR-0028 erratum). A seed table, NOT a CSV column on
/// `EntityDefinitionRow` (ADR-0028 freezes the `grants_capability_while_*` CSV
/// grant surface as legacy).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionInitialCapabilityRow {
    pub entity_def_id: String,
    pub capability_id: String,
    #[serde(default)]
    pub extent_kind_ref: String,
    #[serde(default)]
    pub extent_ref: String,
    pub bundle_id: String,
}

/// sk-bwd: the IDENTITY→capability bridge row — "while an entity bears `tag`,
/// it may act with `capability_id`". The grant lasts while the tag is present.
///
/// `tag` is an OPEN string, resolved against the union of `entity_tag` and
/// `entity_definition_tag`. It is deliberately NOT validated against
/// `tag_value_definition`. That vocabulary is advisory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TagCapabilityGrantRow {
    pub tag: String,
    /// FK into `capability_definition.capability_id`.
    pub capability_id: String,
    pub bundle_id: String,
}

/// Applies the modifier to the wearer while the matching item is equipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionEquipModifierRow {
    /// FK into `entity_definition.entity_def_id` — the item.
    pub entity_def_id: String,
    /// FK into `modifier_template.modifier_template_id`.
    pub modifier_template_id: String,
    pub bundle_id: String,
}

/// Definition-level initial condition seed. Each row binds a
/// `condition_definition_id` (and optional `duration_ticks`) to an
/// `entity_definition`; at admission/spawn every placed instance is born with a
/// `target_condition` row. The server table existed but was previously absent
/// from this `Manifest` author struct; this row kind makes it authorable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionInitialConditionRow {
    pub entity_def_id: String,
    pub condition_definition_id: String,
    /// Duration in world ticks. `0` = no expiry (permanent condition).
    #[serde(default, deserialize_with = "u64_flexible")]
    pub duration_ticks: u64,
    pub bundle_id: String,
}

/// sk-84i.2 (widened G-1): one authored DSL-source slot on an
/// `entity_definition` (e.g. `slot = "srd/damage"`, `source = "2d8"`).
/// Validated at merge-gate (`MF-VAL-318`) and manifest commit like
/// `predicate_source`/`outcome_expression_source` (a hard parse error fails
/// the commit). The server persists the raw `source` plus a commit-time
/// macro-expanded AST; act time evaluates the cached tree (#2218).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionItemDslSourceRow {
    pub entity_def_id: String,
    /// Content-side slot id (e.g. `"srd/damage"`) — opaque to the kernel.
    pub slot: String,
    /// Raw DSL source text (e.g. `"2d8"`).
    pub source: String,
    pub bundle_id: String,
}

/// Definition-level initial containment (sk-84i.3, parent sk-84i). Each row
/// says: "when an entity of this `entity_def_id` is placed or spawned, spawn
/// `quantity` of `contained_entity_def_id` and carry it (loose containment —
/// no `slot_id`/`direction`, mirroring `entity_containment`'s carried-item
/// shape) inside the new instance." The contained definition's OWN seeds
/// (state/capability/condition/containment/equipment) apply recursively to
/// the spawned contained instance, so a bandit's quiver can itself author
/// bolts.
///
/// Quantity rule (mirrors `Entity.quantity` / ADR-0028 D-6, see
/// `entity::validate_quantity_stack_tag`): when the contained definition is
/// `stackable = true`, ONE instance is spawned carrying `quantity` as its
/// stack count. When it is NOT stackable, `quantity` SEPARATE instances are
/// spawned (each with its own `entity_id`, stack count 1) — the same
/// distinction `[[entities]].quantity` already draws for authored instances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionInitialContainmentRow {
    pub entity_def_id: String,
    /// FK into `entity_definition.entity_def_id` — the definition to spawn
    /// and contain.
    pub contained_entity_def_id: String,
    /// How many to spawn (see quantity rule above). Default `1`.
    #[serde(default = "default_quantity", deserialize_with = "u64_flexible")]
    pub quantity: u64,
    pub bundle_id: String,
}

/// Definition-level initial equipment (sk-84i.3, parent sk-84i). Each row
/// says: "when an entity of this `entity_def_id` is placed or spawned, spawn
/// one instance of `equipped_entity_def_id`, contain it (loose carriage, same as
/// `entity_definition_initial_containment`), and equip it into `slot_id` —
/// inserting an `entity_equipment_slot` row on the new instance."
///
/// Use a named slot for equipment that should be immediately available.
/// FRESH instance, never re-equips an already-live entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityDefinitionInitialEquipmentRow {
    pub entity_def_id: String,
    /// FK into `entity_definition.entity_def_id` — the item definition to
    /// spawn and equip.
    pub equipped_entity_def_id: String,
    /// FK into `equipment_slot_definition.slot_id`.
    pub slot_id: String,
    pub bundle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorSpatialLocationRow {
    pub entity_key: String,
    pub place_node_id: String,
    pub bundle_id: String,
    #[serde(default)]
    pub selection_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorRow {
    pub entity_key: String,
    pub bundle_id: String,
    pub name: String,
    /// Authored description (#1968) — e.g. "the guard with the limp". Open
    /// string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorIdentityRow {
    pub entity_key: String,
    pub bundle_id: String,
    pub identity: String,
    pub role: String,
    /// Omitted authored bindings are Service; runtime claims are Person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityKindRow {
    Player,
    Npc,
    Item,
    Place,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Brief 11 D10: `actor_entity_key` is the manifest key for the body entity
/// (resolved to entity_id at apply time). D5: `proficiency` dropped — binary
/// capability gate only; graduation lives in an attribute block.
pub struct ActorCapabilityRow {
    pub actor_entity_key: String,
    pub capability_id: String,
    pub source: String,
    #[serde(default)]
    pub source_args_hash: String,
    pub bundle_id: String,
    /// ADR-0120 read-visibility override for this concrete grant (mirrors
    /// `[[object_state]].visibility`). `None` (field omitted) resolves at
    /// manifest commit to the owning `capability_definition.default_visibility`
    /// (falling back further to `Public` if no definition row exists) — see
    /// `capability_default_visibility` (#1935). `Some(_)` always wins.
    #[serde(default)]
    pub visibility: Option<String>,
    /// ADR-0007 D-4 authored see-as scope reference. Names a place graph;
    /// omitted or empty means the grant broadens no visibility.
    #[serde(default)]
    pub extent_ref: String,
    /// Open vocabulary kind for `extent_ref`, resolving through
    /// `extent_kind_definition.kind_id`. Empty preserves legacy place-graph
    /// interpretation for existing manifests.
    #[serde(default)]
    pub extent_kind_ref: String,
    /// #1938: the read-visibility ceiling THIS grant elevates a body to when
    /// reached only via broadening (never via direct control) — `""`/absent
    /// means `Hidden`-only elevation (the safe default); `"internal"` (or
    /// `"full"`) means `Internal` + ungated `Discoverable`, today's ADR-0081
    /// D-3 behavior, reserved for grants that are actually DM/oversight-shaped.
    /// Meaningless without a non-empty `extent_ref`.
    #[serde(default)]
    pub elevation_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EquipmentSlotDefinitionRow {
    pub slot_id: String,
    pub bundle_id: String,
    pub label: String,
    #[serde(default)]
    pub default_compatibility_predicate: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemDefinitionRow {
    pub item_def_id: String,
    pub bundle_id: String,
    #[serde(default)]
    pub slot_id: String,
    #[serde(default)]
    pub slot_compatibility_predicate: String,
    /// Authored description (#1968) — open string, never load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorEquipmentSlotRow {
    pub actor_entity_key: String,
    pub slot_id: String,
    pub item_key: String,
}

// MachinePortRow — removed; vocabulary folded into entity_port_definition (Phase 2).
// MachinePortInventoryRow — removed; folded into entity_containment with non-empty slot_id.

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpatialNicheAttachmentRow {
    pub bundle_id: String,
    #[serde(default)]
    pub place_node_id: String,
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub place_edge_id: Option<u64>,
    pub niche_id: String,
    #[serde(default = "default_public")]
    pub visibility: String,
    #[serde(default = "default_true")]
    pub condition_bool: bool,
    #[serde(default)]
    pub ecological_invariant: bool,
    /// Optional action dispatched after successful entry into `place_node_id`.
    /// Authored only as a pair with `on_entry_target_entity_key`; empty means
    /// the attachment affects locality only.
    #[serde(default)]
    pub on_entry_action_id: String,
    /// Manifest entity key bound as TargetActor for `on_entry_action_id`.
    /// Resolved to a numeric entity id during manifest admission.
    #[serde(default)]
    pub on_entry_target_entity_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpatialStateDefinitionRow {
    pub state_id: String,
    pub bundle_id: String,
    pub value_type: String,
    pub writer_policy: String,
    pub freshness_rule: String,
    pub merge_rule: String,
    pub lod_policy: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpatialEnvironmentStateRow {
    /// Owning bundle. Bundle-scoped manifest clear deletes this row by
    /// `bundle_id` directly, so authoring against a foreign-bundle definition
    /// is still cleaned up by this bundle's re-apply.
    pub bundle_id: String,
    #[serde(default)]
    pub place_node_id: String,
    #[serde(default, deserialize_with = "option_u64_flexible")]
    pub place_edge_id: Option<u64>,
    /// Author-facing edge key. Resolved to `place_edge_id` during manifest
    /// commit; mutually exclusive with `place_edge_id` and `place_node_id`.
    #[serde(default)]
    pub place_edge_key: String,
    #[serde(default)]
    pub location_def_id: String,
    pub state_id: String,
    #[serde(default)]
    pub value_bool: bool,
    #[serde(default)]
    pub value_int: Option<i64>,
    #[serde(default)]
    pub value_text: Option<String>,
    #[serde(default = "default_public")]
    pub visibility: String,
    #[serde(default, deserialize_with = "u64_flexible")]
    pub updated_tick: u64,
}

fn default_empty_json_array() -> String {
    "[]".to_owned()
}

fn default_empty_json_object() -> String {
    "{}".to_owned()
}

fn default_quantity() -> u64 {
    1
}

fn default_stack_count() -> u32 {
    1
}

fn default_public() -> String {
    "Public".to_owned()
}

fn default_true() -> bool {
    true
}

/// `skip_serializing_if` helper: omit a `bool` field from rendered output when
/// it equals its `false` default (omit-the-default render discipline, #836-A).
/// Parse still re-supplies it via the field's `#[serde(default)]`.
fn is_false(value: &bool) -> bool {
    !*value
}

/// `skip_serializing_if` helper: omit a `traces_json`-shaped field when it
/// equals its `"[]"` default (#836-A). Pairs with
/// `#[serde(default = "default_empty_json_array")]`.
fn is_default_empty_json_array(value: &str) -> bool {
    value == "[]"
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemaVersion(pub Version);

impl SchemaVersion {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(Version::new(major, minor, patch))
    }

    pub fn parse(value: &str) -> Result<Self, semver::Error> {
        Version::parse(value).map(Self)
    }

    pub fn major(&self) -> u64 {
        self.0.major
    }

    pub fn minor(&self) -> u64 {
        self.0.minor
    }

    pub fn patch(&self) -> u64 {
        self.0.patch
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for SchemaVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        SchemaVersion::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for SchemaVersion {
    fn schema_name() -> String {
        "SchemaVersion".to_owned()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let mut schema = String::json_schema(gen);
        if let schemars::schema::Schema::Object(ref mut obj) = schema {
            obj.metadata().description =
                Some("Manifest schema version in MAJOR.MINOR.PATCH format".to_owned());
            obj.string().pattern = Some(r"^\d+\.\d+\.\d+$".to_owned());
        }
        schema
    }
}

pub fn manifest_json_schema() -> schemars::schema::RootSchema {
    schemars::schema_for!(Manifest)
}

pub fn manifest_json_schema_pretty() -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&manifest_json_schema())
}

/// Accepted event kind paths for condition-hook admission (ADR-0024).
pub const ALL_EVENT_KIND_PATHS: &[&str] = &[
    "Tick",
    "Manual.CreateEntity",
    "Manual.MoveEntity",
    "Manual.TakeItem",
    "Manual.DropItem",
    "Manual.ClockAdvance",
    "Manual.PaceTime",
    "Manual.SpawnEntityDefinition",
    "Autonomy.Job",
    "Layer2.Frame",
    "Layer2.SessionOpen",
    "Layer2.SessionMove",
    "Layer2.SessionClose",
    "Layer2.React",
    "Layer2.ApplyConsequence",
    "Layer2.ResourceDelta",
    "Layer2.ConditionDelta",
    "Layer2.TurnStart",
    "Layer2.TurnEnd",
    "Layer2.ChannelSpend",
    "Layer2.BoundaryFired",
    "Layer2.ChannelWindowRefresh",
    "Layer2.ActionRechargeFired",
];

pub const CONDITION_HOOK_INTENT_KIND_MODIFIER_DELTA: &str = "ModifierDelta";
pub const CONDITION_HOOK_INTENT_KIND_MODIFY_RESOURCE: &str = "ModifyResource";
pub const CONDITION_HOOK_INTENT_KIND_APPLY_STATUS: &str = "ApplyStatus";
pub const CONDITION_HOOK_INTENT_KIND_REMOVE_CONDITION: &str = "RemoveCondition";
pub const CONDITION_HOOK_INTENT_KIND_EMIT_TRACE: &str = "EmitTrace";
pub const CONDITION_HOOK_INTENT_KIND_INVOKE_OUTCOME_GRAPH: &str = "InvokeOutcomeGraph";
pub const CONDITION_HOOK_INTENT_KIND_OUTCOME_SEQUENCE: &str = "OutcomeSequence";
pub const CONDITION_HOOK_INTENT_KIND_ARBITRATE: &str = "Arbitrate";

pub const SIDE_EFFECT_INTENT_KINDS: &[&str] = &[
    CONDITION_HOOK_INTENT_KIND_MODIFY_RESOURCE,
    CONDITION_HOOK_INTENT_KIND_APPLY_STATUS,
    CONDITION_HOOK_INTENT_KIND_REMOVE_CONDITION,
    CONDITION_HOOK_INTENT_KIND_EMIT_TRACE,
    CONDITION_HOOK_INTENT_KIND_INVOKE_OUTCOME_GRAPH,
    CONDITION_HOOK_INTENT_KIND_OUTCOME_SEQUENCE,
    CONDITION_HOOK_INTENT_KIND_ARBITRATE,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComposerRegistration {
    pub event_kind: &'static str,
    pub intent_kind: &'static str,
}

/// Returns every `(event_kind, intent_kind)` pair Phase B can consume.
///
/// Side-effect intents flow through the consequence-template pipeline and are
/// therefore registered for every admitted event kind. `ModifierDelta` intents
/// require a downstream production roll-resolution path (close-behavior-loop
/// tasks 3.1–3.5); only `Layer2.ApplyConsequence` currently has one, so other
/// event kinds emitting `ModifierDelta` are rejected at admission with a
/// pointer to this registry.
pub fn composer_registrations() -> &'static [ComposerRegistration] {
    use std::sync::OnceLock;
    static SET: OnceLock<Vec<ComposerRegistration>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut out =
            Vec::with_capacity(ALL_EVENT_KIND_PATHS.len() * SIDE_EFFECT_INTENT_KINDS.len() + 1);
        for event_kind in ALL_EVENT_KIND_PATHS {
            for intent_kind in SIDE_EFFECT_INTENT_KINDS {
                out.push(ComposerRegistration {
                    event_kind,
                    intent_kind,
                });
            }
        }
        out.push(ComposerRegistration {
            event_kind: "Layer2.ApplyConsequence",
            intent_kind: CONDITION_HOOK_INTENT_KIND_MODIFIER_DELTA,
        });
        out
    })
    .as_slice()
}

pub fn is_composer_registered(event_kind: &str, intent_kind: &str) -> bool {
    composer_registrations()
        .iter()
        .any(|reg| reg.event_kind == event_kind && reg.intent_kind == intent_kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_descriptions_are_required_and_default_manifest_has_no_role_rows() {
        let definition = serde_json::json!({"role_id":"house/gm", "bundle_id":"house",
            "display_name":"GM", "capability_ids":[]});
        assert!(serde_json::from_value::<RoleDefinitionRow>(definition.clone()).is_err());
        let mut with_description = definition;
        with_description["description"] = serde_json::json!("Authored role");
        assert!(serde_json::from_value::<RoleDefinitionRow>(with_description).is_ok());
        let aggregation = serde_json::json!({"role_id":"house/gm", "bundle_id":"weather", "capability_ids":["weather/write"]});
        assert!(serde_json::from_value::<RoleAggregationRow>(aggregation).is_err());
        assert!(Manifest::default().role_definitions.is_empty());
        assert!(Manifest::default().role_aggregations.is_empty());
    }

    #[test]
    fn binding_rows_have_authored_identity_and_no_realisation_fields() {
        let mut row = serde_json::json!({"binding_id":"house/dm", "bundle_id":"house",
            "subject":"$sender", "role_id":"dm", "extent_ref":"house/region", "description":"Region DM"});
        let binding: RoleBindingRow = serde_json::from_value(row.clone()).unwrap();
        assert_eq!(binding.expires_at_tick, None);
        assert_eq!(binding.extent_kind_ref, "");
        assert!(Manifest::default().role_bindings.is_empty());
        row["expires_at_tick"] = serde_json::json!("18446744073709551615");
        assert_eq!(
            serde_json::from_value::<RoleBindingRow>(row.clone())
                .unwrap()
                .expires_at_tick,
            Some(u64::MAX)
        );
        row["presence_entity_id"] = serde_json::json!(1);
        assert!(serde_json::from_value::<RoleBindingRow>(row).is_err());
        assert_eq!(
            crate::row_kinds::ROW_KIND_NATURAL_KEY
                .iter()
                .find(|(kind, _)| *kind == "role_bindings")
                .unwrap()
                .1,
            "binding_id"
        );
    }

    #[test]
    fn schema_version_roundtrips() {
        let parsed = SchemaVersion::parse("1.2.3").expect("schema version should parse");
        assert_eq!(parsed.major(), 1);
        assert_eq!(parsed.minor(), 2);
        assert_eq!(parsed.patch(), 3);
        assert_eq!(parsed.to_string(), "1.2.3");
    }

    #[test]
    fn string_significance_classification_is_stable() {
        assert_eq!(
            classify_string_path("rules.predicate"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("rules.outcome"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("modifier_templates.predicate_source"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("modifier_templates.modifier_expression_source"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("condition_definitions.gate_predicate_source"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("outcome_templates.outcome_expression_source"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("outcome_graph_edges.guard_predicate_source"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("condition_definitions.hooks.event_kind"),
            StringSignificance::RuntimeSignificant
        );
        assert_eq!(
            classify_string_path("metadata.title"),
            StringSignificance::Opaque
        );
    }

    #[test]
    fn economy_channel_off_turn_eligible_defaults_false_and_rejects_old_capacity() {
        // Field omission → false (every existing fixture stays turn-gated).
        let omitted: EconomyChannelDefinitionRow = serde_json::from_value(serde_json::json!({
            "id": "srd/economy_channel/bonus",
            "bundle_id": "srd",
            "pool_ms_per_round": 6000,
            "resets_on": "turn_start"
        }))
        .expect("row without off_turn_eligible should parse");
        assert!(!omitted.off_turn_eligible);

        // Explicit `true` round-trips.
        let explicit: EconomyChannelDefinitionRow = serde_json::from_value(serde_json::json!({
            "id": "srd/economy_channel/reaction",
            "bundle_id": "srd",
            "pool_ms_per_round": 6000,
            "resets_on": "turn_start",
            "off_turn_eligible": true
        }))
        .expect("row with off_turn_eligible should parse");
        assert!(explicit.off_turn_eligible);

        let old = serde_json::from_value::<EconomyChannelDefinitionRow>(serde_json::json!({
            "id": "srd/economy_channel/old",
            "bundle_id": "srd",
            "default_capacity": 1,
            "resets_on": "turn_start"
        }))
        .expect_err("old default_capacity must be rejected");
        assert!(old.to_string().contains("unknown field"));
    }

    #[test]
    fn object_state_row_visibility_omitted_parses_to_none() {
        // #1935: an omitted `visibility` field must deserialize to `None` so
        // the manifest commit path can fall back to
        // `object_state_definition.default_visibility` instead of a serde
        // sentinel silently shadowing it with `Public`.
        let omitted: ObjectStateRow = serde_json::from_value(serde_json::json!({
            "entity_key": "core/chest",
            "state_id": "core/locked",
            "source_template_id": "",
            "bundle_id": "core"
        }))
        .expect("row without visibility should parse");
        assert_eq!(omitted.visibility, None);

        let explicit: ObjectStateRow = serde_json::from_value(serde_json::json!({
            "entity_key": "core/chest",
            "state_id": "core/locked",
            "visibility": "hidden",
            "source_template_id": "",
            "bundle_id": "core"
        }))
        .expect("row with visibility should parse");
        assert_eq!(explicit.visibility, Some("hidden".to_owned()));
    }

    #[test]
    fn actor_capability_row_visibility_omitted_parses_to_none() {
        // #1935 sibling: `[[actor_capabilities]].visibility` has the same
        // definition-default shadowing shape as `[[object_state]].visibility`.
        let omitted: ActorCapabilityRow = serde_json::from_value(serde_json::json!({
            "actor_entity_key": "core/hero",
            "capability_id": "core/see",
            "source": "core",
            "bundle_id": "core"
        }))
        .expect("row without visibility should parse");
        assert_eq!(omitted.visibility, None);

        let explicit: ActorCapabilityRow = serde_json::from_value(serde_json::json!({
            "actor_entity_key": "core/hero",
            "capability_id": "core/see",
            "source": "core",
            "bundle_id": "core",
            "visibility": "hidden"
        }))
        .expect("row with visibility should parse");
        assert_eq!(explicit.visibility, Some("hidden".to_owned()));
    }

    #[test]
    fn condition_authoring_rows_reject_unknown_fields() {
        let condition = serde_json::json!({
            "condition_definition_id": "core/poisoned",
            "bundle_id": "core",
            "display_name": "Poisoned",
            "stacking_rule": "core/stacking/single",
            "gate_predicate_source": "true",
            "modifier_template_id": "core/poisoned_penalty",
            "status_definition_id": "legacy"
        });
        let err = serde_json::from_value::<ConditionDefinitionRow>(condition)
            .expect_err("legacy status field must be rejected");
        assert!(err.to_string().contains("unknown field"));

        let modifier = serde_json::json!({
            "modifier_template_id": "core/poisoned_penalty",
            "bundle_id": "core",
            "applies_to": "recovery",
            "display_name": "Poisoned penalty",
            "predicate_source": "true",
            "modifier_expression_source": "true",
            "unexpected": "legacy"
        });
        let err = serde_json::from_value::<ModifierTemplateRow>(modifier)
            .expect_err("unknown modifier field must be rejected");
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn manifest_json_schema_covers_final_substrate_sections() {
        let schema = serde_json::to_value(manifest_json_schema()).expect("schema should serialize");
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("manifest schema should expose top-level properties");

        for section in [
            "entities",
            "entity_containment",
            "object_state",
            "actors",
            "actor_identities",
            "entity_definitions",
            "equipment_slot_definitions",
            "actor_equipment_slots",
            "anchor_kind_definitions",
            "coordinate_kind_definitions",
            "substrate_definitions",
            "place_graphs",
            "place_graph_instances",
            "place_nodes",
            "place_edges",
            "map_layers",
            "place_edge_affordance",
            "metadata_namespaces",
            "metadata_attachments",
            "spatial_niche_attachments",
            "area_shape_definitions",
        ] {
            assert!(
                properties.contains_key(section),
                "manifest schema missing final substrate section {section}"
            );
        }
    }

    #[test]
    fn outcome_template_row_accepts_typed_interaction_descriptors() {
        let row: OutcomeTemplateRow = serde_json::from_value(serde_json::json!({
            "outcome_template_id": "core/equip/outcome",
            "bundle_id": "core",
            "display_name": "Equip",
            "outcome_bool": true,
            "manipulation_action_id": "core/equip",
            "communication_speech_act_id": "core/assert",
            "movement_descriptor": "move",
            "sense_query_kind": "core/look"
        }))
        .expect("typed descriptors should parse");

        assert_eq!(row.manipulation_action_id, "core/equip");
        assert_eq!(row.communication_speech_act_id, "core/assert");
        assert_eq!(row.movement_descriptor, "move");
        assert_eq!(row.sense_query_kind, "core/look");
        assert_eq!(row.sense_visibility, VisibilityClass::Local);

        for (authored, expected) in [
            ("public", VisibilityClass::Public),
            ("hidden", VisibilityClass::Hidden),
        ] {
            let mut value = serde_json::json!({
                "outcome_template_id": "core/look/outcome",
                "bundle_id": "core",
                "display_name": "Look",
                "sense_query_kind": "core/look",
                "sense_visibility": authored
            });
            let explicit: OutcomeTemplateRow = serde_json::from_value(value.take()).unwrap();
            assert_eq!(explicit.sense_visibility, expected);
        }

        let invalid = serde_json::json!({
            "outcome_template_id": "core/look/outcome",
            "bundle_id": "core",
            "display_name": "Look",
            "sense_query_kind": "core/look",
            "sense_visibility": "global"
        });
        assert!(serde_json::from_value::<OutcomeTemplateRow>(invalid).is_err());
    }

    #[test]
    fn action_definition_row_accepts_resolution_mode_fields() {
        let row: ActionDefinitionRow = serde_json::from_value(serde_json::json!({
            "action_id": "core/persuade",
            "bundle_id": "core",
            "required_capability": "core/communicate",
            "on_success": "core/persuade/success",
            "on_failure": "core/persuade/failure",
            "resolution_mode": "Stochastic",
            "resolution_predicate_source": "d20 >= 15",
            "resolution_scope": "homebrew/ability_check",
            "non_steering": true,
            "display_name": "Persuade",
            "cost_time_ms": 500
        }))
        .expect("resolution mode fields should parse");

        assert_eq!(row.resolution_mode, ActionResolutionModeRow::Stochastic);
        assert_eq!(row.resolution_predicate_source, "d20 >= 15");
        assert_eq!(row.resolution_scope, "homebrew/ability_check");
        assert!(row.non_steering);
    }

    #[test]
    fn action_definition_row_defaults_to_deterministic_resolution() {
        let row: ActionDefinitionRow = serde_json::from_value(serde_json::json!({
            "action_id": "core/inspect",
            "bundle_id": "core",
            "required_capability": "core/inspect",
            "on_success": "core/inspect/success",
            "display_name": "Inspect"
        }))
        .expect("resolution mode defaults should parse");

        assert_eq!(row.resolution_mode, ActionResolutionModeRow::Deterministic);
        assert_eq!(row.resolution_predicate_source, "");
        assert_eq!(row.resolution_scope, "");
        assert!(!row.non_steering);
    }

    #[test]
    fn action_definition_row_rejects_legacy_resolution_scalars() {
        let invalid = serde_json::json!({
            "action_id": "core/persuade",
            "bundle_id": "core",
            "required_capability": "core/communicate",
            "on_success": "core/persuade/success",
            "resolution_mode": "Stochastic",
            "resolution_check": "Persuasion",
            "resolution_dc": 15,
            "display_name": "Persuade"
        });
        assert!(
            serde_json::from_value::<ActionDefinitionRow>(invalid).is_err(),
            "legacy resolution_check/resolution_dc must be rejected (deny_unknown_fields)"
        );
    }

    #[test]
    fn spatial_niche_attachment_row_accepts_paired_node_entry_action_fields() {
        let row: SpatialNicheAttachmentRow = serde_json::from_value(serde_json::json!({
            "bundle_id": "core",
            "place_node_id": "core/hall/plate",
            "niche_id": "core/pressure_plate",
            "on_entry_action_id": "core/pressure_plate/save",
            "on_entry_target_entity_key": "core/pressure_plate_1"
        }))
        .expect("paired node-entry action fields should parse");

        assert_eq!(row.on_entry_action_id, "core/pressure_plate/save");
        assert_eq!(row.on_entry_target_entity_key, "core/pressure_plate_1");
    }

    #[test]
    fn spatial_niche_attachment_row_defaults_to_no_entry_action() {
        let row: SpatialNicheAttachmentRow = serde_json::from_value(serde_json::json!({
            "bundle_id": "core",
            "place_node_id": "core/hall",
            "niche_id": "core/inspectable"
        }))
        .expect("existing locality-only attachment should keep parsing");

        assert!(row.on_entry_action_id.is_empty());
        assert!(row.on_entry_target_entity_key.is_empty());
    }

    #[test]
    fn dissolved_manifest_sections_stay_absent_from_json_schema() {
        let schema = serde_json::to_value(manifest_json_schema()).expect("schema should serialize");
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("manifest schema should expose top-level properties");

        for dissolved in [
            "fixture_locations",
            "machine_states",
            "machine_ports",
            "machine_port_inventory",
            "scheduled_phenomena",
            "place_node_spawn_rules",
            "location_spawn_rules",
            "spawn_rule_definitions",
            "traversal_predicate_definitions",
            "traversal_predicate_param_schema",
            "place_edge_predicates",
            // Legacy item and world-object rows superseded by unified entities.
            "items",
            "item_inventory",
            "item_locations",
            "item_states",
            "world_objects",
            "world_object_locations",
        ] {
            assert!(
                !properties.contains_key(dissolved),
                "dissolved manifest section {dissolved} must not return"
            );
        }
    }

    #[test]
    fn legacy_entity_overlay_sections_are_explicit_until_fixture_rewrite() {
        let schema = serde_json::to_value(manifest_json_schema()).expect("schema should serialize");
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("manifest schema should expose top-level properties");

        let legacy = "item_definitions";
        assert!(
            properties.contains_key(legacy),
            "legacy compatibility section {legacy} should remain until Phase-8 fixture rewrite removes its last author"
        );
    }

    #[test]
    fn tag_prefix_declaration_parses_with_neither_optional_field() {
        let parsed: TagPrefixDeclaration =
            serde_json::from_str(r#"{"prefix":"srd5e"}"#).expect("bare prefix should parse");
        assert_eq!(parsed.prefix, "srd5e");
        assert_eq!(parsed.target_kind, None);
        assert_eq!(parsed.natural_key_field, None);
    }

    #[test]
    fn tag_prefix_declaration_parses_with_both_ref_semantics_fields() {
        let parsed: TagPrefixDeclaration = serde_json::from_str(
            r#"{"prefix":"srd.ability","target_kind":"resource_definitions","natural_key_field":"resource_definition_id"}"#,
        )
        .expect("ref-typed multi-segment prefix should parse");
        assert_eq!(parsed.prefix, "srd.ability");
        assert_eq!(parsed.target_kind.as_deref(), Some("resource_definitions"));
        assert_eq!(
            parsed.natural_key_field.as_deref(),
            Some("resource_definition_id")
        );
    }

    #[test]
    fn tag_prefix_declaration_rejects_target_kind_without_natural_key_field() {
        let err = serde_json::from_str::<TagPrefixDeclaration>(
            r#"{"prefix":"srd.ability","target_kind":"resource_definitions"}"#,
        )
        .expect_err("target_kind without natural_key_field is a shape error");
        let message = err.to_string();
        assert!(
            message.contains("srd.ability"),
            "error should name the entry's prefix: {message}"
        );
        assert!(
            message.contains("natural_key_field"),
            "error should name the missing field: {message}"
        );
    }

    #[test]
    fn tag_prefix_declaration_rejects_natural_key_field_without_target_kind() {
        let err = serde_json::from_str::<TagPrefixDeclaration>(
            r#"{"prefix":"srd.ability","natural_key_field":"resource_definition_id"}"#,
        )
        .expect_err("natural_key_field without target_kind is a shape error");
        let message = err.to_string();
        assert!(
            message.contains("srd.ability"),
            "error should name the entry's prefix: {message}"
        );
        assert!(
            message.contains("target_kind"),
            "error should name the missing field: {message}"
        );
    }

    #[test]
    fn bundle_row_round_trips_tag_prefixes_and_allow_flat_tags() {
        let row: BundleRow = serde_json::from_str(
            r#"{
                "bundle_id": "srd",
                "display_name": "SRD",
                "author": "spellkave",
                "version": "1.0.0",
                "tag_prefixes": [
                    { "prefix": "srd5e" },
                    { "prefix": "srd.ability", "target_kind": "resource_definitions", "natural_key_field": "resource_definition_id" }
                ],
                "allow_flat_tags": true
            }"#,
        )
        .expect("bundle row with tag_prefixes/allow_flat_tags should parse");
        assert_eq!(row.tag_prefixes.len(), 2);
        assert_eq!(row.tag_prefixes[0].prefix, "srd5e");
        assert_eq!(row.tag_prefixes[1].prefix, "srd.ability");
        assert!(row.allow_flat_tags);
    }

    #[test]
    fn bundle_row_defaults_tag_prefixes_and_allow_flat_tags_when_absent() {
        let row: BundleRow = serde_json::from_str(
            r#"{
                "bundle_id": "core",
                "display_name": "Core",
                "author": "spellkave",
                "version": "1.0.0"
            }"#,
        )
        .expect("bundle row omitting both fields should parse exactly as before");
        assert!(row.tag_prefixes.is_empty());
        assert!(!row.allow_flat_tags);
    }
    #[test]
    fn dsl_macro_definition_uses_the_shared_dsl_value_type() {
        let row: DslMacroDefinitionRow = serde_json::from_value(serde_json::json!({
            "macro_id": "srd/ability_modifier",
            "bundle_id": "srd",
            "return_type": "Int",
            "parameters": [{ "name": "score", "value_type": "Int" }],
            "body_source": "score - 10"
        }))
        .expect("typed macro row should deserialize");

        assert_eq!(row.return_type, DslValueType::Int);
        assert_eq!(row.parameters[0].value_type, DslValueType::Int);
    }

    #[test]
    fn manifest_json_schema_exposes_dsl_macro_definitions() {
        let schema = serde_json::to_value(manifest_json_schema()).expect("schema should serialize");
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("manifest schema should expose top-level properties");
        assert!(properties.contains_key("dsl_macro_definitions"));
    }

    #[test]
    fn tag_value_definition_row_round_trips_with_exactly_its_four_fields() {
        let row: TagValueDefinitionRow = serde_json::from_str(
            r#"{
                "tag": "demo.creature_type.humanoid",
                "prefix": "demo.creature_type",
                "bundle_id": "demo",
                "description": "Humanoid creature type."
            }"#,
        )
        .expect("exact tag-value definition should parse");
        assert_eq!(row.tag, "demo.creature_type.humanoid");
        assert_eq!(row.prefix, "demo.creature_type");
        assert_eq!(row.bundle_id, "demo");
        assert_eq!(row.description, "Humanoid creature type.");

        let rendered = serde_json::to_value(&row).expect("row should serialize");
        assert_eq!(
            rendered
                .as_object()
                .expect("row should serialize to object")
                .len(),
            4,
            "tag-value definition admits exactly four fields"
        );
    }

    #[test]
    fn manifest_round_trips_tag_value_definitions() {
        let manifest: Manifest = serde_json::from_str(
            r#"
                {
                    "schema_version": "1.0.0",
                    "tag_value_definitions": [{
                        "tag": "demo.creature_type.humanoid",
                        "prefix": "demo.creature_type",
                        "bundle_id": "demo",
                        "description": "Humanoid creature type."
                    }]
                }
            "#,
        )
        .expect("manifest with exact tag-value definition should parse");
        assert_eq!(manifest.tag_value_definitions.len(), 1);

        let rendered = serde_json::to_string(&manifest).expect("manifest should render");
        let reparsed: Manifest =
            serde_json::from_str(&rendered).expect("rendered manifest should parse");
        assert_eq!(reparsed, manifest);
    }

    #[test]
    fn tag_value_definition_is_registered_by_complete_tag() {
        assert!(ROW_KIND_NATURAL_KEY.contains(&("tag_value_definitions", "tag")));
    }

    #[test]
    fn tag_value_definition_requires_its_description_field() {
        let err = serde_json::from_str::<TagValueDefinitionRow>(
            r#"{
                "tag": "demo.creature_type.humanoid",
                "prefix": "demo.creature_type",
                "bundle_id": "demo"
            }"#,
        )
        .expect_err("tag-value definition description is required");
        assert!(err.to_string().contains("description"));
    }
}

/// Caller-reviewed preimage and stable retry correlation for one binding transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindingPrecondition {
    pub binding_id: String,
    pub expected_digest: String,
    pub expected_merged_role_digest: String,
    pub expected_incarnation: String,
    #[serde(deserialize_with = "u64_flexible")]
    pub expected_revision: u64,
    pub request_id: String,
    pub reason: String,
}

#[cfg(test)]
mod hand_over_wire_tests {
    use super::*;
    #[test]
    fn legacy_binding_and_typed_precondition_are_strict_and_lossless() {
        let old = r#"schema_version="1.0.0"
[[role_bindings]]
binding_id="handover/test"
bundle_id=""
description="Legacy binding"
subject="$sender"
role_id="dm"
extent_ref="forest"
"#;
        let legacy: Manifest = toml::from_str(old).unwrap();
        assert!(legacy.binding_preconditions.is_empty());
        assert!(legacy.role_bindings[0].handover_note.is_none());
        let mut updated = legacy.clone();
        updated.role_bindings[0].handover_note = Some("Shared, inert briefing".into());
        updated.binding_preconditions.push(BindingPrecondition {
            binding_id: "handover/test".into(),
            expected_digest: "a".repeat(64),
            expected_merged_role_digest: "b".repeat(64),
            expected_incarnation: "world/random".into(),
            expected_revision: 4,
            request_id: "12345678-1234-1234-1234-123456789abc".into(),
            reason: "Backup resumes".into(),
        });
        let wire = toml::to_string(&updated).unwrap();
        assert_eq!(toml::from_str::<Manifest>(&wire).unwrap(), updated);
        let max_wire = wire.replace(
            "expected_revision = 4",
            "expected_revision = \"18446744073709551615\"",
        );
        assert_eq!(
            toml::from_str::<Manifest>(&max_wire)
                .unwrap()
                .binding_preconditions[0]
                .expected_revision,
            u64::MAX
        );
        assert!(
            toml::from_str::<Manifest>(&wire.replace("expected_revision", "unknown_revision"))
                .is_err()
        );
        assert!(
            toml::from_str::<Manifest>(&wire.replace("handover_note", "private_transcript"))
                .is_err()
        );
    }
}

pub mod refusal;
