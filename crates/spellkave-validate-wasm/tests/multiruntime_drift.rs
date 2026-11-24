use wasm_bindgen_test::wasm_bindgen_test;

// keep(K3): authors receive identical MF-VAL-391 admission diagnostics through
// the native and Wasm entry points.
// The Wasm cases can also run via `wasm-pack test --node`.
#[wasm_bindgen_test]
fn containment_policy_accept_and_reject_match_shared_validator() {
    for (source, rejected) in [
        (
            include_str!("../testdata/containment_policy_accept.toml"),
            false,
        ),
        (
            include_str!("../testdata/containment_policy_reject.toml"),
            true,
        ),
    ] {
        let manifest = spellkave_format::parse_manifest(source).unwrap().manifest;
        let native_diags = spellkave_validate::validate_manifest(&manifest);
        let expected = spellkave_validate::diagnostics_to_canonical_json(&native_diags).unwrap();
        assert_eq!(
            spellkave_validate_wasm::validate_manifest_toml(source),
            expected
        );
        assert_eq!(
            native_diags.iter().any(|d| d.code == "MF-VAL-391"),
            rejected,
            "{native_diags:?}"
        );
        if !rejected {
            assert!(native_diags.is_empty(), "{native_diags:?}");
        }
    }
}

// keep(K3): the author-target bridge must preserve shared Rust scope refusal.
#[wasm_bindgen_test]
fn containment_context_matches_shared_reference_admission() {
    let source = include_str!("../testdata/containment_policy_accept.toml").replacen(
        "true == true",
        "target.status('parity/ready')",
        1,
    );
    let manifest = spellkave_format::parse_manifest(&source).unwrap().manifest;
    for owner in ["parity", "foreign"] {
        let json = serde_json::json!({
            "conditions": {"parity/ready": owner},
            "bundle_dependencies": {"parity": [], "foreign": []}
        })
        .to_string();
        let context: spellkave_validate::containment_policy::ReferenceContext =
            serde_json::from_str(&json).unwrap();
        let expected = spellkave_validate::validate_manifest_with_containment_context(
            &manifest,
            &context.into_vocabulary(),
        );
        assert_eq!(
            spellkave_validate_wasm::validate_manifest_toml_with_containment_context(
                &source, &json
            ),
            spellkave_validate::diagnostics_to_canonical_json(&expected).unwrap(),
        );
        assert_eq!(
            expected.iter().any(|d| d.code == "MF-VAL-391"),
            owner == "foreign"
        );
    }
}

#[wasm_bindgen_test]
fn browser_wasm_matches_native_for_unsupported_version() {
    let browser_json = spellkave_validate_wasm::validate_schema_version("2.0.0");
    let browser_diags: serde_json::Value =
        serde_json::from_str(&browser_json).expect("browser wasm JSON should parse");

    let native_diags = spellkave_validate::validate_manifest_version("2.0.0");
    let native_json =
        serde_json::to_value(native_diags).expect("native diagnostics should serialize");

    assert_eq!(browser_diags, native_json);
}

#[wasm_bindgen_test]
fn browser_wasm_matches_native_for_reference_bindings() {
    let browser_schema_json = spellkave_validate_wasm::manifest_schema_json();
    let native_schema_json = spellkave_schema::manifest_json_schema_pretty()
        .expect("native schema JSON should serialize");
    assert_eq!(browser_schema_json, native_schema_json);
    assert!(browser_schema_json.contains("9007199254740991.0"));
    let browser_schema: serde_json::Value =
        serde_json::from_str(&browser_schema_json).expect("browser schema JSON should parse");
    assert_eq!(browser_schema["title"], "Manifest");

    let browser_dsl: serde_json::Value =
        serde_json::from_str(&spellkave_validate_wasm::dsl_reference_json())
            .expect("browser DSL JSON should parse");
    let native_dsl =
        serde_json::to_value(spellkave_dsl::dsl_reference()).expect("DSL reference serializes");
    assert_eq!(browser_dsl, native_dsl);

    // #1968 B-9: the frozen observation-placeholder token set must match
    // between runtimes exactly like every other reference export above.
    let browser_placeholders: serde_json::Value =
        serde_json::from_str(&spellkave_validate_wasm::observation_placeholder_tokens_json())
            .expect("browser observation placeholder JSON should parse");
    let native_placeholders =
        serde_json::to_value(spellkave_validate::OBSERVATION_PLACEHOLDER_TOKENS)
            .expect("observation placeholder tokens serialize");
    assert_eq!(browser_placeholders, native_placeholders);
}

#[wasm_bindgen_test]
fn browser_wasm_matches_core_for_empty_reserved_fragment_metadata() {
    let source = "[metadata]\nprofile = \"\"\n";
    let exported = spellkave_validate_wasm::split_manifest_fragment(source)
        .expect("empty reserved string is valid metadata");
    let core = spellkave_validate_wasm::split_manifest_fragment_core(source)
        .expect("native core should accept the same metadata");
    assert_eq!(exported, core);
}

