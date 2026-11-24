//! Complete source shape checks; not publication or reference-closure proof.
use spellkave_validate::{dm_document_authority::unique_json, dm_duties::parse_source_shape_json};

/// The duty source with every SRD class on the creation menu, frozen from the
/// exporter's output before the adventure narrowed its menu
/// (sk-jay0.1.55.60.19). The generated fixture now carries the adventure's
/// four classes; the tests that enumerate the complete class domain keep
/// running on this one. It follows the same worlds as the generated file: when
/// the worlds change, re-export with all twelve classes offered.
const FULL_CLASS_SOURCE: &str = include_str!("fixtures/dm-duty-source-v1.twelve-classes.json");

// K2: the full unchanged-TS export must fit the typed inventory before closure.
#[test]
fn captured_source_matches_the_complete_duty_shape() {
    parse_source_shape_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
}

// K2 (sk-jay0.1.55.60.19): the creation menu may offer a strict subset of the
// admitted SRD classes, never a class outside them and never none.
#[test]
fn creation_classes_are_a_nonempty_subset_of_the_admitted_srd_classes() {
    let original = unique_json(FULL_CLASS_SOURCE).unwrap();
    let all: Vec<String> = original["creation"]["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|class| class["slug"].as_str().unwrap().to_owned())
        .collect();
    assert!(all.len() > 4, "the fixture carries the full SRD class list");
    // Keep the first four classes and only their kits: admitted.
    let mut four = original.clone();
    let kept: Vec<String> = all[..4].to_vec();
    four["creation"]["classes"]
        .as_array_mut()
        .unwrap()
        .retain(|class| kept.iter().any(|slug| class["slug"] == slug.as_str()));
    four["creation"]["kits"]
        .as_array_mut()
        .unwrap()
        .retain(|kit| kept.iter().any(|slug| kit["class_slug"] == slug.as_str()));
    assert_eq!(four["creation"]["classes"].as_array().unwrap().len(), 4);
    assert_eq!(
        four["srd"]["classes"].as_array().unwrap().len(),
        original["srd"]["classes"].as_array().unwrap().len(),
        "the SRD class tables stay complete"
    );
    parse_source_shape_json(&four.to_string()).unwrap();
    // A kit of a class that left the menu is still refused.
    let mut stray_kit = four.clone();
    let dropped = original["creation"]["kits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|kit| !kept.iter().any(|slug| kit["class_slug"] == slug.as_str()))
        .unwrap()
        .clone();
    stray_kit["creation"]["kits"]
        .as_array_mut()
        .unwrap()
        .push(dropped);
    assert!(parse_source_shape_json(&stray_kit.to_string())
        .unwrap_err()
        .contains(&"dm-duties: kit class is outside the menu".to_string()));
    // A class the SRD data does not admit is refused, alone or as an addition.
    for base in [&four, &original] {
        let mut unknown = base.clone();
        let mut extra = unknown["creation"]["classes"][0].clone();
        extra["slug"] = "not-an-srd-class".into();
        unknown["creation"]["classes"]
            .as_array_mut()
            .unwrap()
            .push(extra);
        assert!(parse_source_shape_json(&unknown.to_string())
            .unwrap_err()
            .contains(&"dm-duties: creation class is outside admitted SRD data".to_string()));
    }
    // No class at all is refused.
    let mut none = original.clone();
    none["creation"]["classes"] = serde_json::json!([]);
    none["creation"]["kits"] = serde_json::json!([]);
    assert!(parse_source_shape_json(&none.to_string())
        .unwrap_err()
        .contains(&"dm-duties: empty or duplicate classes".to_string()));
}

// K2: supported creation menus cannot omit or duplicate an admitted generated
// SRD origin while retaining otherwise valid individual references; classes
// cannot be duplicated.
#[test]
fn creation_origin_domains_equal_the_complete_admitted_srd_component() {
    let original = unique_json(FULL_CLASS_SOURCE).unwrap();
    for field in ["classes", "species", "backgrounds"] {
        for duplicate in [false, true] {
            let mut changed = original.clone();
            let values = changed["creation"][field].as_array_mut().unwrap();
            if duplicate {
                values.push(values[0].clone());
            } else if field == "classes" {
                // Omitting a class is the subset case, covered above.
                continue;
            } else {
                values.pop().unwrap();
            }
            let error = parse_source_shape_json(&changed.to_string()).unwrap_err();
            if !duplicate {
                assert_eq!(
                    error,
                    vec![
                        "dm-duties: creation origin domain differs from admitted SRD data"
                            .to_string()
                    ],
                    "{field}"
                );
            }
        }
    }
}

// K2: typed shape cannot carry extended authority or silently incomplete data.
#[test]
fn refuses_unknown_fields_missing_sections_invalid_policies_and_quantities() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    for (path, bad) in [
        ("/schema_version", serde_json::json!(2)),
        ("/routes/region_binding_id", serde_json::json!("")),
        (
            "/vessels/hero_resolution",
            serde_json::json!("unsupported/v1"),
        ),
        (
            "/advancement/draft_opener_policy",
            serde_json::json!("unsupported/v1"),
        ),
        (
            "/watches/quest_xp/recipient_policy",
            serde_json::json!("unsupported/v1"),
        ),
        (
            "/watches/threshold/hero_policy",
            serde_json::json!("unsupported/v1"),
        ),
        ("/watches/initiative/mode", serde_json::json!("unsupported")),
        ("/watches/quest_loot_enabled", serde_json::json!(true)),
        ("/creation/kits/0/items/0/count", serde_json::json!(0)),
        (
            "/creation/kits/0/items/0/count",
            serde_json::json!(4_294_967_296_u64),
        ),
        (
            "/creation/kits/0/items/0/parent_index",
            serde_json::json!(0),
        ),
        ("/creation/classes", serde_json::json!([])),
        ("/companions/rogue/resume_level", serde_json::json!(0)),
        ("/companions/offer/reopen_terms", serde_json::json!([".*"])),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "{path}"
        );
    }
    for path in [
        "",
        "/creation",
        "/creation/classes/0/catalog",
        "/watches",
        "/companions/offer",
    ] {
        let mut changed = original.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), serde_json::json!(true));
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "{path}"
        );
    }
    for section in [
        "routes",
        "vessels",
        "persona",
        "creation",
        "advancement",
        "dialog",
        "companions",
        "watches",
        "srd",
    ] {
        let mut changed = original.clone();
        changed.as_object_mut().unwrap().remove(section);
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "{section}"
        );
    }
}

