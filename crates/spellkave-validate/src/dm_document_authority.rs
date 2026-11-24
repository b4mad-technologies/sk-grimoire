//! Compile-time authority derivation for the opt-in DM document carrier.
//! This helper alone admits no document, publication, principal or runtime.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const PRESENTATION_NAMESPACE: &str = "npc/dm/presentation/v1";
pub const DUTIES_NAMESPACE: &str = "npc/dm/duties/v1";
pub const MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub binding_id: String,
    pub role_id: String,
    pub extent_kind_ref: String,
    pub extent_ref: String,
    pub description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SightGrant {
    pub capability_id: String,
    pub extent_kind_ref: String,
    pub extent_ref: String,
    pub elevation_class: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PrincipalPlan {
    #[serde(rename = "schema_version")]
    pub schema_version: u32,
    pub bundle_id: String,
    pub body_key: String,
    pub bindings: Vec<Binding>,
    pub retired_bindings: Vec<Binding>,
    pub sight_grants: Vec<SightGrant>,
    pub fiction_capabilities: Vec<String>,
    pub seat_capability: String,
    pub role_capabilities: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reads {
    pub reader_role_id: String,
    pub reader_binding_id: String,
    pub catalog_read_capability_id: String,
    pub duty_read_capability_id: String,
    pub presentation_read_capability_id: String,
}
/// Intermediate compiler input, assembled from the admitted source and catalog.
/// Not an alternative top-level publication artifact.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityInput {
    pub schema_version: u32,
    pub bundle_id: String,
    pub body_key: String,
    pub principal_plan: PrincipalPlan,
    pub reads: Reads,
    pub catalog_namespace: String,
}

/// Constructor is private: no caller can bypass this facet's checks. Complete
/// carrier admission must additionally validate typed duties, presentation,
/// catalog, references, envelopes and the exact rederived publication artifact.
#[derive(Clone, Debug)]
pub struct AdmittedAuthority(AuthorityInput);
impl AdmittedAuthority {
    pub fn principal_plan(&self) -> PrincipalPlan {
        let mut plan = self.0.principal_plan.clone();
        let reads = &self.0.reads;
        plan.bindings.push(Binding {
            binding_id: reads.reader_binding_id.clone(),
            role_id: reads.reader_role_id.clone(),
            extent_kind_ref: "bundle".into(),
            extent_ref: self.0.bundle_id.clone(),
            description: "Read trusted DM duties and the shared quest catalog.".into(),
        });
        plan.role_capabilities.insert(
            reads.reader_role_id.clone(),
            vec![
                reads.catalog_read_capability_id.clone(),
                reads.duty_read_capability_id.clone(),
            ],
        );
        plan
    }

    /// Definitions and private-body grant only. Binding subject materialization
    /// stays in the publisher's single Pdocs contribution. No metadata payloads
    /// or host readiness can be obtained from this intermediate row set.
    pub fn definition_rows(&self) -> Value {
        let bundle = &self.0.bundle_id;
        let reads = &self.0.reads;
        let capability = |id: &str| {
            json!({
                "bundle_id":bundle, "capability_id":id, "actor_kind":"entity",
                "display_name":"Read DM document", "description":"Read the declared metadata audience",
                "default_visibility":"Public"
            })
        };
        // Service identities only: no person reads DM documents through
        // these capabilities (spellkave_schema::SERVICE_ONLY_AUDIENCE_TAG).
        let service_only = |id: &str| {
            json!({
                "bundle_id":bundle, "capability_id":id,
                "tag":spellkave_schema::SERVICE_ONLY_AUDIENCE_TAG, "source":"manifest"
            })
        };
        json!({
            "schema_version":"1.0.0",
            "capability_definitions":[capability(&reads.duty_read_capability_id),capability(&reads.presentation_read_capability_id)],
            "capability_definition_tag":[service_only(&reads.duty_read_capability_id),service_only(&reads.presentation_read_capability_id)],
            "role_definitions":[{
                "bundle_id":bundle,"role_id":reads.reader_role_id,
                "display_name":"DM document reader","description":"Read catalog and trusted duties only",
                "capability_ids":[reads.catalog_read_capability_id,reads.duty_read_capability_id]
            }],
            "metadata_namespaces":[
                {"bundle_id":bundle,"namespace":PRESENTATION_NAMESPACE,"audience":reads.presentation_read_capability_id,"on_demand":false},
                {"bundle_id":bundle,"namespace":DUTIES_NAMESPACE,"audience":reads.duty_read_capability_id,"on_demand":false}
            ],
            "actor_capabilities":[{
                "bundle_id":bundle,"actor_entity_key":self.0.body_key,
                "capability_id":reads.presentation_read_capability_id,"source":bundle,
                "source_args_hash":"","elevation_class":"","extent_kind_ref":"","extent_ref":""
            }]
        })
    }
}

fn nonblank(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 16 * 1024
}
fn distinct<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .all(|value| nonblank(value) && seen.insert(value))
}
fn scoped(bundle: &str, id: &str) -> bool {
    nonblank(id)
        && id
            .strip_prefix(bundle)
            .is_some_and(|tail| tail.starts_with('/') && tail.len() > 1)
}
fn invalid(message: &str) -> Vec<String> {
    vec![format!("dm-authority: {message}")]
}

