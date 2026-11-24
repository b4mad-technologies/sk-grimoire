//! Shared admission for generated runtime SRD tables inside a trusted DM duty
//! document. Admitting these values grants no runtime or publication authority.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tables {
    pub classes: Vec<Class>,
    pub species: Vec<Species>,
    pub backgrounds: Vec<Background>,
    pub skills: Vec<Skill>,
    #[serde(deserialize_with = "unique_map")]
    pub xp_thresholds: BTreeMap<String, u64>,
    #[serde(deserialize_with = "unique_map")]
    pub point_costs: BTreeMap<String, u64>,
    pub caster_profiles: Vec<CasterProfile>,
    #[serde(deserialize_with = "unique_map")]
    pub caster_payloads: BTreeMap<String, CasterPayload>,
    pub spell_slot_grants: Vec<SlotGrant>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Class {
    pub slug: String,
    pub display_name: String,
    pub primary_ability_text: String,
    pub skill_proficiencies_text: String,
    pub starting_equipment_text: String,
    pub hit_die_size: u64,
    pub level1_hp_base: u64,
    pub fixed_hp_per_level_base: u64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub unarmored_defense_extra_ability: Option<String>,
    #[serde(deserialize_with = "unique_map")]
    pub standard_array: BTreeMap<String, u64>,
    pub source_pk: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Species {
    pub slug: String,
    pub display_name: String,
    pub speed_cells: u64,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Background {
    pub slug: String,
    pub display_name: String,
    pub skill_capability_ids: Vec<String>,
    pub feat_entity_id: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Skill {
    pub slug: String,
    pub display_name: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CasterProfile {
    pub class_slug: String,
    pub class_display_name: String,
    pub ability: String,
    pub ability_anchor: String,
    pub feature_slug: String,
    pub feature_anchor: String,
    pub ritual_casting: bool,
    pub class_source_pk: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CasterPayload {
    pub class: String,
    pub ability: String,
    pub feature: String,
    pub ritual_casting: bool,
    #[serde(deserialize_with = "unique_map")]
    pub prepared_spells_by_level: BTreeMap<String, u64>,
    #[serde(default, deserialize_with = "present_map")]
    pub cantrips_known_by_level: Option<BTreeMap<String, u64>>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SlotGrant {
    pub class_slug: String,
    pub level: u64,
    pub slot_level: u64,
    pub count: u64,
}
fn unique_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map with unique keys")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, T>()? {
                if result.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate DM table key"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}
fn present_map<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, u64>>, D::Error> {
    unique_map(deserializer).map(Some)
}
fn unique<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .all(|v| !v.trim().is_empty() && seen.insert(v))
}
fn numeric_map(values: &BTreeMap<String, u64>) -> bool {
    !values.is_empty()
        && values.keys().all(|key| {
            key.parse::<u64>()
                .is_ok_and(|n| n > 0 && n <= 9_007_199_254_740_991 && n.to_string() == *key)
        })
}
impl Tables {
    /// Shape and reference relationships against domains declared by these tables.
    fn validate(&self) -> Result<(), &'static str> {
        if self.classes.is_empty()
            || self.species.is_empty()
            || self.backgrounds.is_empty()
            || self.skills.is_empty()
            || self.caster_profiles.is_empty()
            || self.spell_slot_grants.is_empty()
            || !unique(self.classes.iter().map(|v| v.slug.as_str()))
            || !unique(self.species.iter().map(|v| v.slug.as_str()))
            || !unique(self.backgrounds.iter().map(|v| v.slug.as_str()))
            || !unique(self.skills.iter().map(|v| v.slug.as_str()))
            || !unique(self.caster_profiles.iter().map(|v| v.class_slug.as_str()))
        {
            return Err("dm-srd: missing or duplicate catalog row");
        }
        let abilities: BTreeSet<_> = self.classes[0]
            .standard_array
            .keys()
            .map(String::as_str)
            .collect();
        if abilities.is_empty() {
            return Err("dm-srd: empty ability domain");
        }
        if self.classes.iter().any(|row| {
            row.hit_die_size == 0
                || row.level1_hp_base == 0
                || row.fixed_hp_per_level_base == 0
                || row
                    .standard_array
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>()
                    != abilities
                || row
                    .unarmored_defense_extra_ability
                    .as_deref()
                    .is_some_and(|v| !abilities.contains(v))
        }) || self.species.iter().any(|v| v.speed_cells == 0)
            || self.backgrounds.iter().any(|v| {
                v.skill_capability_ids.is_empty()
                    || !unique(v.skill_capability_ids.iter().map(String::as_str))
            })
            || !numeric_map(&self.xp_thresholds)
            || !numeric_map(&self.point_costs)
        {
            return Err("dm-srd: invalid class, origin or numeric table");
        }
        let classes: BTreeMap<_, _> = self.classes.iter().map(|v| (v.slug.as_str(), v)).collect();
        let mut caster_keys = BTreeSet::new();
        for row in &self.caster_profiles {
            let Some(class) = classes.get(row.class_slug.as_str()) else {
                return Err("dm-srd: unknown caster class");
            };
            let Some(payload) = self.caster_payloads.get(&row.class_slug) else {
                return Err("dm-srd: missing caster payload");
            };
            if !abilities.contains(row.ability.as_str())
                || row.class_display_name != class.display_name
                || row.class_source_pk != class.source_pk
                || payload.class != row.class_slug
                || payload.ability != row.ability
                || payload.ritual_casting != row.ritual_casting
                || !numeric_map(&payload.prepared_spells_by_level)
                || payload
                    .cantrips_known_by_level
                    .as_ref()
                    .is_some_and(|v| !numeric_map(v))
            {
                return Err("dm-srd: inconsistent caster payload");
            }
            caster_keys.insert(row.class_slug.as_str());
        }
        if self
            .caster_payloads
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != caster_keys
        {
            return Err("dm-srd: foreign caster payload");
        }
        let mut slots = BTreeSet::new();
        for row in &self.spell_slot_grants {
            if !caster_keys.contains(row.class_slug.as_str())
                || row.level == 0
                || row.slot_level == 0
                || row.count == 0
                || !slots.insert((&row.class_slug, row.level, row.slot_level))
            {
                return Err("dm-srd: invalid or duplicate slot grant");
            }
        }
        Ok(())
    }
}
fn scalar_domain(value: &serde_json::Value) -> bool {
    use serde_json::Value;
    match value {
        Value::Number(v) => v.as_u64().is_some_and(|n| n <= 9_007_199_254_740_991),
        Value::String(v) => !v.trim().is_empty(),
        Value::Array(values) => values.iter().all(scalar_domain),
        Value::Object(fields) => fields
            .iter()
            .all(|(k, v)| !k.trim().is_empty() && scalar_domain(v)),
        Value::Bool(_) | Value::Null => true,
    }
}
/// Used by WASM compilation; native duty admission in a later slice must call
/// this validator too. Hash and publisher authorization remain carrier checks.
pub fn validate_tables_json(input: &str) -> Vec<String> {
    if input.len() > crate::npc_documents::MAX_DOCUMENT_BYTES {
        return vec!["dm-srd: exceeds version-1 byte bound".into()];
    }
    let value: serde_json::Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(_) => return vec!["dm-srd: invalid table JSON".into()],
    };
    if !scalar_domain(&value) {
        return vec!["dm-srd: invalid scalar value".into()];
    }
    let tables: Tables = match serde_json::from_str(input) {
        Ok(tables) => tables,
        Err(_) => return vec!["dm-srd: invalid table schema".into()],
    };
    tables
        .validate()
        .err()
        .into_iter()
        .map(str::to_owned)
        .collect()
}
