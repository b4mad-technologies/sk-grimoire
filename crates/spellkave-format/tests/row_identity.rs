// Row identity and the semantic row hash (ADR-0145 D-2), exercised through the
// real parser: the hash must be a property of the parsed row, not of the text.

use spellkave_format::parse_manifest_fragment;
use spellkave_schema::row_identity::{manifest_row_identities, RowIdentity};

fn rows(source: &str) -> Vec<RowIdentity> {
    manifest_row_identities(&parse_manifest_fragment(source).expect("fragment parses")).rows
}

const ENTITY: &str = r#"
[[entity_definitions]]
bundle_id = "demo"
entity_def_id = "demo/lamp"
display_name = "Lamp"
description = "A brass lamp."
"#;

#[test]
fn comments_order_and_whitespace_do_not_change_the_hash() {
    let shuffled = r#"
# a comment the author added
[[entity_definitions]]
description   = "A brass lamp."
display_name = "Lamp"
entity_def_id = "demo/lamp"   # trailing comment
bundle_id = "demo"
"#;
    assert_eq!(rows(ENTITY)[0].hash, rows(shuffled)[0].hash);
}

#[test]
fn an_omitted_field_hashes_like_its_default() {
    let explicit = format!("{ENTITY}stackable = false\n");
    assert_eq!(rows(ENTITY)[0].hash, rows(&explicit)[0].hash);
}

#[test]
fn a_description_edit_changes_the_hash_but_not_the_identity() {
    let edited = ENTITY.replace("A brass lamp.", "A dented brass lamp.");
    let (before, after) = (&rows(ENTITY)[0], &rows(&edited)[0]);
    assert_eq!((&before.kind, &before.key), (&after.kind, &after.key));
    assert_ne!(before.hash, after.hash);
}

#[test]
fn block_order_does_not_change_row_hashes() {
    let two = format!(
        "{ENTITY}\n[[entity_definitions]]\nbundle_id = \"demo\"\nentity_def_id = \"demo/rope\"\ndisplay_name = \"Rope\"\ndescription = \"Hemp.\"\n"
    );
    let reversed = format!(
        "[[entity_definitions]]\nbundle_id = \"demo\"\nentity_def_id = \"demo/rope\"\ndisplay_name = \"Rope\"\ndescription = \"Hemp.\"\n{ENTITY}"
    );
    let mut a = rows(&two);
    let mut b = rows(&reversed);
    a.sort_by(|x, y| x.key.cmp(&y.key));
    b.sort_by(|x, y| x.key.cmp(&y.key));
    assert_eq!(a, b);
}
