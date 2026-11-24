//! The data shape of a guided dialogue: an ordered list of steps an NPC asks.
//!
//! Parsing is only a shape check. What a `source` or an `input` means belongs
//! to the NPC kind that carries the guide; the kind passes the ids it knows to
//! [`validate`].
use serde::Deserialize;
use std::collections::BTreeSet;

/// Most steps one guide may hold. A bound for the validator, not a rule.
pub const MAX_STEPS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guide {
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Stable and opaque: no engine and no client branches on it.
    pub id: String,
    /// Short name for the outline.
    pub title: String,
    /// Named option source, resolved by the kind.
    #[serde(default)]
    pub source: Option<String>,
    /// Inline options, for a talk whose answers are fixed text.
    #[serde(default)]
    pub options: Option<Vec<InlineOption>>,
    /// How many inline options are picked. A source states its own count.
    #[serde(default)]
    pub pick: Option<PickCount>,
    /// The step is answered by one of the kind's inputs, not by a pick.
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub show_when: Option<ShowWhen>,
    /// Steps this one hangs on: a different answer there drops the answers here.
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "yes")]
    pub reopen: bool,
    #[serde(default)]
    pub paged: bool,
    #[serde(default)]
    pub announce_remaining: bool,
    #[serde(default = "yes")]
    pub listed: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InlineOption {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "ref")]
    pub reference: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PickCount {
    pub min: u32,
    pub max: u32,
}

/// One atom, never an expression (ADR-0165 D-2: no general Boolean
/// expressions in NPC documents). The id names another step of the guide.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ShowWhen {
    Answered(String),
    Unanswered(String),
}

impl ShowWhen {
    pub fn step(&self) -> &str {
        match self {
            Self::Answered(id) | Self::Unanswered(id) => id,
        }
    }
}

impl Guide {
    pub fn step(&self, id: &str) -> Option<&Step> {
        self.steps.iter().find(|step| step.id == id)
    }
    /// The step a source belongs to. [`validate`] admits a source once only.
    pub fn step_of_source(&self, source: &str) -> Option<&Step> {
        self.steps
            .iter()
            .find(|step| step.source.as_deref() == Some(source))
    }
}

