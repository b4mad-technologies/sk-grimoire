//! Shared rules for deriving action-economy windows outside a session.

/// Core turn-boundary tokens a channel can name in `resets_on`.
pub const TURN_START_BOUNDARY: &str = "turn_start";
pub const TURN_END_BOUNDARY: &str = "turn_end";

/// Outside an active session, both core turn boundaries derive their
/// reset window from elapsed time.
pub fn uses_derived_window(in_active_session: bool, resets_on: &str) -> bool {
    !in_active_session && matches!(resets_on, TURN_START_BOUNDARY | TURN_END_BOUNDARY)
}

/// Floor `now_ms` to the nearest `round_duration_ms` boundary at or before
/// it. `round_duration_ms` of `0` is treated as `1` so this never panics on
/// `%` for a malformed zero round duration.
pub fn window_start_ms(now_ms: u64, round_duration_ms: u64) -> u64 {
    let duration = round_duration_ms.max(1);
    now_ms - now_ms % duration
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_start_ms_floors_to_the_round_boundary() {
        assert_eq!(window_start_ms(11_999, 6_000), 6_000);
        assert_eq!(window_start_ms(12_000, 6_000), 12_000);
    }

    #[test]
    fn window_start_ms_treats_zero_duration_as_one() {
        assert_eq!(window_start_ms(5, 0), 5);
    }

    /// sk-63r: both core boundaries drive the derived window out of session;
    /// neither does in session; a non-core `resets_on` never does.
    #[test]
    fn uses_derived_window_true_only_for_core_boundaries_out_of_session() {
        assert!(uses_derived_window(false, "turn_start"));
        assert!(!uses_derived_window(true, "turn_start"));
        assert!(uses_derived_window(false, "turn_end"));
        assert!(!uses_derived_window(true, "turn_end"));
        assert!(!uses_derived_window(false, "short_rest"));
        assert!(!uses_derived_window(true, "short_rest"));
    }
}
