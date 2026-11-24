use spellkave_schema::refusal::{CONTROL_STABLE_REFUSAL_CODES, WORLD_STABLE_REFUSAL_CODES};

#[test]
fn stable_refusal_codes_are_plain_identifiers() {
    for code in WORLD_STABLE_REFUSAL_CODES
        .iter()
        .chain(CONTROL_STABLE_REFUSAL_CODES)
    {
        assert!(
            !code.is_empty()
                && code
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "stable refusal code {code:?} must be a plain identifier"
        );
    }
}
