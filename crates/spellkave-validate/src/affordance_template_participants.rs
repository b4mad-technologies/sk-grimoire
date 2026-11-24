//! Affordance template participant declarations (ADR-0132).

use spellkave_dsl::{compile_int_expression_with_macros, compile_predicate_with_macros};
use spellkave_schema::Manifest;
use std::collections::{BTreeMap, BTreeSet};

use crate::{Diagnostic, Severity};

// This code is distinct from the combat-grid metric diagnostic.
pub const MF_VAL_PARTICIPANT_DECLARATION: &str = "MF-VAL-336";
const MAX_DECLARATIONS_PER_TEMPLATE: usize = 32;

fn fatal(path: &str, message: String) -> Diagnostic {
    Diagnostic {
        code: MF_VAL_PARTICIPANT_DECLARATION.to_string(),
        severity: Severity::Fatal,
        message,
        path: path.to_string(),
        span: None,
        hint: None,
        details: None,
    }
}

pub fn validate_affordance_template_participants(
    manifest: &Manifest,
    live: Option<&crate::refs::LiveVocabulary>,
    diags: &mut Vec<Diagnostic>,
) {
    // sk-jay0.7.174.26: widened with `live.affordance_templates`
    // (existence-only, already-published) so a delta that redeclares a
    // participant without repeating its template still resolves.
    let local_templates: BTreeSet<&str> = manifest
        .affordance_templates
        .iter()
        .map(|row| row.template_id.as_str())
        .collect();
    let templates: BTreeSet<&str> = local_templates
        .iter()
        .copied()
        .chain(
            live.into_iter()
                .flat_map(|l| l.affordance_templates.keys().map(String::as_str)),
        )
        .collect();
    // `live.equipment_slots` already exists (LiveVocabulary, sk-9ll.3) —
    // reused here for the exact same reason as `affordance_templates` above.
    let slots: BTreeSet<&str> = manifest
        .equipment_slot_definitions
        .iter()
        .map(|row| row.slot_id.as_str())
        .chain(
            live.into_iter()
                .flat_map(|l| l.equipment_slots.keys().map(String::as_str)),
        )
        .collect();

    let mut by_template: BTreeMap<&str, Vec<&spellkave_schema::AffordanceTemplateParticipantRow>> =
        BTreeMap::new();
    for row in &manifest.affordance_template_participants {
        by_template
            .entry(row.template_id.as_str())
            .or_default()
            .push(row);
    }

    for (template_id, rows) in by_template {
        if !templates.contains(template_id) {
            diags.push(fatal(
                "affordance_template_participants.template_id",
                format!("participant declaration references unknown affordance template '{template_id}'"),
            ));
            continue;
        }
        if rows.len() > MAX_DECLARATIONS_PER_TEMPLATE {
            diags.push(fatal(
                "affordance_template_participants",
                format!(
                    "template '{template_id}' declares {} participants; at most {MAX_DECLARATIONS_PER_TEMPLATE} are admitted",
                    rows.len()
                ),
            ));
        }
        // The role-usage ("is this role actually referenced") check below
        // walks the template's OWN predicate/outcome/action/consequence-
        // frame-write chain, which only the LOCAL manifest can supply (no
        // live full-row reconstruction exists for affordance_template/
        // outcome_template/action_definition — same avoided-cascading-
        // false-fatal reasoning as `modifier_template`, `WorldDefinitions`'s
        // doc comment). A template resolved ONLY via `live`
        // (sk-jay0.7.174.26) has no local chain to walk, so that ONE check is
        // skipped for it — trusted as already validated when the template
        // and its participants were declared together. Slot existence
        // (`slots`, above) is a separate, already-widened live index and
        // still enforced either way.
        let template_is_local = local_templates.contains(template_id);
        let mut seen_roles = BTreeSet::new();
        let required_slots = required_equipped_slots(manifest, template_id);
        let participant_roles = referenced_participant_roles(manifest, template_id);
        for row in rows {
            if row.role_id.trim().is_empty() {
                diags.push(fatal(
                    "affordance_template_participants.role_id",
                    format!("template '{template_id}' has a blank participant role"),
                ));
                continue;
            }
            if !seen_roles.insert(row.role_id.as_str()) {
                diags.push(fatal(
                    "affordance_template_participants.role_id",
                    format!(
                        "template '{template_id}' declares duplicate participant role '{}'",
                        row.role_id
                    ),
                ));
            }
            if row.slot_id.trim().is_empty() {
                diags.push(fatal(
                    "affordance_template_participants.slot_id",
                    format!(
                        "template '{template_id}' role '{}' has a blank slot selector",
                        row.role_id
                    ),
                ));
                continue;
            }
            if !slots.contains(row.slot_id.as_str()) {
                diags.push(fatal(
                    "affordance_template_participants.slot_id",
                    format!(
                        "template '{template_id}' role '{}' references unknown equipment slot '{}'",
                        row.role_id, row.slot_id
                    ),
                ));
            }
            let used_as_source = required_slots.contains(row.slot_id.as_str());
            let used_as_participant = participant_roles.contains(row.role_id.as_str());
            if template_is_local && !used_as_source && !used_as_participant {
                diags.push(fatal(
                    "affordance_template_participants",
                    format!(
                        "template '{template_id}' role '{}' is unused: slot '{}' is not an exact equipped-item source and Participant({}) is not referenced",
                        row.role_id, row.slot_id, row.role_id
                    ),
                ));
            }
        }
    }
}