// K2: the byte source is checked before map parsing, and Job kit recipes cannot
// enter the runtime kit vocabulary through an unknown field.
#[test]
fn refuses_duplicate_json_and_job_recipe_fields_in_runtime_kits() {
    let raw = include_str!("fixtures/dm-duty-source-v1.generated.json");
    let duplicate = raw.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert!(parse_source_shape_json(&duplicate).is_err());
    let duplicate = raw.replacen("\"count\":1", "\"count\":0,\"count\":1", 1);
    assert_ne!(duplicate, raw);
    assert!(parse_source_shape_json(&duplicate).is_err());
    let mut source = unique_json(raw).unwrap();
    let mut kit = source["creation"]["kits"][0].clone();
    assert!(
        serde_json::from_value::<spellkave_validate::dm_duties::RuntimeKit>(kit.clone()).is_err()
    );
    kit.as_object_mut().unwrap().remove("items");
    assert!(serde_json::from_value::<spellkave_validate::dm_duties::RuntimeKit>(kit).is_ok());
    source["creation"]["kits"][0]["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("parent_index");
    assert!(parse_source_shape_json(&source.to_string()).is_err());
}

// K2: finite menu vocabularies and nullable training bridges cannot be widened
// or made ambiguous by duplicate keys, holes, or inconsistent availability.
#[test]
fn refuses_duplicate_or_gapped_domains_and_inconsistent_training_action_presence() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    for path in [
        "/creation/wizard/phase_actions",
        "/dialog/ask_menu_actions",
        "/dialog/open_actions",
        "/dialog/prompt_actions",
        "/dialog/choice_actions",
        "/creation/observation_vocabulary/ability_states",
        "/creation/classes/0/advancement_tags",
        "/vessels/party_companion_keys",
        "/companions/level_sync_keys",
        "/watches/initiative/hostile_actor_keys",
        "/watches/quest_xp/quest_refs",
    ] {
        let mut changed = original.clone();
        let values = changed.pointer_mut(path).unwrap().as_array_mut().unwrap();
        values.push(values[0].clone());
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "{path}"
        );
    }
    let mut retired = original.clone();
    retired["creation"]["draft"] = serde_json::json!({"class_index_state_id":"missing"});
    assert!(parse_source_shape_json(&retired.to_string()).is_err());
    let mut legacy_action = original.clone();
    legacy_action["creation"]["classes"][0]["pick_action_id"] = serde_json::json!("missing");
    assert!(parse_source_shape_json(&legacy_action.to_string()).is_err());
    for path in [
        "/creation/wizard/phase_actions",
        "/dialog/ask_menu_actions",
        "/dialog/open_actions",
        "/dialog/prompt_actions",
        "/dialog/choice_actions",
    ] {
        let mut changed = original.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .remove(1);
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "{path}"
        );
    }
    for (path, bad) in [
        (
            "/creation/training/skill/action_id",
            serde_json::Value::Null,
        ),
        (
            "/creation/training/cantrip/action_id",
            serde_json::json!("example/invented"),
        ),
        (
            "/creation/training/skill/members/0/availability",
            serde_json::json!("unknown"),
        ),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "{path}"
        );
    }
    let index = original["creation"]["classes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|class| {
            !class["catalog"]["choiceGroups"]
                .as_array()
                .unwrap()
                .is_empty()
        })
        .unwrap();
    let mut changed = original.clone();
    changed["creation"]["classes"][index]["catalog"]["choiceGroups"][0]["rechoose"] =
        serde_json::json!("unknown");
    assert!(parse_source_shape_json(&changed.to_string()).is_err());
    let mut changed = original.clone();
    let groups = changed["creation"]["classes"][index]["catalog"]["choiceGroups"]
        .as_array_mut()
        .unwrap();
    groups.push(groups[0].clone());
    assert!(parse_source_shape_json(&changed.to_string()).is_err());
}

// K2: score aliases are literal input vocabulary, never optional or ambiguous
// across the case-insensitive canonical ability domain.
#[test]
fn ability_aliases_are_required_unambiguous_and_include_the_canonical_key() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let parsed = parse_source_shape_json(&original.to_string()).unwrap();
    let states = &parsed.creation.observation_vocabulary.ability_states;
    assert!(states
        .iter()
        .all(|state| state.aliases.contains(&state.ability)));
    let first = &states[0];
    for bad in [
        serde_json::json!([]),
        serde_json::json!([first.ability, ""]),
        serde_json::json!([first.ability, " "]),
        serde_json::json!([first.ability, " leading"]),
        serde_json::json!([first.ability, "trailing "]),
        serde_json::json!(["anothername"]),
        serde_json::json!([first.ability, "ſtrength"]),
        serde_json::json!([first.ability, "str|dex"]),
    ] {
        let mut changed = original.clone();
        changed["creation"]["observation_vocabulary"]["ability_states"][0]["aliases"] = bad;
        assert!(parse_source_shape_json(&changed.to_string()).is_err());
    }
    for alias in [first.aliases[0].clone(), first.aliases[0].to_uppercase()] {
        let mut changed = original.clone();
        changed["creation"]["observation_vocabulary"]["ability_states"][0]["aliases"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!(alias));
        assert!(parse_source_shape_json(&changed.to_string()).is_err());
    }
    let mut cross_ability = original.clone();
    cross_ability["creation"]["observation_vocabulary"]["ability_states"][1]["aliases"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(first.aliases[0].to_uppercase()));
    assert!(parse_source_shape_json(&cross_ability.to_string()).is_err());
    for field in ["aliases", "unknown_aliases"] {
        let mut changed = original.clone();
        let state = changed["creation"]["observation_vocabulary"]["ability_states"][0]
            .as_object_mut()
            .unwrap();
        if field == "aliases" {
            state.remove(field);
        } else {
            state.insert(field.into(), serde_json::json!(["ignored"]));
        }
        assert!(parse_source_shape_json(&changed.to_string()).is_err());
    }
}