/// Shape rules of one guide. `sources` and `inputs` are the ids the carrying
/// kind can resolve.
pub fn validate(guide: &Guide, sources: &[&str], inputs: &[&str]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut require = |valid: bool, message: String| {
        if !valid {
            errors.push(format!("guide: {message}"));
        }
    };
    require(
        !guide.steps.is_empty() && guide.steps.len() <= MAX_STEPS,
        format!("a guide holds 1 to {MAX_STEPS} steps"),
    );
    let mut seen = BTreeSet::new();
    let mut used = BTreeSet::new();
    for step in &guide.steps {
        let id = &step.id;
        require(
            !id.trim().is_empty() && id.len() <= 64 && !step.title.trim().is_empty(),
            format!("step '{id}' needs a bounded id and a title"),
        );
        let kinds = usize::from(step.source.is_some())
            + usize::from(step.options.is_some())
            + usize::from(step.input.is_some());
        require(
            kinds == 1,
            format!("step '{id}' takes exactly one of source, options, input"),
        );
        if let Some(source) = &step.source {
            require(
                sources.contains(&source.as_str()),
                format!("step '{id}' names the unknown source '{source}'"),
            );
            require(
                used.insert(source.as_str()),
                format!("source '{source}' is used by more than one step"),
            );
        }
        if let Some(input) = &step.input {
            require(
                inputs.contains(&input.as_str()),
                format!("step '{id}' names the unknown input '{input}'"),
            );
        }
        if let Some(options) = &step.options {
            let ids: BTreeSet<_> = options.iter().map(|option| option.id.as_str()).collect();
            require(
                !options.is_empty()
                    && ids.len() == options.len()
                    && options
                        .iter()
                        .all(|option| !option.id.is_empty() && !option.label.is_empty()),
                format!("step '{id}' needs distinct inline options with id and label"),
            );
            let pick = step.pick.unwrap_or(PickCount { min: 1, max: 1 });
            require(
                pick.min >= 1 && pick.min <= pick.max && pick.max as usize <= options.len(),
                format!("step '{id}' picks 1 <= min <= max <= options"),
            );
        } else {
            require(
                step.pick.is_none(),
                format!("step '{id}': pick counts belong to inline options only"),
            );
        }
        // An earlier step only: a step cannot hang on what is asked after it.
        for dependency in &step.depends_on {
            require(
                seen.contains(dependency.as_str()),
                format!("step '{id}' depends on '{dependency}', which is not an earlier step"),
            );
        }
        seen.insert(id.as_str());
    }
    require(
        seen.len() == guide.steps.len(),
        "step ids must be distinct".into(),
    );
    for step in &guide.steps {
        if let Some(rule) = &step.show_when {
            require(
                rule.step() != step.id && seen.contains(rule.step()),
                format!(
                    "step '{}': show_when names '{}', which is not another step",
                    step.id,
                    rule.step()
                ),
            );
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guide(json: serde_json::Value) -> Guide {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn a_plain_guide_passes_and_defaults_apply() {
        let guide = guide(serde_json::json!({"steps": [
            {"id": "a", "title": "A", "source": "list-a"},
            {"id": "b", "title": "B", "source": "list-b", "depends_on": ["a"]},
            {"id": "c", "title": "C", "input": "text", "show_when": {"unanswered": "d"}},
            {"id": "d", "title": "D", "options": [{"id": "x", "label": "X"}]}
        ]}));
        assert!(guide.steps[0].reopen && guide.steps[0].listed && !guide.steps[0].paged);
        assert_eq!(validate(&guide, &["list-a", "list-b"], &["text"]), [""; 0]);
        assert_eq!(guide.step_of_source("list-b").unwrap().id, "b");
    }

    #[test]
    fn each_shape_rule_refuses() {
        let bad = |json: serde_json::Value, needle: &str| {
            let errors = validate(&guide(json), &["s"], &["i"]);
            assert!(
                errors.iter().any(|error| error.contains(needle)),
                "{needle}: {errors:?}"
            );
        };
        bad(serde_json::json!({"steps": []}), "1 to");
        bad(
            serde_json::json!({"steps": [{"id": "a", "title": "A"}]}),
            "exactly one",
        );
        bad(
            serde_json::json!({"steps": [{"id": "a", "title": "A", "source": "s", "input": "i"}]}),
            "exactly one",
        );
        bad(
            serde_json::json!({"steps": [{"id": "a", "title": "A", "source": "t"}]}),
            "unknown source",
        );
        bad(
            serde_json::json!({"steps": [{"id": "a", "title": "A", "input": "t"}]}),
            "unknown input",
        );
        bad(
            serde_json::json!({"steps": [
                {"id": "a", "title": "A", "source": "s"},
                {"id": "b", "title": "B", "source": "s"}]}),
            "more than one step",
        );
        bad(
            serde_json::json!({"steps": [
                {"id": "a", "title": "A", "source": "s", "depends_on": ["b"]},
                {"id": "b", "title": "B", "input": "i"}]}),
            "not an earlier step",
        );
        bad(
            serde_json::json!({"steps": [
                {"id": "a", "title": "A", "source": "s", "show_when": {"answered": "a"}}]}),
            "not another step",
        );
        bad(
            serde_json::json!({"steps": [
                {"id": "a", "title": "A", "source": "s"},
                {"id": "a", "title": "B", "input": "i"}]}),
            "distinct",
        );
        bad(
            serde_json::json!({"steps": [{"id": "a", "title": "A",
                "options": [{"id": "x", "label": "X"}], "pick": {"min": 1, "max": 2}}]}),
            "min <= max <= options",
        );
        bad(
            serde_json::json!({"steps": [{"id": "a", "title": "A", "source": "s",
                "pick": {"min": 1, "max": 1}}]}),
            "inline options only",
        );
    }

    #[test]
    fn show_when_is_one_atom() {
        let nested = serde_json::json!({"steps": [{"id": "a", "title": "A", "source": "s",
            "show_when": {"all": [{"answered": "b"}]}}]});
        assert!(serde_json::from_value::<Guide>(nested).is_err());
    }
}
