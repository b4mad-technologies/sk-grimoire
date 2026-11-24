//! Parser strictness: unknown fields in the manifest or in any authored row
//! must surface as a parse error rather than being silently dropped.
//!
//! Why: silent-drop turns modder typos (`nmae = "Goblin"`) into invisible
//! bugs that only manifest at play time. `#[serde(deny_unknown_fields)]` on
//! every schema struct converts the entire class into a parse-time error.

use spellkave_format::{parse_manifest, ParseError};

fn schema_header() -> &'static str {
    "#:schema https://spellkave.example/schema/manifest-v1.json\nschema_version = \"1.0.0\"\n"
}

fn expect_unknown_field_error(source: &str, expected_token: &str) {
    let err = parse_manifest(source)
        .err()
        .unwrap_or_else(|| panic!("expected parse error mentioning `{expected_token}`"));
    let msg = match err {
        ParseError::Toml(e) => e.to_string(),
        ParseError::MissingSchemaDirective => {
            panic!("unexpected MissingSchemaDirective for source:\n{source}")
        }
        ParseError::UnsupportedMetadataValue(value) => {
            panic!("unexpected UnsupportedMetadataValue ({value}) for source:\n{source}")
        }
        ParseError::FragmentApplyset => {
            panic!("unexpected FragmentApplyset for source:\n{source}")
        }
    };
    assert!(
        msg.contains("unknown field") && msg.contains(expected_token),
        "expected `unknown field` mentioning `{expected_token}`, got:\n{msg}"
    );
}

#[test]
fn unknown_top_level_field_is_rejected() {
    let source = format!("{}\njunk_top_level = 1\n", schema_header());
    expect_unknown_field_error(&source, "junk_top_level");
}

#[test]
fn unknown_field_in_bundle_row_is_rejected() {
    let source = format!(
        "{}\n[[bundles]]\nbundle_id = \"core\"\ndisplay_name = \"Core\"\nauthor = \"x\"\nversion = \"1\"\njunk_in_bundle = true\n",
        schema_header()
    );
    expect_unknown_field_error(&source, "junk_in_bundle");
}

#[test]
fn unknown_field_in_entity_row_is_rejected() {
    let source = format!(
        "{}\n[[entities]]\nentity_key = \"goblin\"\nbundle_id = \"core\"\nname = \"Skraz\"\nkind = \"npc\"\nplace_node_id = \"cave\"\nnmae = \"typo\"\n",
        schema_header()
    );
    expect_unknown_field_error(&source, "nmae");
}

#[test]
fn unknown_field_in_action_definition_row_is_rejected() {
    let source = format!(
        "{}\n[[action_definitions]]\naction_id = \"core/attack\"\nbundle_id = \"core\"\nrequired_capability = \"core/attack\"\non_success = \"core/attack/success\"\ndisplay_name = \"Attack\"\njunk_in_action = 7\n",
        schema_header()
    );
    expect_unknown_field_error(&source, "junk_in_action");
}

#[test]
fn retired_items_row_kind_is_rejected_as_unknown_top_level_field() {
    // #1968 B-10: `[[items]]` (and its item_inventory/item_locations/
    // item_states/world_objects/world_object_locations satellites) no
    // longer exist as Manifest fields at all — a manifest carrying one now
    // fails loudly as an unknown TOP-LEVEL field, one step earlier than the
    // old row-level rejection this replaces.
    let source = format!(
        "{}\n[[items]]\nitem_key = \"sword\"\nbundle_id = \"core\"\nitem_def_id = \"sword_def\"\nname = \"Sword\"\ndescription = \"\"\nkind = \"weapon\"\n",
        schema_header()
    );
    expect_unknown_field_error(&source, "items");
}

#[test]
fn unknown_field_in_capability_definition_is_rejected() {
    let source = format!(
        "{}\n[[capability_definitions]]\ncapability_id = \"core/attack\"\nbundle_id = \"core\"\nactor_kind = \"any\"\ndisplay_name = \"Attack\"\ndescription = \"\"\njunk_in_capability = \"oops\"\n",
        schema_header()
    );
    expect_unknown_field_error(&source, "junk_in_capability");
}

#[test]
fn known_fields_still_parse() {
    // Sanity check: a minimal valid manifest with only the schema header
    // should still parse cleanly under deny_unknown_fields.
    let source = schema_header();
    parse_manifest(source).expect("minimal manifest should parse");
}

#[test]
fn include_directives_parse_under_deny_unknown_fields() {
    // The CLI manifest loader extracts `[[include]]` directives and
    // concatenates included files into one source string. That source
    // is then re-parsed as `Manifest`. The `include` field on `Manifest`
    // absorbs the now-redundant directives so `deny_unknown_fields`
    // doesn't reject them. Regression guard for the integration_core_test
    // failure that landed when deny_unknown_fields was first introduced.
    let source = format!(
        "{}\n[[include]]\npath = \"base/spatial.toml\"\n\n[[include]]\npath = \"base/items.toml\"\n",
        schema_header()
    );
    parse_manifest(&source).expect("include directives must parse under deny_unknown_fields");
}