// K2: the canonical-key requirement uses the same ASCII case folding as
// collision checks; an exact-match mutation must reject this positive control.
#[test]
fn ability_alias_canonical_membership_ignores_ascii_case() {
    let mut source =
        unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let row = &mut source["creation"]["observation_vocabulary"]["ability_states"][0];
    let canonical = row["ability"].as_str().unwrap().to_owned();
    let uppercase = canonical.to_ascii_uppercase();
    assert_ne!(uppercase, canonical);
    row["aliases"] = serde_json::json!([uppercase]);
    assert!(parse_source_shape_json(&source.to_string()).is_ok());
}

// K2: canonical names and aliases are authored data, without a Rust SRD list.
#[test]
fn ability_aliases_follow_an_alternate_authored_domain() {
    let mut source =
        unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let old = source["creation"]["observation_vocabulary"]["ability_states"][0]["ability"]
        .as_str()
        .unwrap()
        .to_owned();
    source["creation"]["observation_vocabulary"]["ability_states"][0]["ability"] =
        serde_json::json!("resolve");
    source["creation"]["observation_vocabulary"]["ability_states"][0]["aliases"] =
        serde_json::json!(["resolve", "Determination"]);
    for key in source["creation"]["observation_vocabulary"]["menu_tokens"]["score_ability_order"]
        .as_array_mut()
        .unwrap()
    {
        if *key == old {
            *key = serde_json::json!("resolve");
        }
    }
    for class in source["srd"]["classes"].as_array_mut().unwrap() {
        let values = class["standardArray"].as_object_mut().unwrap();
        let value = values.remove(&old).unwrap();
        values.insert("resolve".into(), value);
        if class["unarmoredDefenseExtraAbility"] == old {
            class["unarmoredDefenseExtraAbility"] = serde_json::json!("resolve");
        }
    }
    for profile in source["srd"]["caster_profiles"].as_array_mut().unwrap() {
        if profile["ability"] == old {
            profile["ability"] = serde_json::json!("resolve");
        }
    }
    for payload in source["srd"]["caster_payloads"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        if payload["ability"] == old {
            payload["ability"] = serde_json::json!("resolve");
        }
    }
    let parsed = parse_source_shape_json(&source.to_string()).unwrap();
    assert_eq!(
        parsed.creation.observation_vocabulary.ability_states[0].aliases,
        ["resolve", "Determination"]
    );
}

// K2: observed token grammar is complete, unambiguous literal data; malformed
// transport cannot silently select a different meaning or executable route.
#[test]
fn creation_menu_grammar_refuses_missing_ambiguous_or_inconsistent_transport() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let base = "/creation/observation_vocabulary/menu_tokens";
    for (field, bad, expected) in [
        (
            "namespace_prefix",
            serde_json::json!(""),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "namespace_prefix",
            serde_json::json!("ambiguous.*/"),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "class_prefix",
            serde_json::json!(""),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "skill_prefix",
            serde_json::json!("class/"),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "skill_prefix",
            serde_json::json!("class/sub/"),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "skill_prefix",
            serde_json::json!("unique"),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "skill_prefix",
            serde_json::json!("a*b/"),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "skill_prefix",
            serde_json::json!("/absolute/"),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "array_adopt_token",
            serde_json::json!("unrelated/token"),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "array_adopt_token",
            original["creation"]["wizard"]["discard_action_id"].clone(),
            "duplicate or blank creation input token",
        ),
        (
            "array_adopt_token",
            serde_json::json!(format!(
                "{}new\n",
                original["creation"]["observation_vocabulary"]["menu_tokens"]["namespace_prefix"]
                    .as_str()
                    .unwrap()
            )),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "array_adopt_token",
            original["creation"]["classes"][0]["pick_token"].clone(),
            "missing, malformed or ambiguous creation menu token prefix",
        ),
        (
            "codecs/choice",
            serde_json::json!("base64url-utf8-fatal/v1"),
            "unsupported creation menu transport codec",
        ),
        (
            "codecs/slug",
            serde_json::json!("unicode-slug/v1"),
            "unsupported creation menu transport codec",
        ),
        (
            "codecs/kit",
            serde_json::json!("ascii-lowercase-one/v1"),
            "unsupported creation menu transport codec",
        ),
        (
            "codecs/adopted_text",
            serde_json::json!("uppercase-hex/v1"),
            "unsupported creation menu transport codec",
        ),
        (
            "codecs/reference",
            serde_json::json!("arbitrary-regex"),
            "unsupported creation menu transport codec",
        ),
        (
            "score_digits",
            serde_json::json!(0),
            "incomplete or invalid creation menu score transport",
        ),
        (
            "score_digits",
            serde_json::json!(1),
            "incomplete or invalid creation menu score transport",
        ),
        (
            "score_digits",
            serde_json::json!(21),
            "incomplete or invalid creation menu score transport",
        ),
        (
            "score_ability_order",
            serde_json::json!([]),
            "incomplete or invalid creation menu score transport",
        ),
        (
            "score_ability_order",
            serde_json::json!(["unknown"]),
            "incomplete or invalid creation menu score transport",
        ),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(&format!("{base}/{field}")).unwrap() = bad.clone();
        if field == "namespace_prefix" {
            // Keep array membership coherent so it cannot mask the namespace rule.
            changed.pointer_mut(base).unwrap()["array_adopt_token"] =
                serde_json::json!(format!("{}new", bad.as_str().unwrap()));
        }
        assert!(
            parse_source_shape_json(&changed.to_string())
                .unwrap_err()
                .iter()
                .any(|error| error.contains(expected)),
            "{field}: expected {expected}"
        );
    }
    let mut changed = original.clone();
    let order = changed
        .pointer_mut(&format!("{base}/score_ability_order"))
        .unwrap()
        .as_array_mut()
        .unwrap();
    order[1] = order[0].clone();
    assert!(parse_source_shape_json(&changed.to_string()).is_err());
    for field in ["menu_tokens", "ability_states"] {
        let mut changed = original.clone();
        changed["creation"]["observation_vocabulary"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "missing {field}"
        );
    }
    for field in [
        "class_prefix",
        "codecs",
        "score_digits",
        "score_ability_order",
    ] {
        let mut changed = original.clone();
        changed
            .pointer_mut(base)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "missing {field}"
        );
    }
    let mut changed = original.clone();
    changed.pointer_mut(base).unwrap()["executable"] = serde_json::json!(true);
    assert!(parse_source_shape_json(&changed.to_string()).is_err());
    let mut changed = original.clone();
    changed["creation"]["classes"][0]["slug"] = serde_json::json!("Uppercase");
    let namespace = changed["creation"]["observation_vocabulary"]["menu_tokens"]
        ["namespace_prefix"]
        .as_str()
        .unwrap();
    let prefix = changed["creation"]["observation_vocabulary"]["menu_tokens"]["class_prefix"]
        .as_str()
        .unwrap();
    changed["creation"]["classes"][0]["pick_token"] =
        serde_json::json!(format!("{namespace}{prefix}Uppercase"));
    assert!(parse_source_shape_json(&changed.to_string())
        .unwrap_err()
        .iter()
        .any(|error| error.contains("creation pick token differs from declared menu grammar")));
    let mut changed = original;
    changed["creation"]["classes"][0]["pick_token"] = serde_json::json!("different/pick");
    assert!(parse_source_shape_json(&changed.to_string()).is_err());
}

