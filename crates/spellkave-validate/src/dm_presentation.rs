//! Presentation-only DM data. No action, target, quantity or authority selectors.
//! Full document hashes, publication and world references are separate admission.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_TEMPLATE_PARTS: usize = 64;
pub const MAX_TEXT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub schema_version: u32,
    pub voice: Voice,
    pub creation: Creation,
    pub level_up: LevelUp,
    pub companion_offer: CompanionOffer,
    pub telepathic_invitation: String,
    pub quest_board: QuestBoard,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestBoard {
    pub lines: BTreeMap<String, QuestLines>,
}
/// The existing giver line vocabulary, without any executable selector.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestLines {
    pub greeting: String,
    pub accept: String,
    pub decline: String,
    pub complete: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub display_name: String,
    pub persona_name: String,
    pub persona_slug: String,
    pub possessive: String,
    pub referral_name: String,
    pub return_place_name: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelUp {
    pub class_flavour: Template,
    pub offer_with_class_flavour: Template,
    pub explanation: String,
    pub direction: String,
    pub offer: String,
    pub acknowledgement: String,
    pub closing: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub rule: String,
    pub summary: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Speech {
    pub rule: String,
    pub summary: String,
    pub line: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionOffer {
    pub offer: Speech,
    pub accepted: Speech,
    pub declined: Speech,
    pub agreement_accept: Label,
    pub agreement_decline: Label,
    pub agreement_reopen: Label,
    pub open_answers: Label,
    pub close_answers: Label,
    pub react_abstain: Label,
    pub agreement_unavailable: Label,
    pub apply_pending: Label,
    pub idle: Label,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Creation {
    pub welcome: String,
    pub class_acknowledgement: Template,
    pub species_acknowledgement: Template,
    pub background_acknowledgement: Template,
    pub confirm_rejection: Template,
    pub class_array_proposal: Template,
    pub ability_drafting_paths_offer: String,
    pub slot_menu_offer: Template,
    pub name_contract_offer: String,
    pub scores_adopt_acknowledgement: Template,
    pub closing: String,
    pub name_readback: Template,
    pub name_unparseable: String,
    pub appearance_readback: Template,
    pub appearance_unparseable: String,
    pub backstory_readback: Template,
    pub backstory_unparseable: String,
    pub parse_readback: Template,
    pub parse_unparseable: String,
    pub parse_unaffordable: Template,
    pub progress_ready: String,
    pub progress_missing: Template,
}

/// A flat concatenation of text and preformatted display values. No condition,
/// expression, recursive node, reference lookup or executable callback.
#[derive(Clone, Debug, Deserialize)]
#[serde(transparent)]
pub struct Template(pub Vec<Part>);
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "snake_case")]
pub enum Part {
    Text { text: String },
    Value { name: String },
}
impl Template {
    fn admits(&self, allowed: &[&str]) -> bool {
        !self.0.is_empty()
            && self.0.len() <= MAX_TEMPLATE_PARTS
            && self.0.iter().all(|part| match part {
                Part::Text { text } => text.len() <= MAX_TEXT_BYTES,
                Part::Value { name } => allowed.contains(&name.as_str()),
            })
            && self.0.iter().any(|part| match part {
                Part::Text { text } => !text.trim().is_empty(),
                Part::Value { .. } => true,
            })
    }

    /// Values must be exactly the template's named display inputs. Their
    /// calculation/order belongs to the caller's admitted formatting policy.
    pub fn render(&self, values: &BTreeMap<String, String>) -> Result<String, &'static str> {
        let required: BTreeSet<_> = self
            .0
            .iter()
            .filter_map(|part| match part {
                Part::Value { name } => Some(name.as_str()),
                Part::Text { .. } => None,
            })
            .collect();
        if values.keys().map(String::as_str).collect::<BTreeSet<_>>() != required {
            return Err("dm-presentation: display inputs mismatch");
        }
        let mut result = String::new();
        for part in &self.0 {
            let text = match part {
                Part::Text { text } => text,
                Part::Value { name } => &values[name],
            };
            if result.len().saturating_add(text.len()) > crate::npc_documents::MAX_DOCUMENT_BYTES {
                return Err("dm-presentation: rendered text exceeds byte bound");
            }
            result.push_str(text);
        }
        Ok(result)
    }
}
impl Presentation {
    fn admits(&self) -> bool {
        let c = &self.creation;
        self.schema_version == 1
            && !self.quest_board.lines.is_empty()
            && self
                .quest_board
                .lines
                .keys()
                .all(|id| !id.trim().is_empty() && id.len() <= crate::dm_duties::MAX_REF_BYTES)
            && [
                (
                    &self.level_up.class_flavour,
                    &["class_name", "primary_ability"][..],
                ),
                (
                    &self.level_up.offer_with_class_flavour,
                    &["offer", "class_flavour"][..],
                ),
                (&c.class_acknowledgement, &["class_name"][..]),
                (&c.species_acknowledgement, &["species_name"][..]),
                (&c.background_acknowledgement, &["background_name"][..]),
                (&c.confirm_rejection, &["missing"][..]),
                (&c.class_array_proposal, &["class_name", "scores"][..]),
                (&c.slot_menu_offer, &["menu_kind", "labels"][..]),
                (&c.scores_adopt_acknowledgement, &["scores"][..]),
                (&c.name_readback, &["name"][..]),
                (&c.appearance_readback, &["appearance"][..]),
                (&c.backstory_readback, &["backstory"][..]),
                (&c.parse_readback, &["patch", "scores", "total_cost"][..]),
                (&c.parse_unaffordable, &["scores", "total_cost"][..]),
                (&c.progress_missing, &["missing"][..]),
            ]
            .iter()
            .all(|(template, allowed)| template.admits(allowed))
    }
}

fn text_domain(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES
        }
        serde_json::Value::Array(values) => values.iter().all(text_domain),
        serde_json::Value::Object(values) => values.iter().all(|(key, value)| {
            if key == "text"
                && values.get("kind").and_then(serde_json::Value::as_str) == Some("text")
            {
                // Literal whitespace between display values is meaningful.
                value
                    .as_str()
                    .is_some_and(|text| !text.is_empty() && text.len() <= MAX_TEXT_BYTES)
            } else {
                text_domain(value)
            }
        }),
        serde_json::Value::Number(value) => value.as_u64() == Some(1),
        _ => false,
    }
}

/// Component/compile check only. Publication and activation stages and their
/// negative controls remain owed by sk-jay0.3.33.7.7 with the slices that build
/// those stages. No mounted engine, read grant or publication hash is admitted.
pub fn validate_presentation_json(input: &str) -> Vec<String> {
    if input.len() > crate::npc_documents::MAX_DOCUMENT_BYTES {
        return vec!["dm-presentation: exceeds version-1 byte bound".into()];
    }
    let parsed = serde_json::from_str::<Presentation>(input);
    let value = crate::dm_document_authority::unique_json(input);
    if !parsed.is_ok_and(|value| value.admits()) || !value.is_ok_and(|value| text_domain(&value)) {
        return vec!["dm-presentation: invalid presentation schema".into()];
    }
    Vec::new()
}
