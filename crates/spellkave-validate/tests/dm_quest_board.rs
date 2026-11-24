use serde_json::{json, Value};
use spellkave_validate::{dm_duties::parse_source_shape_json, dm_source::parse_source_json};

fn source() -> Value {
    serde_json::from_str(include_str!("fixtures/dm-carrier-source-v1.generated.json")).unwrap()
}
fn refs(source: &Value) -> Vec<String> {
    serde_json::from_value(source["duties"]["quest_board"]["quest_refs"].clone()).unwrap()
}
fn foreign(source: &Value) -> String {
    source["catalog"]["source"]["quests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["giverKey"] != source["body_key"])
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .into()
}

// keep(K5): generated TS board data is checked against independently compiled
// prompt lines; arbitrary admitted order is retained without sorting.
#[test]
fn actual_board_retains_order_and_exact_authored_prompt_lines() {
    let source = source();
    let admitted = parse_source_json(&source.to_string()).unwrap();
    let original = refs(&source);
    assert_eq!(admitted.duties.quest_board.quest_refs, original);
    for id in &original {
        let quest = source["catalog"]["source"]["quests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["id"] == *id)
            .unwrap();
        assert_eq!(
            source["presentation"]["quest_board"]["lines"][id],
            quest["prompt_lines"]
        );
    }
    let mut reversed = source.clone();
    let mut expected = original;
    expected.reverse();
    reversed["duties"]["quest_board"]["quest_refs"] = json!(expected);
    assert_eq!(
        parse_source_json(&reversed.to_string())
            .unwrap()
            .duties
            .quest_board
            .quest_refs,
        expected
    );
}

// keep(K2): isolate component-domain refusals from catalog and key closure.
#[test]
fn board_reference_domain_refusals_are_exact() {
    let source = source();
    let ids = refs(&source);
    let id = &ids[0];
    for value in [json!([]), json!([id, id]), json!([" "])] {
        let mut duties = source["duties"].clone();
        duties["quest_board"]["quest_refs"] = value;
        assert_eq!(
            parse_source_shape_json(&duties.to_string()).unwrap_err(),
            ["dm-duties: quest board must contain nonempty distinct bounded references"]
        );
    }
}

// keep(K2): each full-source refusal has exactly its expected cause, including
// ownership with matching keys and missing/extra keys with valid ownership.
#[test]
fn board_source_refuses_invalid_refs_lines_and_privilege_fields() {
    for case in 0..10 {
        let mut changed = source();
        let own = refs(&changed)[0].clone();
        let foreign = foreign(&changed);
        let expected = match case {
            0 | 1 => {
                let id = if case == 0 { "missing/quest" } else { &foreign };
                changed["duties"]["quest_board"]["quest_refs"][0] = json!(id);
                let lines = changed["presentation"]["quest_board"]["lines"]
                    .as_object_mut()
                    .unwrap();
                let prose = lines.remove(&own).unwrap();
                lines.insert(id.into(), prose);
                format!("dm-quest-board: unknown or foreign quest {id}")
            }
            2 => {
                changed["presentation"]["quest_board"]["lines"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&own);
                "dm-quest-board: presentation line keys differ from duty board refs".into()
            }
            3 => {
                changed["presentation"]["quest_board"]["lines"][&foreign] =
                    changed["presentation"]["quest_board"]["lines"][&own].clone();
                "dm-quest-board: presentation line keys differ from duty board refs".into()
            }
            4 | 5 | 8 => {
                let lines = &mut changed["presentation"]["quest_board"]["lines"][&own];
                match case {
                    4 => lines["greeting"] = json!(" "),
                    5 => lines["complete"] = Value::Null,
                    _ => {
                        lines["greeting"] = json!(
                            "x".repeat(spellkave_validate::dm_presentation::MAX_TEXT_BYTES + 1)
                        )
                    }
                }
                "dm-presentation: invalid presentation schema".into()
            }
            6 => {
                changed["presentation"]["quest_board"]["lines"][&own]["action_id"] =
                    json!("forbidden");
                "dm-source: invalid shape: unknown field `action_id`, expected one of `greeting`, `accept`, `decline`, `complete`".into()
            }
            7 => {
                changed["duties"]["quest_board"]["lines"] = json!({});
                "dm-source: invalid shape: unknown field `lines`, expected `quest_refs`".into()
            }
            _ => {
                changed["presentation"]["quest_board"]["lines"][&own]["greeting"] =
                    json!("Different source prose");
                format!("dm-quest-board: presentation differs from catalog prompt lines for {own}")
            }
        };
        assert_eq!(
            parse_source_json(&changed.to_string()).unwrap_err(),
            [expected],
            "case {case}"
        );
    }
}