// K2: prefixes and score order are authored data, not a hidden Rust namespace
// or canonical ability-order inventory. Alternate coherent data stays usable.
#[test]
fn creation_menu_grammar_accepts_alternate_literal_domains_and_score_order() {
    let mut changed =
        unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let menu = &mut changed["creation"]["observation_vocabulary"]["menu_tokens"];
    menu["namespace_prefix"] = serde_json::json!("example.alternate/input/");
    menu["array_adopt_token"] = serde_json::json!("example.alternate/input/recommended");
    menu["score_ability_order"]
        .as_array_mut()
        .unwrap()
        .reverse();
    for (section, prefix_field) in [
        ("classes", "class_prefix"),
        ("species", "species_prefix"),
        ("backgrounds", "background_prefix"),
    ] {
        let prefix = changed["creation"]["observation_vocabulary"]["menu_tokens"][prefix_field]
            .as_str()
            .unwrap()
            .to_owned();
        for row in changed["creation"][section].as_array_mut().unwrap() {
            row["pick_token"] = serde_json::json!(format!(
                "example.alternate/input/{prefix}{}",
                row["slug"].as_str().unwrap()
            ));
        }
    }
    parse_source_shape_json(&changed.to_string()).unwrap();
}

// keep(K2): initiative selectors are required authored vocabulary, not Rust ruleset literals.
#[test]
fn initiative_selectors_require_nonempty_distinct_bounded_references() {
    let mut source =
        unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    source["watches"]["initiative"]["trigger_capability_ids"] =
        serde_json::json!(["example/strike", "example/miss"]);
    source["watches"]["initiative"]["party_tag_id"] = serde_json::json!("example.allies");
    source["watches"]["initiative"]["hostile_tag_id"] = serde_json::json!("example.opponents");
    assert!(parse_source_shape_json(&source.to_string()).is_ok());
    for field in ["trigger_capability_ids", "party_tag_id", "hostile_tag_id"] {
        let mut changed = source.clone();
        changed["watches"]["initiative"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "missing {field}"
        );
    }
    for bad in [
        serde_json::json!([]),
        serde_json::json!([""]),
        serde_json::json!([" "]),
        serde_json::json!(["example/strike", "example/strike"]),
        serde_json::json!(["x".repeat(spellkave_validate::dm_duties::MAX_REF_BYTES + 1)]),
        serde_json::json!([1]),
        serde_json::json!(null),
        serde_json::json!("example/strike"),
    ] {
        let mut changed = source.clone();
        changed["watches"]["initiative"]["trigger_capability_ids"] = bad.clone();
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "trigger {bad}"
        );
    }
    for field in ["party_tag_id", "hostile_tag_id"] {
        for bad in [
            serde_json::json!(""),
            serde_json::json!(" "),
            serde_json::json!("x".repeat(spellkave_validate::dm_duties::MAX_REF_BYTES + 1)),
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!(0),
        ] {
            let mut changed = source.clone();
            changed["watches"]["initiative"][field] = bad.clone();
            let errors = parse_source_shape_json(&changed.to_string()).unwrap_err();
            if bad.is_string() {
                let expected = format!("dm-duties: duties.watches.initiative.{field}: expected bounded nonblank reference");
                assert!(errors.contains(&expected), "{field}: {errors:?}");
            } else {
                assert!(
                    errors
                        .iter()
                        .any(|error| error.contains("invalid source shape")
                            && error.contains("expected a string")),
                    "{field}: {errors:?}"
                );
            }
        }
    }
    source["watches"]["initiative"]["hostile_tag_id"] =
        source["watches"]["initiative"]["party_tag_id"].clone();
    let errors = parse_source_shape_json(&source.to_string()).unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .contains("initiative party and hostile tags must be distinct bounded references")),
        "{errors:?}"
    );
}