fn required_equipped_slots(manifest: &Manifest, template_id: &str) -> BTreeSet<String> {
    let mut slots = BTreeSet::new();
    let Some(template) = manifest
        .affordance_templates
        .iter()
        .find(|row| row.template_id == template_id)
    else {
        return slots;
    };
    collect_slots_from_source(&template.predicate_source, &mut slots);
    if let Some(outcome) = manifest
        .outcome_templates
        .iter()
        .find(|row| row.outcome_template_id == template.outcome_template_id)
    {
        collect_slots_from_source(&outcome.outcome_expression_source, &mut slots);
        if let Some(action) = manifest
            .action_definitions
            .iter()
            .find(|row| row.action_id == outcome.manipulation_action_id)
        {
            collect_slots_from_source(&action.resolution_predicate_source, &mut slots);
            collect_consequence_slots(manifest, &action.on_success, &mut slots);
            collect_consequence_slots(manifest, &action.on_failure, &mut slots);
        }
    }
    slots
}

fn collect_consequence_slots(
    manifest: &Manifest,
    consequence_id: &str,
    slots: &mut BTreeSet<String>,
) {
    if consequence_id.is_empty() {
        return;
    }
    for write in manifest
        .consequence_frame_writes
        .iter()
        .filter(|row| row.template_id == consequence_id)
    {
        collect_slots_from_source(&write.frame_write.to_string(), slots);
    }
}

fn collect_slots_from_source(source: &str, slots: &mut BTreeSet<String>) {
    if source.trim().is_empty() {
        return;
    }
    if let Ok(expr) = compile_predicate_with_macros(source, &[]) {
        slots.extend(expr.nodes.iter().flat_map(|node| {
            node.op
                .equipped_item_slots()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        }));
    }
    if let Ok(expr) = compile_int_expression_with_macros(source, &[]) {
        slots.extend(expr.nodes.iter().flat_map(|node| {
            node.op
                .equipped_item_slots()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        }));
    }
}

fn referenced_participant_roles(manifest: &Manifest, template_id: &str) -> BTreeSet<String> {
    let mut roles = BTreeSet::new();
    let Some(template) = manifest
        .affordance_templates
        .iter()
        .find(|row| row.template_id == template_id)
    else {
        return roles;
    };
    let Some(outcome) = manifest
        .outcome_templates
        .iter()
        .find(|row| row.outcome_template_id == template.outcome_template_id)
    else {
        return roles;
    };
    collect_participant_roles(&outcome.outcome_expression_source, &mut roles);
    if let Some(action) = manifest
        .action_definitions
        .iter()
        .find(|row| row.action_id == outcome.manipulation_action_id)
    {
        collect_participant_roles(&action.resolution_predicate_source, &mut roles);
        collect_consequence_participant_roles(manifest, &action.on_success, &mut roles);
        collect_consequence_participant_roles(manifest, &action.on_failure, &mut roles);
    }
    roles
}

fn collect_consequence_participant_roles(
    manifest: &Manifest,
    consequence_id: &str,
    roles: &mut BTreeSet<String>,
) {
    if consequence_id.is_empty() {
        return;
    }
    for write in manifest
        .consequence_frame_writes
        .iter()
        .filter(|row| row.template_id == consequence_id)
    {
        collect_participant_roles(&write.frame_write.to_string(), roles);
    }
}

