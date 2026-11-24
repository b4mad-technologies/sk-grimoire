//! Core manifest fragments shared by integration tests.

pub const CORE_AFFORDANCES: &str =
    include_str!("../../../packages/spellkave-mechanics/fixtures/core/base/affordances.toml");
pub const CORE_CONDITIONS: &str =
    include_str!("../../../packages/spellkave-mechanics/fixtures/core/base/conditions.toml");
pub const CORE_ROLES: &str =
    include_str!("../../../packages/spellkave-mechanics/fixtures/core/base/roles.toml");
pub const CORE_ROLL_TABLES: &str =
    include_str!("../../../packages/spellkave-mechanics/fixtures/core/base/roll-tables.toml");

/// Core fragments in the order used by the manifest apply snapshot tests.
pub const CORE_BASE_FIXTURES: &[(&str, &str)] = &[
    ("affordances.toml", CORE_AFFORDANCES),
    ("conditions.toml", CORE_CONDITIONS),
    ("roles.toml", CORE_ROLES),
    ("roll-tables.toml", CORE_ROLL_TABLES),
];
