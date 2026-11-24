//! DM-only rewarded-quest prerequisites over an independently admitted shared
//! catalog. The catalog schema and generic quest driver remain unchanged.
use crate::dm_duties::DutySource;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// `catalog` is the existing admitted source/catalog object containing quests;
/// its authoritative shape/hash/reference admission is a separate requirement.
pub fn validate_quest_prerequisites(
    source: &DutySource,
    catalog: &Value,
) -> Result<(), Vec<String>> {
    let quests = catalog["quests"]
        .as_array()
        .ok_or_else(|| vec!["dm-quest-references: missing admitted catalog quests".into()])?;
    let catalog_ids: BTreeMap<_, _> = quests
        .iter()
        .filter_map(|row| row["id"].as_str().map(|id| (id, row)))
        .collect();
    let gates: BTreeMap<_, _> = source
        .quest_prerequisites
        .iter()
        .map(|row| (row.quest_ref.as_str(), &row.requires_rewarded_quests))
        .collect();
    let mut errors = vec![];
    if gates.len() != source.quest_prerequisites.len() {
        errors.push("dm-quest-references: duplicate gate".into());
    }
    for gate in &source.quest_prerequisites {
        match catalog_ids.get(gate.quest_ref.as_str()) {
            Some(quest) if quest["giverKey"] == source.body_key => {}
            _ => errors.push(format!(
                "dm-quest-references: gated quest is unknown or not DM-owned: {}",
                gate.quest_ref
            )),
        }
        let mut seen = BTreeSet::new();
        if gate.requires_rewarded_quests.is_empty() {
            errors.push("dm-quest-references: empty prerequisite list".into());
        }
        for required in &gate.requires_rewarded_quests {
            if required == &gate.quest_ref
                || !catalog_ids.contains_key(required.as_str())
                || !seen.insert(required)
            {
                errors.push(format!(
                    "dm-quest-references: unknown, self or repeated prerequisite {required}"
                ));
            }
        }
    }
    // Iterative cycle detection bounds work by the admitted source population;
    // hostile deep chains cannot overflow a recursive call stack.
    let mut incoming: BTreeMap<_, usize> = gates.keys().map(|id| (*id, 0)).collect();
    for required in gates.values().flat_map(|values| values.iter()) {
        if let Some(count) = incoming.get_mut(required.as_str()) {
            *count += 1;
        }
    }
    let mut queue: VecDeque<_> = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut visited = 0;
    while let Some(id) = queue.pop_front() {
        visited += 1;
        for required in &gates[id][..] {
            if let Some(count) = incoming.get_mut(required.as_str()) {
                *count -= 1;
                if *count == 0 {
                    queue.push_back(required.as_str());
                }
            }
        }
    }
    if visited != gates.len() {
        errors.push("dm-quest-references: cyclic prerequisites".into());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Close the ordered board against independently admitted catalog ownership.
/// Shape admission separately requires nonempty, unique, bounded references.
pub fn validate_quest_board(
    board: &crate::dm_duties::QuestBoard,
    body_key: &str,
    catalog: &Value,
) -> Result<(), Vec<String>> {
    let quests = catalog["quests"]
        .as_array()
        .ok_or_else(|| vec!["dm-quest-board: missing admitted catalog quests".into()])?;
    let owned: BTreeSet<_> = quests
        .iter()
        .filter(|quest| quest["giverKey"] == body_key)
        .filter_map(|quest| quest["id"].as_str())
        .collect();
    let errors: Vec<_> = board
        .quest_refs
        .iter()
        .filter(|id| !owned.contains(id.as_str()))
        .map(|id| format!("dm-quest-board: unknown or foreign quest {id}"))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Presentation text is keyed by exactly the board's already admitted refs.
/// Keys supply no authority, availability, ordering or executable selectors.
pub fn validate_quest_board_lines(
    board: &crate::dm_duties::QuestBoard,
    presentation: &crate::dm_presentation::QuestBoard,
) -> Result<(), Vec<String>> {
    let expected: BTreeSet<_> = board.quest_refs.iter().map(String::as_str).collect();
    let actual: BTreeSet<_> = presentation.lines.keys().map(String::as_str).collect();
    if expected == actual {
        Ok(())
    } else {
        Err(vec![
            "dm-quest-board: presentation line keys differ from duty board refs".into(),
        ])
    }
}