fn collect_participant_roles(source: &str, roles: &mut BTreeSet<String>) {
    let mut rest = source;
    while let Some(start) = rest.find("Participant(") {
        rest = &rest[start + "Participant(".len()..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let role = rest[..end].trim();
        if !role.is_empty() {
            roles.insert(role.to_string());
        }
        rest = &rest[end + 1..];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spellkave_schema::AffordanceTemplateParticipantRow;

    fn empty_manifest() -> Manifest {
        Manifest::default()
    }

    /// sk-jay0.7.174.26: a delta redeclaring an `affordance_template_participants`
    /// row sends only the changed row, not its template — `live.affordance_templates`
    /// must resolve it instead of false-fataling MF-VAL-336
    /// ("references unknown affordance template").
    #[test]
    fn participant_resolves_via_live_affordance_template_without_local_declaration() {
        let mut manifest = empty_manifest();
        manifest.affordance_template_participants = vec![AffordanceTemplateParticipantRow {
            template_id: "srd/attack/melee/affordance".into(),
            role_id: "srd/instrument".into(),
            slot_id: "srd/slot/main-hand".into(),
            bundle_id: "srd".into(),
        }];
        let live = crate::refs::LiveVocabulary {
            affordance_templates: std::collections::BTreeMap::from([(
                "srd/attack/melee/affordance".to_string(),
                "srd".to_string(),
            )]),
            equipment_slots: std::collections::BTreeMap::from([(
                "srd/slot/main-hand".to_string(),
                "srd".to_string(),
            )]),
            ..Default::default()
        };
        let mut diags = Vec::new();
        validate_affordance_template_participants(&manifest, Some(&live), &mut diags);
        // Full round trip, matching the live proof: no local affordance_templates/
        // equipment_slot_definitions at all, yet the redeclare fully resolves —
        // template existence and slot existence via the live index, and the
        // role-usage check skipped (not falsely fatal) since the template's own
        // predicate/outcome/action chain isn't in this delta to walk.
        assert!(
            diags.is_empty(),
            "a participant whose template is live (not locally declared) must fully resolve: {diags:?}"
        );
    }

    /// Regression guard for the `template_is_local` gate: when the template
    /// IS locally declared (the ordinary authoring case, no `live` at all),
    /// an unused role must still fatal exactly as before — the live-only
    /// skip above must never weaken this for a fully local delta.
    #[test]
    fn unused_role_still_fatals_when_the_template_is_declared_locally() {
        let mut manifest = empty_manifest();
        manifest.affordance_templates = vec![serde_json::from_value(serde_json::json!({
            "template_id": "demo/swing",
            "bundle_id": "demo",
            "niche_id": "demo/n",
            "capability_id": "demo/c",
            "display_name": "Swing",
            "outcome_template_id": "demo/out",
        }))
        .unwrap()];
        manifest.equipment_slot_definitions = vec![serde_json::from_value(serde_json::json!({
            "slot_id": "demo/hand",
            "bundle_id": "demo",
            "label": "Hand",
        }))
        .unwrap()];
        manifest.affordance_template_participants = vec![AffordanceTemplateParticipantRow {
            template_id: "demo/swing".into(),
            role_id: "demo/unused_role".into(),
            slot_id: "demo/hand".into(),
            bundle_id: "demo".into(),
        }];
        let mut diags = Vec::new();
        validate_affordance_template_participants(&manifest, None, &mut diags);
        assert!(
            diags.iter().any(|d| d.message.contains("is unused")),
            "an unused role on a LOCALLY declared template must still fatal: {diags:?}"
        );
    }

    /// The live index is existence-only: a template absent from BOTH the
    /// delta and the live set must stay fatal (no vacuous check).
    #[test]
    fn participant_with_unknown_template_stays_fatal_even_with_a_live_index_present() {
        let mut manifest = empty_manifest();
        manifest.affordance_template_participants = vec![AffordanceTemplateParticipantRow {
            template_id: "srd/attack/melee/affordance".into(),
            role_id: "srd/instrument".into(),
            slot_id: "srd/slot/main-hand".into(),
            bundle_id: "srd".into(),
        }];
        let live = crate::refs::LiveVocabulary {
            affordance_templates: std::collections::BTreeMap::from([(
                "some/other/template".to_string(),
                "srd".to_string(),
            )]),
            ..Default::default()
        };
        let mut diags = Vec::new();
        validate_affordance_template_participants(&manifest, Some(&live), &mut diags);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unknown affordance template")),
            "a template absent from both the delta and the live index must stay fatal: {diags:?}"
        );
    }

    #[test]
    fn blank_role_is_rejected() {
        let mut manifest = empty_manifest();
        manifest.affordance_templates = vec![serde_json::from_value(serde_json::json!({
            "template_id": "demo/swing",
            "bundle_id": "demo",
            "niche_id": "demo/n",
            "capability_id": "demo/c",
            "display_name": "Swing",
            "outcome_template_id": "demo/out",
        }))
        .unwrap()];
        manifest.affordance_template_participants = vec![AffordanceTemplateParticipantRow {
            template_id: "demo/swing".into(),
            role_id: " ".into(),
            slot_id: "demo/hand".into(),
            bundle_id: "demo".into(),
        }];
        let mut diags = Vec::new();
        validate_affordance_template_participants(&manifest, None, &mut diags);
        assert!(
            diags
                .iter()
                .any(|d| d.code == MF_VAL_PARTICIPANT_DECLARATION),
            "expected participant declaration diagnostic, got {diags:?}"
        );
    }
}