/// Raw JSON parser with duplicate refusal at every depth, before typed parsing.
/// Also used for the eventual source/artifact envelopes: parsing into Value
/// first with serde_json's ordinary map handling would silently discard keys.
pub fn unique_json(input: &str) -> Result<Value, Vec<String>> {
    bounded_unique_json(
        input,
        MAX_ARTIFACT_BYTES,
        "raw JSON exceeds Job artifact byte bound",
    )
}

/// Full canonical roots retain the existing publisher total world budget,
/// independently of the smaller Job/projection and runtime-document bounds.
pub(crate) fn unique_canonical_worlds_json(input: &str) -> Result<Value, Vec<String>> {
    bounded_unique_json(
        input,
        64 * 1024 * 1024,
        "canonical JSON exceeds publisher total world byte bound",
    )
}

fn bounded_unique_json(
    input: &str,
    maximum: usize,
    bound_error: &str,
) -> Result<Value, Vec<String>> {
    use serde::de::{MapAccess, SeqAccess, Visitor};
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct JsonVisitor;
            impl<'de> Visitor<'de> for JsonVisitor {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("JSON with unique object keys")
                }
                fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Unique, E> {
                    Ok(Unique(Value::Bool(value)))
                }
                fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Unique, E> {
                    Ok(Unique(value.into()))
                }
                fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Unique, E> {
                    Ok(Unique(value.into()))
                }
                fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Unique, E> {
                    serde_json::Number::from_f64(value)
                        .map(|v| Unique(Value::Number(v)))
                        .ok_or_else(|| E::custom("invalid number"))
                }
                fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Unique, E> {
                    Ok(Unique(Value::String(value.into())))
                }
                fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Unique, E> {
                    Ok(Unique(Value::String(value)))
                }
                fn visit_unit<E: serde::de::Error>(self) -> Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
                    let mut values = vec![];
                    while let Some(Unique(value)) = seq.next_element()? {
                        values.push(value);
                    }
                    Ok(Unique(Value::Array(values)))
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
                    let mut values = serde_json::Map::new();
                    while let Some((key, Unique(value))) = map.next_entry::<String, Unique>()? {
                        if values.insert(key, value).is_some() {
                            return Err(serde::de::Error::custom("duplicate object key"));
                        }
                    }
                    Ok(Unique(Value::Object(values)))
                }
            }
            d.deserialize_any(JsonVisitor)
        }
    }
    if input.len() > maximum {
        return Err(invalid(bound_error));
    }
    serde_json::from_str::<Unique>(input)
        .map(|value| value.0)
        .map_err(|_| invalid("invalid or duplicate JSON"))
}