#[wasm_bindgen_test]
fn browser_wasm_matches_native_for_parse_and_render_bindings() {
    let source =
        "#:schema https://spellkave.dev/schema/world-manifest-1.0.0.json\nschema_version = \"1.0.0\"\n";
    let browser_json = spellkave_validate_wasm::parse_manifest_toml_to_json(source)
        .expect("browser parse should succeed");
    let native_manifest = spellkave_format::parse_manifest(source)
        .expect("native parse should succeed")
        .manifest;
    let native_json =
        serde_json::to_string_pretty(&native_manifest).expect("native manifest should serialize");
    assert_eq!(browser_json, native_json);

    let browser_toml = spellkave_validate_wasm::render_manifest_json_to_toml(&browser_json)
        .expect("browser render should succeed");
    let native_toml =
        spellkave_format::render_manifest(&native_manifest).expect("native render should succeed");
    assert_eq!(browser_toml, native_toml);
}

#[wasm_bindgen_test]
fn browser_wasm_matches_native_for_equipment_ref_lints_dangling_item_key() {
    let manifest = spellkave_schema::Manifest {
        schema_version: spellkave_schema::SchemaVersion::new(1, 0, 0),
        item_definitions: vec![spellkave_schema::ItemDefinitionRow {
            item_def_id: "homebrew/item/horn_blade".to_owned(),
            bundle_id: "core".to_owned(),
            slot_id: "homebrew/horn-sheath".to_owned(),
            slot_compatibility_predicate: String::new(),
            description: String::new(),
        }],
        actor_equipment_slots: vec![spellkave_schema::ActorEquipmentSlotRow {
            actor_entity_key: "core/player".to_owned(),
            slot_id: "homebrew/horn-sheath".to_owned(),
            item_key: "homebrew/item/missing".to_owned(),
        }],
        ..spellkave_schema::Manifest::default()
    };
    let manifest_json = serde_json::to_string(&manifest).expect("manifest JSON should serialize");

    let browser_json = spellkave_validate_wasm::validate_manifest_json(&manifest_json);
    let native_diags = spellkave_validate::validate_manifest(&manifest);
    let native_json = spellkave_validate::diagnostics_to_canonical_json(&native_diags)
        .expect("native diagnostics should serialize");

    assert_eq!(browser_json, native_json);
}

#[wasm_bindgen_test]
fn browser_wasm_matches_native_for_equipment_ref_lints_duplicate_actor_slot() {
    let manifest = spellkave_schema::Manifest {
        schema_version: spellkave_schema::SchemaVersion::new(1, 0, 0),
        bundles: vec![spellkave_schema::BundleRow {
            bundle_id: "core".to_owned(),
            display_name: "core".to_owned(),
            author: "test".to_owned(),
            version: "1.0.0".to_owned(),
            depends_on: vec![],
            description: String::new(),
            tag_prefixes: Vec::new(),
            allow_flat_tags: false,
            default_extent_shape_id: String::new(),
            default_extent_params_json: String::new(),
        }],
        entities: vec![
            spellkave_schema::EntityRow {
                entity_key: "core/player".to_owned(),
                bundle_id: "core".to_owned(),
                name: "Player".to_owned(),
                kind: spellkave_schema::EntityKindRow::Player,
                entity_def_id: String::new(),
                place_node_id: String::new(),
                quantity: 1,
                owned_by_entity_key: String::new(),
                description: String::new(),
            },
            // #1968 B-10: items are authored as `[[entities]]` rows with
            // kind="item" (the `[[items]]` row kind was retired).
            spellkave_schema::EntityRow {
                entity_key: "homebrew/item/horn_blade-1".to_owned(),
                bundle_id: "core".to_owned(),
                name: "Horn Blade 1".to_owned(),
                kind: spellkave_schema::EntityKindRow::Item,
                entity_def_id: "homebrew/item/horn_blade".to_owned(),
                place_node_id: String::new(),
                quantity: 1,
                owned_by_entity_key: String::new(),
                description: String::new(),
            },
            spellkave_schema::EntityRow {
                entity_key: "homebrew/item/horn_blade-2".to_owned(),
                bundle_id: "core".to_owned(),
                name: "Horn Blade 2".to_owned(),
                kind: spellkave_schema::EntityKindRow::Item,
                entity_def_id: "homebrew/item/horn_blade".to_owned(),
                place_node_id: String::new(),
                quantity: 1,
                owned_by_entity_key: String::new(),
                description: String::new(),
            },
        ],
        actor_equipment_slots: vec![
            spellkave_schema::ActorEquipmentSlotRow {
                actor_entity_key: "core/player".to_owned(),
                slot_id: "homebrew/horn-sheath".to_owned(),
                item_key: "homebrew/item/horn_blade-1".to_owned(),
            },
            spellkave_schema::ActorEquipmentSlotRow {
                actor_entity_key: "core/player".to_owned(),
                slot_id: "homebrew/horn-sheath".to_owned(),
                item_key: "homebrew/item/horn_blade-2".to_owned(),
            },
        ],
        ..spellkave_schema::Manifest::default()
    };
    let manifest_json = serde_json::to_string(&manifest).expect("manifest JSON should serialize");

    let browser_json = spellkave_validate_wasm::validate_manifest_json(&manifest_json);
    let native_diags = spellkave_validate::validate_manifest(&manifest);
    let native_json = spellkave_validate::diagnostics_to_canonical_json(&native_diags)
        .expect("native diagnostics should serialize");

    assert_eq!(browser_json, native_json);
}