// K2: presentation is admitted text, while placeholders and aliases are a
// finite parsing contract. Each mutation must hit its specific diagnostic.
#[test]
fn creation_presentation_refuses_incomplete_ambiguous_or_malformed_data() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let base = "/creation/presentation_vocabulary";
    let assert_error = |path: &str, value: serde_json::Value, expected: &str| {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert_eq!(
            parse_source_shape_json(&changed.to_string()).unwrap_err(),
            vec![format!("dm-duties: {expected}")],
            "{path}"
        );
    };
    for group in ["menu_labels", "missing_phrases", "list"] {
        for key in original
            .pointer(&format!("{base}/{group}"))
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
        {
            for bad in ["".to_string(), "x".repeat(16 * 1024 + 1)]
                .into_iter()
                .chain((group != "list" || key == "empty").then(|| " ".to_string()))
            {
                assert_error(
                    &format!("{base}/{group}/{key}"),
                    serde_json::json!(bad),
                    &if group == "list" {
                        "empty or oversized creation list vocabulary".to_string()
                    } else {
                        format!("blank or oversized creation {group}")
                    },
                );
            }
        }
    }
    for (key, value) in original
        .pointer(&format!("{base}/templates"))
        .unwrap()
        .as_object()
        .unwrap()
    {
        let text = value.as_str().unwrap();
        let mut bad = vec![
            "".to_string(),
            format!("{text}{{unknown}}"),
            format!("{text}{{"),
            format!("{text}}}"),
            "x".repeat(16 * 1024 + 1),
        ];
        if let Some(start) = text.find('{') {
            let end = start + text[start..].find('}').unwrap() + 1;
            bad.push(text.replacen(&text[start..end], "", 1));
            bad.push(format!("{text}{}", &text[start..end]));
        }
        for value in bad {
            assert_error(
                &format!("{base}/templates/{key}"),
                serde_json::json!(value),
                "invalid creation presentation template placeholders",
            );
        }
    }
    for field in ["spoken_confirm_aliases", "spoken_discard_aliases"] {
        for bad in [
            serde_json::json!([]),
            serde_json::json!([""]),
            serde_json::json!(["ok", "ok"]),
            serde_json::json!(["Upper"]),
            serde_json::json!(["--"]),
            serde_json::json!(["''"]),
            serde_json::json!([original["creation"]["classes"][0]["pick_token"]]),
            serde_json::json!([" ok"]),
            serde_json::json!(["ok!"]),
            serde_json::json!([".*"]),
            serde_json::json!(["x".repeat(257)]),
            serde_json::json!(vec!["ok"; 65]),
        ] {
            assert_error(
                &format!("{base}/{field}"),
                bad,
                "missing, malformed or ambiguous creation spoken aliases",
            );
        }
    }
    assert_error(
        &format!("{base}/spoken_confirm_aliases"),
        original
            .pointer(&format!("{base}/spoken_discard_aliases"))
            .unwrap()
            .clone(),
        "missing, malformed or ambiguous creation spoken aliases",
    );
    for field in ["name", "appearance", "backstory"] {
        for bad in [
            serde_json::json!([]),
            serde_json::json!([""]),
            serde_json::json!(["word", "word"]),
            serde_json::json!(["word", "WORD"]),
            serde_json::json!([".*"]),
            serde_json::json!(["two words"]),
            serde_json::json!(["x".repeat(65)]),
            serde_json::json!(vec!["word"; 65]),
            serde_json::json!([
                original["creation"]["observation_vocabulary"]["ability_states"][0]["aliases"][0]
            ]),
        ] {
            assert_error(
                &format!("{base}/text_field_aliases/{field}"),
                bad,
                "missing, malformed or ambiguous creation text field aliases",
            );
        }
    }
    assert_error(
        &format!("{base}/text_field_aliases/name"),
        original
            .pointer(&format!("{base}/text_field_aliases/appearance"))
            .unwrap()
            .clone(),
        "missing, malformed or ambiguous creation text field aliases",
    );
    for path in [
        base.to_string(),
        format!("{base}/menu_labels/class"),
        format!("{base}/templates/adopt_name"),
        format!("{base}/text_field_aliases/name"),
    ] {
        let mut changed = original.clone();
        let (parent, key) = path.rsplit_once('/').unwrap();
        changed
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        let errors = parse_source_shape_json(&changed.to_string()).unwrap_err();
        assert!(
            errors[0].starts_with("dm-duties: invalid source shape: missing field"),
            "{path}: {errors:?}"
        );
    }
    assert_error(
        &format!("{base}/text_field_aliases/name"),
        serde_json::json!(["APPEARANCE"]),
        "missing, malformed or ambiguous creation text field aliases",
    );
    let mut changed = original;
    changed.pointer_mut(base).unwrap()["unexpected"] = serde_json::json!(true);
    assert!(
        parse_source_shape_json(&changed.to_string()).unwrap_err()[0]
            .contains("unknown field `unexpected`")
    );
}

// K2: English wording is not a validator policy; alternate presentation and
// reordered template arguments remain admitted without dispatch authority.
#[test]
fn creation_presentation_admits_alternate_words_and_argument_order() {
    let mut changed =
        unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let presentation = &mut changed["creation"]["presentation_vocabulary"];
    for group in ["menu_labels", "missing_phrases", "list", "templates"] {
        for value in presentation[group].as_object_mut().unwrap().values_mut() {
            *value = serde_json::json!(format!("Alternate {}", value.as_str().unwrap()));
        }
    }
    presentation["list"]["separator"] = serde_json::json!(" ");
    presentation["templates"]["equipment_page"] =
        serde_json::json!("{pages}/{page}: {source_text}");
    presentation["spoken_confirm_aliases"] = serde_json::json!(["accept draft", "approve"]);
    presentation["spoken_discard_aliases"] = serde_json::json!(["cancel draft"]);
    presentation["text_field_aliases"] = serde_json::json!({"name":["DESIGNATION","title"], "appearance":["look"], "backstory":["history"]});
    parse_source_shape_json(&changed.to_string()).unwrap();
}

// K2: missing capability definitions are source observations, with empty
// lists permitted. Their presence is required at every ladder row.
#[test]
fn advancement_missing_definitions_are_required_distinct_source_data() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let path = "/creation/classes/0/advancement_tags/0/missing_capability_definitions";
    for bad in [
        serde_json::json!([""]),
        serde_json::json!(["missing", "missing"]),
        serde_json::json!(["x".repeat(16 * 1024 + 1)]),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = bad;
        assert_eq!(parse_source_shape_json(&changed.to_string()).unwrap_err(), vec!["dm-duties: blank, oversized or duplicate missing advancement capability definition".to_string()]);
    }
    // A later class/level and later member must receive the same checks.
    let mut changed = original.clone();
    let rows = changed["creation"]["classes"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["advancement_tags"]
        .as_array_mut()
        .unwrap();
    rows.last_mut().unwrap()["missing_capability_definitions"] =
        serde_json::json!(["valid-source-gap", ""]);
    assert_eq!(
        parse_source_shape_json(&changed.to_string()).unwrap_err(),
        vec![
            "dm-duties: blank, oversized or duplicate missing advancement capability definition"
                .to_string()
        ]
    );
    for value in [
        serde_json::json!([]),
        serde_json::json!(["alternate/not-defined"]),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = value;
        parse_source_shape_json(&changed.to_string()).unwrap();
    }
    let mut changed = original;
    changed["creation"]["classes"][0]["advancement_tags"][0]
        .as_object_mut()
        .unwrap()
        .remove("missing_capability_definitions");
    assert!(
        parse_source_shape_json(&changed.to_string()).unwrap_err()[0]
            .contains("missing field `missing_capability_definitions`")
    );
}