fn input_valid(input: &AuthorityInput) -> bool {
    let plan = &input.principal_plan;
    let reads = &input.reads;
    let caps = [
        reads.catalog_read_capability_id.as_str(),
        reads.duty_read_capability_id.as_str(),
        reads.presentation_read_capability_id.as_str(),
    ];
    let original_caps: BTreeSet<_> = plan
        .role_capabilities
        .values()
        .flatten()
        .chain(&plan.fiction_capabilities)
        .chain(plan.sight_grants.iter().map(|v| &v.capability_id))
        .map(String::as_str)
        .chain([plan.seat_capability.as_str()])
        .collect();
    let bound_roles: BTreeSet<_> = plan.bindings.iter().map(|v| v.role_id.as_str()).collect();
    let new_ids = [
        reads.reader_role_id.as_str(),
        reads.reader_binding_id.as_str(),
        reads.catalog_read_capability_id.as_str(),
        reads.duty_read_capability_id.as_str(),
        reads.presentation_read_capability_id.as_str(),
    ];
    input.schema_version == 1
        && plan.schema_version == 1
        && nonblank(&input.bundle_id)
        && input.bundle_id == plan.bundle_id
        && scoped(&input.bundle_id, &input.body_key)
        && input.body_key == plan.body_key
        && nonblank(&input.catalog_namespace)
        && ![PRESENTATION_NAMESPACE, DUTIES_NAMESPACE].contains(&input.catalog_namespace.as_str())
        && !plan.bindings.is_empty()
        && distinct(
            plan.bindings
                .iter()
                .chain(&plan.retired_bindings)
                .map(|v| v.binding_id.as_str()),
        )
        && plan.bindings.iter().chain(&plan.retired_bindings).all(|v| {
            nonblank(&v.role_id) && nonblank(&v.extent_kind_ref) && nonblank(&v.extent_ref)
        })
        && plan
            .role_capabilities
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            == bound_roles
        && plan.role_capabilities.values().all(|v| {
            !v.is_empty()
                && distinct(v.iter().map(String::as_str))
                && !v.iter().any(|cap| cap == "frontier.drive")
        })
        && distinct(plan.fiction_capabilities.iter().map(String::as_str))
        && distinct(plan.sight_grants.iter().map(|v| v.capability_id.as_str()))
        && plan
            .sight_grants
            .iter()
            .all(|v| nonblank(&v.extent_kind_ref) && nonblank(&v.extent_ref))
        && nonblank(&plan.seat_capability)
        && plan.fiction_capabilities.contains(&plan.seat_capability)
        && distinct(caps.into_iter())
        && distinct(new_ids.into_iter())
        && new_ids.iter().all(|id| {
            !bound_roles.contains(id)
                && !plan
                    .bindings
                    .iter()
                    .chain(&plan.retired_bindings)
                    .any(|v| v.binding_id == *id)
                && !original_caps.contains(id)
        })
        && caps
            .iter()
            .all(|cap| scoped(&input.bundle_id, cap) && !original_caps.contains(cap))
        && scoped(&input.bundle_id, &reads.reader_binding_id)
        && scoped(&input.bundle_id, &reads.reader_role_id)
        && !plan
            .bindings
            .iter()
            .chain(&plan.retired_bindings)
            .any(|v| v.binding_id == reads.reader_binding_id || v.role_id == reads.reader_role_id)
}

