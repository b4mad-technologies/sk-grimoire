//! Canonical reference data for the opt-in DM compiler. This is independent of
//! scoped runtime observations and grants no publication or action authority.
use serde_json::Value;
use spellkave_schema::{
    row_identity::{manifest_row_identities, row_key_of, SET_LEVEL_FIELD},
    ConsequenceFrameWriteRow, Manifest,
};
use std::collections::BTreeMap;

/// Constructed from complete typed reference roots. Authority-only projections
/// must never be passed in place of these roots. Row identities come from the
/// existing shared schema registry, including set-level consequence writes.
pub struct CanonicalWorlds {
    rows: BTreeMap<(String, String), Value>,
    row_order: Vec<(String, String)>,
    writes: BTreeMap<String, Vec<ConsequenceFrameWriteRow>>,
}

impl CanonicalWorlds {
    pub fn new(worlds: &[Manifest]) -> Result<Self, Vec<String>> {
        if worlds.is_empty() {
            return Err(vec!["dm-references: missing canonical roots".into()]);
        }
        let mut index = Self {
            rows: BTreeMap::new(),
            row_order: vec![],
            writes: BTreeMap::new(),
        };
        let mut errors = vec![];
        for world in worlds {
            // Shared identity enumeration excludes manifest directives and
            // reports unregistered row kinds separately. Use its population
            // rather than maintaining another directive/row-kind list here.
            let identities = manifest_row_identities(world);
            for (table, _) in &identities.untrackable {
                errors.push(format!("dm-references: unregistered row kind {table}"));
            }
            let tracked: std::collections::BTreeSet<_> = identities
                .rows
                .iter()
                .map(|row| row.kind.as_str())
                .collect();
            let value = serde_json::to_value(world).expect("typed canonical manifest serializes");
            for (table, value) in value.as_object().expect("manifest is an object") {
                let Some(rows) = value.as_array() else {
                    continue;
                };
                // UAC14380: boundary definitions have no kernel storage and
                // resolve per bundle through depends_on, not this global key.
                // See action_economy.rs:18-29. Duties contain no boundary refs;
                // any attempted index lookup refuses rather than bypassing the
                // existing bundle-scoped canonical validation.
                if table == "boundary_definitions"
                    || table == SET_LEVEL_FIELD
                    || !tracked.contains(table.as_str())
                {
                    continue;
                }
                for row in rows {
                    let key = match canonical_row_key(table, row) {
                        Ok(key) => key,
                        Err(error) => {
                            errors.push(error);
                            continue;
                        }
                    };
                    let identity = (table.clone(), key.clone());
                    match index.rows.get(&identity) {
                        Some(previous) if previous != row => {
                            errors.push(format!("dm-references: conflicting {table} {key}"))
                        }
                        Some(_) => {}
                        None => {
                            index.row_order.push(identity.clone());
                            index.rows.insert(identity, row.clone());
                        }
                    }
                }
            }
            // Kernel applies these as a whole ordered set per template, never
            // as independently upserted ordinal rows. Reject partial/conflicting
            // copies across roots rather than merging them into a new recipe.
            let mut sets: BTreeMap<String, Vec<ConsequenceFrameWriteRow>> = BTreeMap::new();
            for row in &world.consequence_frame_writes {
                sets.entry(row.template_id.clone())
                    .or_default()
                    .push(row.clone());
            }
            for (template, mut writes) in sets {
                writes.sort_by_key(|row| row.ordinal);
                writes.dedup();
                if writes
                    .windows(2)
                    .any(|pair| pair[0].ordinal == pair[1].ordinal)
                {
                    errors.push(format!(
                        "dm-references: conflicting consequence ordinal {template}"
                    ));
                }
                match index.writes.get(&template) {
                    Some(previous) if previous != &writes => errors.push(format!(
                        "dm-references: conflicting consequence set {template}"
                    )),
                    Some(_) => {}
                    None => {
                        index.writes.insert(template, writes);
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(index)
        } else {
            Err(errors)
        }
    }

    pub fn row(&self, table: &str, key: &str) -> Result<&Value, String> {
        require_indexed_kind(table)?;
        self.rows
            .get(&(table.into(), key.into()))
            .ok_or_else(|| format!("dm-references: missing {table} {key}"))
    }

    pub fn keyed_row(&self, table: &str, identity: &Value) -> Result<&Value, String> {
        let key = canonical_row_key(table, identity)?;
        self.row(table, &key)
    }

    pub fn rows<'a>(&'a self, table: &'a str) -> Result<impl Iterator<Item = &'a Value>, String> {
        require_indexed_kind(table)?;
        Ok(self
            .row_order
            .iter()
            .filter(move |(kind, _)| kind == table)
            .map(|identity| &self.rows[identity]))
    }

    pub fn action_writes(&self, action_id: &str) -> Result<&[ConsequenceFrameWriteRow], String> {
        let action = self.row("action_definitions", action_id)?;
        let template = action["on_success"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                format!("dm-references: action has no success consequence {action_id}")
            })?;
        self.row("consequence_templates", template)?;
        self.writes
            .get(template)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("dm-references: missing consequence writes {template}"))
    }

    /// For recipes/handle writes whose entire promised effect is the ordered
    /// frame-write set. Extra effect or trace channels cannot hide alongside
    /// otherwise matching writes.
    pub fn pure_action_writes(
        &self,
        action_id: &str,
    ) -> Result<&[ConsequenceFrameWriteRow], String> {
        let writes = self.action_writes(action_id)?;
        let action = self.row("action_definitions", action_id)?;
        if action
            .get("on_failure")
            .is_some_and(|value| value.as_str().is_none_or(|id| !id.is_empty()))
        {
            return Err(format!(
                "dm-references: unexpected action failure consequence {action_id}"
            ));
        }
        let template = self.row(
            "consequence_templates",
            action["on_success"]
                .as_str()
                .ok_or("missing action success template")?,
        )?;
        if template
            .get("effect_refs_csv")
            .is_some_and(|value| value.as_str().is_none_or(|csv| !csv.trim().is_empty()))
        {
            return Err(format!(
                "dm-references: unexpected additional action effects {action_id}"
            ));
        }
        if let Some(traces) = template.get("traces_json") {
            let raw = traces.as_str().ok_or("invalid typed action traces")?;
            let traces = crate::dm_document_authority::unique_json(raw)
                .map_err(|errors| errors.join("; "))?;
            if traces != serde_json::json!([]) {
                return Err(format!(
                    "dm-references: unexpected additional action traces {action_id}"
                ));
            }
        }
        Ok(writes)
    }
}

fn canonical_row_key(table: &str, row: &Value) -> Result<String, String> {
    row_key_of(table, row).ok_or_else(|| format!("dm-references: unregistered row kind {table}"))
}

fn require_indexed_kind(table: &str) -> Result<(), String> {
    if table == "boundary_definitions" {
        Err("dm-references: boundary definitions require bundle-scoped canonical validation".into())
    } else {
        Ok(())
    }
}