// K2: completion is admitted numeric data, not a truthiness or magic-one rule.
#[test]
fn kit_granted_value_requires_a_positive_javascript_safe_integer() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    for value in [
        serde_json::json!(0),
        serde_json::json!(9_007_199_254_740_992_u64),
    ] {
        let mut changed = original.clone();
        changed["creation"]["kit_granted_value"] = value;
        assert_eq!(
            parse_source_shape_json(&changed.to_string()).unwrap_err(),
            vec!["dm-duties: invalid creation kit granted value".to_string()]
        );
    }
    for value in [
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!("1"),
    ] {
        let mut changed = original.clone();
        changed["creation"]["kit_granted_value"] = value;
        assert!(
            parse_source_shape_json(&changed.to_string()).unwrap_err()[0]
                .starts_with("dm-duties: invalid source shape:")
        );
    }
    let mut changed = original.clone();
    changed["creation"]["kit_granted_value"] = serde_json::json!(2);
    parse_source_shape_json(&changed.to_string()).unwrap();
    let mut missing = original;
    missing["creation"]
        .as_object_mut()
        .unwrap()
        .remove("kit_granted_value");
    assert!(
        parse_source_shape_json(&missing.to_string()).unwrap_err()[0]
            .contains("missing field `kit_granted_value`")
    );
}

// K2: form wire keys have their own required domain, independent of aliases.
#[test]
fn typed_input_keys_are_required_distinct_and_independent_of_spoken_aliases() {
    let source = include_str!("fixtures/dm-duty-source-v1.generated.json");
    let original: serde_json::Value = serde_json::from_str(source).unwrap();
    for field in ["name", "appearance", "backstory"] {
        for bad in ["".to_string(), "two words".into(), "x".repeat(65)] {
            let mut changed = original.clone();
            changed["creation"]["input_field_keys"][field] = serde_json::json!(bad);
            assert_eq!(
                parse_source_shape_json(&changed.to_string()).unwrap_err(),
                vec!["dm-duties: invalid creation input field keys"]
            );
        }
        let mut changed = original.clone();
        changed["creation"]["input_field_keys"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            parse_source_shape_json(&changed.to_string()).unwrap_err(),
            vec![format!(
                "dm-duties: invalid source shape: missing field `{field}`"
            )]
        );
    }
    let mut changed = original.clone();
    changed["creation"]["input_field_keys"]["name"] =
        original["creation"]["input_field_keys"]["appearance"].clone();
    assert_eq!(
        parse_source_shape_json(&changed.to_string()).unwrap_err(),
        vec!["dm-duties: invalid creation input field keys"]
    );
    changed["creation"]["input_field_keys"]["name"] = serde_json::json!("typed_name");
    changed["creation"]["presentation_vocabulary"]["text_field_aliases"]["name"] =
        serde_json::json!(["DESIGNATION"]);
    parse_source_shape_json(&changed.to_string()).unwrap();
}

// K2: order and links are authored vocabulary, never Rust story IDs or actions.
#[test]
fn layer_topology_is_required_bounded_and_relational_without_closing_the_vocabulary() {
    use serde_json::json;
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    let ids_error = "dm-duties: layer ids must contain exactly two distinct bounded references";
    let role_error = "dm-duties: layer roles must be distinct declared ids";
    let yield_error =
        "dm-duties: companion yield ids must be distinct declared quest-board references";
    let topology = &original["layer_topology"];
    let companion = topology["companion_offer_id"].as_str().unwrap();
    let quest = topology["quest_board_id"].as_str().unwrap();
    let check = |field: &str, bad: serde_json::Value, expected: Vec<&str>| {
        let mut changed = original.clone();
        changed["layer_topology"][field] = bad;
        assert_eq!(
            parse_source_shape_json(&changed.to_string()).unwrap_err(),
            expected,
            "{field}"
        );
    };
    for bad in [
        json!([companion, quest, quest]),
        json!([companion, quest, " "]),
        json!([companion, quest, "x".repeat(16 * 1024 + 1)]),
    ] {
        check("ordered_ids", bad, vec![ids_error]);
    }
    let mut many = vec![companion.to_string(), quest.to_string()];
    many.extend((0..63).map(|i| format!("other/{i}")));
    check("ordered_ids", json!(many), vec![ids_error]);
    check(
        "ordered_ids",
        json!([]),
        vec![ids_error, role_error, yield_error],
    );
    check(
        "companion_offer_id",
        json!(quest),
        vec![role_error, yield_error],
    );
    check(
        "quest_board_id",
        json!(companion),
        vec![role_error, yield_error],
    );
    for field in ["companion_offer_id", "quest_board_id"] {
        check(
            field,
            json!("undeclared"),
            if field == "quest_board_id" {
                vec![role_error, yield_error]
            } else {
                vec![role_error]
            },
        );
    }
    for bad in [
        json!([quest, quest]),
        json!([companion]),
        json!(["undeclared"]),
        json!([""]),
        json!(["x".repeat(16 * 1024 + 1)]),
    ] {
        check("companion_yields_to", bad, vec![yield_error]);
    }
    for field in [
        "ordered_ids",
        "companion_offer_id",
        "quest_board_id",
        "companion_yields_to",
    ] {
        let mut changed = original.clone();
        changed["layer_topology"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            parse_source_shape_json(&changed.to_string()).unwrap_err(),
            vec![format!(
                "dm-duties: invalid source shape: missing field `{field}`"
            )]
        );
    }
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("layer_topology");
    assert_eq!(
        parse_source_shape_json(&missing.to_string()).unwrap_err(),
        vec!["dm-duties: invalid source shape: missing field `layer_topology`"]
    );
    for key in ["action_id", "binding_id", "rule", "unknown"] {
        let mut changed = original.clone();
        changed["layer_topology"][key] = json!("injected");
        let errors = parse_source_shape_json(&changed.to_string()).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with(&format!(
            "dm-duties: invalid source shape: unknown field `{key}`"
        )));
    }
    let mut alternate = original;
    alternate["layer_topology"] = json!({
        "ordered_ids": ["second/board", "other/observer", "first/invitation"],
        "companion_offer_id": "first/invitation", "quest_board_id": "second/board",
        "companion_yields_to": ["other/observer", "second/board"],
    });
    assert_eq!(
        parse_source_shape_json(&alternate.to_string()).unwrap_err(),
        vec![ids_error, yield_error]
    );
    alternate["layer_topology"]["ordered_ids"] = json!(["second/board", "first/invitation"]);
    alternate["layer_topology"]["companion_yields_to"] = json!(["second/board"]);
    let parsed = parse_source_shape_json(&alternate.to_string()).unwrap();
    assert_eq!(
        parsed.layer_topology.ordered_ids,
        vec!["second/board", "first/invitation"]
    );
    assert_eq!(
        parsed.layer_topology.companion_yields_to,
        vec!["second/board"]
    );
    alternate["layer_topology"]["companion_yields_to"] = json!([]);
    parse_source_shape_json(&alternate.to_string()).unwrap();
    let wide = "é".repeat(16 * 1024 / 2);
    alternate["layer_topology"] = json!({
        "ordered_ids": [wide, "board"], "companion_offer_id": wide,
        "quest_board_id": "board", "companion_yields_to": ["board"],
    });
    parse_source_shape_json(&alternate.to_string()).unwrap();
}

