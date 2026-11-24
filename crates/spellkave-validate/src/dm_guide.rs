//! What the DM kind's creation guide may name. The guide's shape is
//! `crate::guide`; these ids are the DM kind's own vocabulary for it.

/// Option sources a creation guide may name. Each names a list of this
/// document's own catalogs; the DM kind resolves them.
pub const SOURCE_CLASSES: &str = "classes";
pub const SOURCE_BACKGROUNDS: &str = "backgrounds";
pub const SOURCE_SPECIES: &str = "species";
pub const SOURCE_CLASS_SKILLS: &str = "class-skills";
pub const SOURCE_CLASS_CHOICE_GROUPS: &str = "class-choice-groups";
pub const SOURCE_CLASS_CANTRIPS: &str = "class-cantrips";
pub const SOURCE_CLASS_PREPARED: &str = "class-prepared";
pub const SOURCE_CLASS_KITS: &str = "class-kits";
pub const SOURCE_KIT_EQUIPMENT: &str = "kit-equipment";
pub const SOURCE_PENDING_NAME: &str = "pending-name";
pub const SOURCE_STANDARD_ARRAY: &str = "standard-array";
pub const SOURCE_CONFIRM: &str = "confirm";
pub const CREATION_SOURCES: [&str; 12] = [
    SOURCE_CLASSES,
    SOURCE_BACKGROUNDS,
    SOURCE_SPECIES,
    SOURCE_CLASS_SKILLS,
    SOURCE_CLASS_CHOICE_GROUPS,
    SOURCE_CLASS_CANTRIPS,
    SOURCE_CLASS_PREPARED,
    SOURCE_CLASS_KITS,
    SOURCE_KIT_EQUIPMENT,
    SOURCE_PENDING_NAME,
    SOURCE_STANDARD_ARRAY,
    SOURCE_CONFIRM,
];
/// Inputs a creation guide may name: answers that arrive as typed text.
pub const INPUT_NAME: &str = "name";
pub const CREATION_INPUTS: [&str; 1] = [INPUT_NAME];
