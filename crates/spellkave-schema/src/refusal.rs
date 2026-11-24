//! Stable refusal codes for world and control operations.

/// Seat cap reached on a runtime seat claim (`world-mutation-semantics`).
pub const SEAT_CAP_REFUSAL_CODE: &str = "SEAT-CAP-001";

// Driver assignment and summon refusals (`r_assign_driver`), mapped by the
// `sk` NPC setup.
pub const DRIVE_REQUIRES_OWNED_PRESENCE: &str = "drive_requires_owned_presence";
pub const DRIVE_REQUIRES_NON_PRESENCE_BODY: &str = "drive_requires_non_presence_body";
pub const DRIVE_TARGET_IS_OFFERED: &str = "drive_target_is_offered";
pub const DRIVE_TARGET_IS_BINDING_SUBJECT: &str = "drive_target_is_binding_subject";
pub const DRIVE_REQUIRES_EXTENT: &str = "drive_requires_extent";
pub const DRIVE_IDENTITY_MUST_BE_SEPARATE: &str = "drive_identity_must_be_separate";
pub const DRIVER_ROW_NOT_ASSIGNED: &str = "driver_row_not_assigned";
pub const DRIVE_IDENTITY_ALREADY_BOUND: &str = "drive_identity_already_bound";
pub const DRIVER_CONTROL_MISSING_STATE: &str = "driver_control_missing_state";
pub const DRIVER_ASSIGN_WHILE_HELD: &str = "driver_assign_while_held";
pub const DRIVER_ASSIGNMENT_NOT_ISOLATED: &str = "driver_assignment_not_isolated";
pub const DRIVER_SUMMON_MISSING_BODY: &str = "driver_summon_missing_body";
pub const DRIVER_SUMMON_WHILE_HELD: &str = "driver_summon_while_held";

/// Every stable code the world module refuses with.
pub const WORLD_STABLE_REFUSAL_CODES: &[&str] = &[
    SEAT_CAP_REFUSAL_CODE,
    DRIVE_REQUIRES_OWNED_PRESENCE,
    DRIVE_REQUIRES_NON_PRESENCE_BODY,
    DRIVE_TARGET_IS_OFFERED,
    DRIVE_TARGET_IS_BINDING_SUBJECT,
    DRIVE_REQUIRES_EXTENT,
    DRIVE_IDENTITY_MUST_BE_SEPARATE,
    DRIVER_ROW_NOT_ASSIGNED,
    DRIVE_IDENTITY_ALREADY_BOUND,
    DRIVER_CONTROL_MISSING_STATE,
    DRIVER_ASSIGN_WHILE_HELD,
    DRIVER_ASSIGNMENT_NOT_ISOLATED,
    DRIVER_SUMMON_MISSING_BODY,
    DRIVER_SUMMON_WHILE_HELD,
];

/// Every stable code the control module refuses with (none yet).
pub const CONTROL_STABLE_REFUSAL_CODES: &[&str] = &[];