// K2: a query prefix is explicit data and must close over every class identity.
#[test]
fn class_identity_query_prefix_is_required_bounded_and_relational() {
    let original = unique_json(FULL_CLASS_SOURCE).unwrap();
    let path = "/creation/observation_vocabulary/class_tag_prefix";
    for bad in ["", " ", "other.", &"x".repeat(16 * 1024 + 1)] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = serde_json::json!(bad);
        assert_eq!(
            parse_source_shape_json(&changed.to_string()).unwrap_err(),
            vec![
                "dm-duties: class identity query prefix differs from admitted class tags"
                    .to_string()
            ]
        );
    }
    let mut missing = original.clone();
    missing["creation"]["observation_vocabulary"]
        .as_object_mut()
        .unwrap()
        .remove("class_tag_prefix");
    assert_eq!(
        parse_source_shape_json(&missing.to_string()).unwrap_err(),
        vec!["dm-duties: invalid source shape: missing field `class_tag_prefix`"]
    );
    let mut blank = original.clone();
    blank["creation"]["observation_vocabulary"]["class_tag_prefix"] = serde_json::json!("");
    for class in blank["creation"]["classes"].as_array_mut().unwrap() {
        class["identity_tag"] = class["slug"].clone();
    }
    assert_eq!(
        parse_source_shape_json(&blank.to_string()).unwrap_err(),
        vec!["dm-duties: class identity query prefix differs from admitted class tags"]
    );
    // Keep the prefix relation valid; the slug also violates the existing menu/domain rules.
    let mut dotted = original.clone();
    let class = &mut dotted["creation"]["classes"][0];
    class["slug"] = serde_json::json!("hero.class");
    class["catalog"]["classSlug"] = class["slug"].clone();
    class["identity_tag"] = serde_json::json!(format!(
        "{}hero.class",
        original["creation"]["observation_vocabulary"]["class_tag_prefix"]
            .as_str()
            .unwrap()
    ));
    assert_eq!(
        parse_source_shape_json(&dotted.to_string()).unwrap_err(),
        vec![
            "dm-duties: creation pick token differs from declared menu grammar",
            "dm-duties: creation class is outside admitted SRD data",
            "dm-duties: class identity query prefix differs from admitted class tags",
            "dm-duties: kit class is outside the menu",
            "dm-duties: kit class is outside the menu",
        ]
    );
    let mut changed = original;
    changed["creation"]["observation_vocabulary"]["class_tag_prefix"] =
        serde_json::json!("roles.hero.");
    for class in changed["creation"]["classes"].as_array_mut().unwrap() {
        class["identity_tag"] =
            serde_json::json!(format!("roles.hero.{}", class["slug"].as_str().unwrap()));
    }
    parse_source_shape_json(&changed.to_string()).unwrap();
}

// ADR-0168 D-2/D-4: a DM document has no `hooks` field and no other place for
// script content; its closed schema refuses both.
#[test]
fn dm_source_has_no_carrier_for_hooks_or_script_content() {
    let original = unique_json(include_str!("fixtures/dm-duty-source-v1.generated.json")).unwrap();
    for field in ["hooks", "script", "script_path", "npc_kind"] {
        let mut changed = original.clone();
        changed[field] = serde_json::json!({"decide": "--!strict\nreturn function() end"});
        assert!(
            parse_source_shape_json(&changed.to_string()).is_err(),
            "{field}"
        );
        let mut nested = original.clone();
        nested["creation"][field] = serde_json::json!("scripted");
        assert!(
            parse_source_shape_json(&nested.to_string()).is_err(),
            "creation.{field}"
        );
    }
}