/// Check one static authority facet against prepared canonical worlds. This is
/// not the full carrier compiler, a live materialization verifier or an upgrade
/// path for a world that has already applied P.
pub fn admit_read_authority(
    input_json: &str,
    worlds_json: &str,
) -> Result<AdmittedAuthority, Vec<String>> {
    let input: AuthorityInput = serde_json::from_value(unique_json(input_json)?)
        .map_err(|_| invalid("invalid authority shape"))?;
    if !input_valid(&input) {
        return Err(invalid("inconsistent plan or read authority"));
    }
    let worlds = unique_json(worlds_json)?;
    let worlds = worlds
        .as_array()
        .ok_or_else(|| invalid("invalid canonical worlds"))?;
    if worlds.is_empty() || worlds.iter().any(|v| !v.is_object()) {
        return Err(invalid("missing canonical worlds"));
    }
    let rows = |table: &str| -> Result<Vec<Value>, Vec<String>> {
        let mut rows = vec![];
        for world in worlds {
            if let Some(value) = world.get(table) {
                let Some(values) = value.as_array() else {
                    return Err(invalid("invalid canonical table"));
                };
                for value in values {
                    if !value.is_object() {
                        return Err(invalid("invalid canonical row"));
                    }
                    // Existing flattened/dependency composition can repeat an
                    // identical row. Conflicting identities are checked below.
                    if !rows.contains(value) {
                        rows.push(value.clone());
                    }
                }
            }
        }
        Ok(rows)
    };
    let reads = &input.reads;
    let definitions: Vec<spellkave_schema::RoleDefinitionRow> = rows("role_definitions")?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|_| invalid("invalid role definitions"))?;
    let aggregations: Vec<spellkave_schema::RoleAggregationRow> = rows("role_aggregations")?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|_| invalid("invalid role aggregations"))?;
    for (role, expected) in &input.principal_plan.role_capabilities {
        if definitions.iter().filter(|v| v.role_id == *role).count() != 1
            || crate::merged_role_capability_ids(role, &definitions, &aggregations)
                .into_iter()
                .collect::<BTreeSet<_>>()
                != expected.iter().cloned().collect()
        {
            return Err(invalid("original merged role capabilities mismatch"));
        }
    }
    if definitions
        .iter()
        .any(|v| v.role_id == reads.reader_role_id)
        || aggregations
            .iter()
            .any(|v| v.role_id == reads.reader_role_id)
    {
        return Err(invalid("reader role already declared or aggregated"));
    }
    if definitions
        .iter()
        .flat_map(|v| &v.capability_ids)
        .chain(aggregations.iter().flat_map(|v| &v.capability_ids))
        .any(|cap| {
            cap == &reads.duty_read_capability_id || cap == &reads.presentation_read_capability_id
        })
    {
        return Err(invalid("new read capability already referenced by a role"));
    }
    let capabilities = rows("capability_definitions")?;
    let catalog: Vec<_> = capabilities
        .iter()
        .filter(|v| v["capability_id"] == reads.catalog_read_capability_id)
        .collect();
    if catalog.len() != 1
        || catalog[0]["bundle_id"] != input.bundle_id
        || catalog[0]["actor_kind"] != "entity"
    {
        return Err(invalid("catalog read capability mismatch"));
    }
    let namespaces = rows("metadata_namespaces")?;
    let catalog_ns: Vec<_> = namespaces
        .iter()
        .filter(|v| v["namespace"] == input.catalog_namespace)
        .collect();
    if catalog_ns.len() != 1
        || catalog_ns[0]["bundle_id"] != input.bundle_id
        || catalog_ns[0]["audience"] != reads.catalog_read_capability_id
        || catalog_ns[0].get("on_demand").is_some_and(|v| v != false)
    {
        return Err(invalid("catalog audience mismatch"));
    }
    if capabilities.iter().any(|v| {
        v["capability_id"] == reads.duty_read_capability_id
            || v["capability_id"] == reads.presentation_read_capability_id
    }) || namespaces.iter().any(|v| {
        [Some(PRESENTATION_NAMESPACE), Some(DUTIES_NAMESPACE)].contains(&v["namespace"].as_str())
            || v["audience"] == reads.duty_read_capability_id
            || v["audience"] == reads.presentation_read_capability_id
    }) {
        return Err(invalid("document definitions already exist"));
    }
    let read_caps = [
        &reads.catalog_read_capability_id,
        &reads.duty_read_capability_id,
        &reads.presentation_read_capability_id,
    ];
    if rows("action_definitions")?.iter().any(|v| {
        read_caps
            .iter()
            .any(|cap| v["required_capability"] == **cap)
    }) {
        return Err(invalid("read capability is used by an action"));
    }
    // Never rewrite an existing identity binding or accept body metadata grants
    // as principal authority, including a previously installed P/Pdocs world.
    if rows("role_bindings")?.iter().any(|v| {
        v["binding_id"] == reads.reader_binding_id
            || v["role_id"] == reads.reader_role_id
            || input
                .principal_plan
                .bindings
                .iter()
                .chain(&input.principal_plan.retired_bindings)
                .any(|b| v["binding_id"] == b.binding_id)
    }) {
        return Err(invalid("DM binding already exists in prepared input"));
    }
    if rows("actor_capabilities")?.iter().any(|v| {
        v["capability_id"] == reads.duty_read_capability_id
            || v["capability_id"] == reads.presentation_read_capability_id
            || (v["capability_id"] == reads.catalog_read_capability_id
                && v["actor_entity_key"] == input.body_key)
    }) {
        return Err(invalid("preexisting DM document actor grant"));
    }
    if !rows("bundles")?
        .iter()
        .any(|v| v["bundle_id"] == input.bundle_id)
        || !rows("entities")?
            .iter()
            .any(|v| v["entity_key"] == input.body_key && v["bundle_id"] == input.bundle_id)
    {
        return Err(invalid("missing bundle or body reference"));
    }
    Ok(AdmittedAuthority(input))
}