// K2 (sk-jay0.1.55.60.19): the generated fixture is the adventure's real
// menu, a strict subset of the SRD classes with only its own kits. The same
// refusals hold on it as on the full list.
#[test]
fn the_generated_four_class_menu_is_admitted_and_keeps_the_class_refusals() {
    let source = include_str!("fixtures/dm-duty-source-v1.generated.json");
    parse_source_shape_json(source).unwrap();
    let original = unique_json(source).unwrap();
    let slugs = |value: &serde_json::Value| -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|class| class["slug"].as_str().unwrap().to_owned())
            .collect()
    };
    let menu = slugs(&original["creation"]["classes"]);
    let srd = slugs(&original["srd"]["classes"]);
    assert_eq!(menu, ["fighter", "rogue", "cleric", "wizard"]);
    assert_eq!(srd.len(), 12, "the SRD class tables stay complete");
    assert!(menu.iter().all(|slug| srd.contains(slug)));
    let kits = original["creation"]["kits"].as_array().unwrap();
    assert!(kits
        .iter()
        .all(|kit| menu.iter().any(|slug| kit["class_slug"] == slug.as_str())));
    // A dotted slug on the first menu class: the same refusals as on the full
    // list, with one kit refusal per kit of that class.
    let first_class_kits = kits
        .iter()
        .filter(|kit| kit["class_slug"] == menu[0].as_str())
        .count();
    assert_eq!(first_class_kits, 3);
    let mut dotted = original.clone();
    let class = &mut dotted["creation"]["classes"][0];
    class["slug"] = serde_json::json!("hero.class");
    class["catalog"]["classSlug"] = class["slug"].clone();
    class["identity_tag"] = serde_json::json!(format!(
        "{}hero.class",
        original["creation"]["observation_vocabulary"]["class_tag_prefix"]
            .as_str()
            .unwrap()
    ));
    let mut expected = vec![
        "dm-duties: creation pick token differs from declared menu grammar".to_string(),
        "dm-duties: creation class is outside admitted SRD data".to_string(),
        "dm-duties: class identity query prefix differs from admitted class tags".to_string(),
    ];
    expected.extend(std::iter::repeat_n(
        "dm-duties: kit class is outside the menu".to_string(),
        first_class_kits,
    ));
    assert_eq!(
        parse_source_shape_json(&dotted.to_string()).unwrap_err(),
        expected
    );
    // A class outside the SRD data, a duplicate and an emptied menu are refused.
    let mut unknown = original.clone();
    let mut extra = unknown["creation"]["classes"][0].clone();
    extra["slug"] = "not-an-srd-class".into();
    unknown["creation"]["classes"]
        .as_array_mut()
        .unwrap()
        .push(extra);
    assert!(parse_source_shape_json(&unknown.to_string())
        .unwrap_err()
        .contains(&"dm-duties: creation class is outside admitted SRD data".to_string()));
    let mut duplicate = original.clone();
    let again = duplicate["creation"]["classes"][0].clone();
    duplicate["creation"]["classes"]
        .as_array_mut()
        .unwrap()
        .push(again);
    assert!(parse_source_shape_json(&duplicate.to_string()).is_err());
    let mut none = original.clone();
    none["creation"]["classes"] = serde_json::json!([]);
    none["creation"]["kits"] = serde_json::json!([]);
    assert!(parse_source_shape_json(&none.to_string())
        .unwrap_err()
        .contains(&"dm-duties: empty or duplicate classes".to_string()));
}

// K2 (sk-jay0.3.27.4.2): on-sight initiative is opt-in data. Absent keeps the
// document as it is; present, it must name listed hostiles and well-formed,
// distinct conditions, and it belongs to the legacy owner only.
#[test]
fn on_sight_initiative_is_optional_and_refuses_empty_unlisted_or_controller_use() {
    // The frozen twelve-class document predates the trigger and carries none.
    // The generated Elven Mini document switches it on and is admitted as is.
    assert!(
        unique_json(FULL_CLASS_SOURCE).unwrap()["watches"]["initiative"]
            .get("on_sight")
            .is_none()
    );
    let authored =
        parse_source_shape_json(include_str!("fixtures/dm-duty-source-v1.generated.json"))
            .unwrap()
            .watches
            .initiative
            .on_sight
            .expect("the generated Elven Mini source switches on-sight initiative on");
    assert!(!authored.actor_keys.is_empty());
    assert!(!authored.peace_condition_ids.is_empty());
    for source in [
        FULL_CLASS_SOURCE,
        include_str!("fixtures/dm-duty-source-v1.generated.json"),
    ] {
        // Every case below starts from the document without the object.
        let mut original = unique_json(source).unwrap();
        original["watches"]["initiative"]
            .as_object_mut()
            .unwrap()
            .remove("on_sight");
        let hostile = original["watches"]["initiative"]["hostile_actor_keys"][0].clone();
        let with = |on_sight: serde_json::Value| {
            let mut changed = original.clone();
            changed["watches"]["initiative"]["on_sight"] = on_sight;
            changed
        };
        let good = serde_json::json!({
            "actor_keys": [hostile],
            "unseen_condition_ids": ["srd/condition/invisible"],
            "peace_condition_ids": [],
        });
        let admitted = parse_source_shape_json(&with(good.clone()).to_string()).unwrap();
        assert_eq!(
            admitted
                .watches
                .initiative
                .on_sight
                .unwrap()
                .actor_keys
                .len(),
            1
        );
        for (name, change, message) in [
            (
                "no actor",
                serde_json::json!({"actor_keys": []}),
                "dm-duties: on-sight actors must be nonempty, distinct bounded references",
            ),
            (
                "duplicate actor",
                serde_json::json!({"actor_keys": [hostile, hostile]}),
                "dm-duties: on-sight actors must be nonempty, distinct bounded references",
            ),
            (
                "unlisted actor",
                serde_json::json!({"actor_keys": ["example/not-a-listed-hostile"]}),
                "dm-duties: on-sight actor is not a listed hostile",
            ),
            (
                "blank condition",
                serde_json::json!({"unseen_condition_ids": [" "]}),
                "dm-duties: on-sight unseen and peace conditions must be distinct bounded references",
            ),
            (
                "one condition in both lists",
                serde_json::json!({"peace_condition_ids": ["srd/condition/invisible"]}),
                "dm-duties: on-sight unseen and peace conditions must be distinct bounded references",
            ),
        ] {
            let mut on_sight = good.clone();
            for (key, value) in change.as_object().unwrap() {
                on_sight[key] = value.clone();
            }
            assert_eq!(
                parse_source_shape_json(&with(on_sight).to_string()).unwrap_err(),
                vec![message.to_string()],
                "{name}"
            );
        }
        // A missing list or an unknown field is a shape error, not a default.
        for broken in [
            serde_json::json!({"actor_keys": [hostile], "unseen_condition_ids": []}),
            serde_json::json!({"actor_keys": [hostile], "unseen_condition_ids": [], "peace_condition_ids": [], "radius": 3}),
        ] {
            assert!(parse_source_shape_json(&with(broken).to_string()).is_err());
        }
        // The seatless controller owns initiation in its mode.
        let mut controller = with(good.clone());
        controller["watches"]["initiative"]["mode"] = "controller".into();
        assert!(parse_source_shape_json(&controller.to_string())
            .unwrap_err()
            .contains(
                &"dm-duties: on-sight initiative belongs to the legacy owner only".to_string()
            ));
    }
}
