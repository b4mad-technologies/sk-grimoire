// #1968 backlog item 2 — the real fix for u64 precision loss at the wasm/JS
// JSON boundary. JS numbers are float64 (53-bit mantissa); an H3 cell index
// (or any other u64 manifest field) above `Number.MAX_SAFE_INTEGER` silently
// rounds the instant JS `JSON.parse`s it. The fix has two independent halves,
// applied to every u64-carrying `Manifest` field (see `U64_FIELDS_BY_ROW_KIND`
// below for the enumerated set) — NOT special-cased to H3 cells:
//
// 1. **Serialize stays untouched** — `Manifest`'s derived `Serialize` still
//    emits a native JSON number for every u64 field, exactly as before. This
//    is deliberate: `spellkave-format::render_manifest` re-serializes a parsed
//    `Manifest` through `serde_json::to_value` on its way to
//    `toml::to_string_pretty`, and the critical invariant (rendered TOML
//    keeps native integers, never gains quotes) falls out for free as long as
//    serialize never changes. Only `spellkave-validate-wasm`'s OWN JSON-facing
//    functions convert Number -> String on the way OUT to JS (see that
//    crate's `stringify_u64_fields`) — the string form exists ONLY at the
//    wasm/JS wire, never inside this crate's own Serialize impl.
//
// 2. **Deserialize is widened, not narrowed** — every u64 / `Option<u64>`
//    field uses `#[serde(deserialize_with = "...")]` from this module so it
//    accepts EITHER a native integer (what `toml::from_str` hands it when
//    parsing committed TOML, and what most manifest-authoring call sites that
//    never approach 2^53 still pass as plain JS numbers) OR a decimal-digit
//    string (what a JS caller must pass for a value that would round as a
//    number — the wire form `spellkave-validate-wasm` now emits). This is
//    the "single clean format" the wasm/JS wire uses (string, always);
//    widened deserialize on the Rust side is what lets that string round-trip
//    back into a native `u64` without forcing every existing small-value call
//    site (session `round_duration_ms`, `quantity`, ms fields, ...) to
//    start emitting digit strings for values that were never at risk.
pub fn u64_flexible<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct V;
    impl serde::de::Visitor<'_> for V {
        type Value = u64;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a u64, as a native integer or a decimal-digit string")
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }

        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<u64, E> {
            u64::try_from(v).map_err(|_| E::custom(format!("negative value {v} cannot be a u64")))
        }

        fn visit_u32<E: serde::de::Error>(self, v: u32) -> Result<u64, E> {
            Ok(v.into())
        }

        fn visit_i32<E: serde::de::Error>(self, v: i32) -> Result<u64, E> {
            u64::try_from(v).map_err(|_| E::custom(format!("negative value {v} cannot be a u64")))
        }

        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<u64, E> {
            if v.is_finite() && v.fract() == 0.0 && v >= 0.0 && v <= u64::MAX as f64 {
                Ok(v as u64)
            } else {
                Err(E::custom(format!(
                    "{v} is not a whole non-negative number representable as u64"
                )))
            }
        }

        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<u64, E> {
            v.parse::<u64>()
                .map_err(|err| E::custom(format!("invalid u64 digit string {v:?}: {err}")))
        }
    }
    deserializer.deserialize_any(V)
}

/// `Option<u64>` counterpart — bespoke (not `Option::deserialize` composed
/// with `u64_flexible`) because specifying `deserialize_with` on an
/// `Option<T>` field replaces serde's derive-generated Option-awareness
/// entirely; this Visitor supplies it back (`visit_none`/`visit_unit` for
/// JSON `null` and TOML's no-null "value is just absent" shape, `visit_some`
/// forwarding into `u64_flexible` for a present value).
pub fn option_u64_flexible<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = Option<u64>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an optional u64, as a native integer, decimal-digit string, or null")
        }

        fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
        where
            D2: serde::Deserializer<'de>,
        {
            u64_flexible(deserializer).map(Some)
        }
    }
    deserializer.deserialize_option(V)
}

/// `(top-level Manifest array key, u64-or-Option<u64> field names within each
/// row of that array)` — every field that loses precision crossing the
/// wasm/JS JSON boundary. Consumed ONLY by `spellkave-validate-wasm`'s
/// `stringify_u64_fields` to decide which `serde_json::Value::Number`s to
/// rewrite to `Value::String` on the way out to JS.
///
/// Deliberately scoped per ROW KIND, not a flat field-name set: the same
/// field name can carry different types in different rows —
/// `target_conditions.target_id` is `u64`, `modifier_target_definitions.
/// target_id` is `String` — so a name-only match would corrupt the latter.
/// `spellkave-format` (TOML parse/render) never reads this table; it always
/// sees the native `u64`/`toml::Value::Integer` `Manifest` field directly, so
/// the rendered TOML is untouched by this fix (the binding invariant).
pub const U64_FIELDS_BY_ROW_KIND: &[(&str, &[&str])] = &[
    ("role_bindings", &["expires_at_tick"]),
    ("binding_preconditions", &["expected_revision"]),
    (
        "place_graphs",
        &["origin_cell_res10", "origin_cell_res7", "origin_cell_res4"],
    ),
    ("place_nodes", &["cell_res10", "cell_res7", "cell_res4"]),
    ("place_edges", &["cell_anchor"]),
    (
        "location_definitions",
        &["cell_res10", "cell_res7", "cell_res4"],
    ),
    ("session_definitions", &["round_duration_ms"]),
    (
        "autonomy_jobs",
        &[
            "frequency_ms",
            "next_fire_at_ms",
            "repeats_remaining",
            "subject",
            "owner_actor",
        ],
    ),
    (
        "action_definitions",
        &["cost_time_ms", "reaction_window_ms"],
    ),
    ("target_conditions", &["target_id", "expires_at_tick"]),
    ("entities", &["quantity"]),
    ("object_state", &["expires_at_tick"]),
    ("entity_definition_initial_condition", &["duration_ticks"]),
    ("items", &["quantity"]),
    ("spatial_niche_attachments", &["place_edge_id"]),
    (
        "spatial_environment_state",
        &["place_edge_id", "updated_tick"],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Probe {
        #[serde(deserialize_with = "u64_flexible")]
        value: u64,
    }

    #[derive(Debug, Deserialize)]
    struct OptProbe {
        #[serde(default, deserialize_with = "option_u64_flexible")]
        value: Option<u64>,
    }

    #[test]
    fn accepts_native_number() {
        let probe: Probe = serde_json::from_value(serde_json::json!({ "value": 42 })).unwrap();
        assert_eq!(probe.value, 42);
    }

    #[test]
    fn accepts_digit_string_above_2_pow_53() {
        // 2^53 + 1 — the first integer JS float64 cannot represent exactly.
        let probe: Probe =
            serde_json::from_value(serde_json::json!({ "value": "9007199254740993" })).unwrap();
        assert_eq!(probe.value, 9_007_199_254_740_993);
    }

    #[test]
    fn accepts_full_width_u64_digit_string() {
        let probe: Probe =
            serde_json::from_value(serde_json::json!({ "value": u64::MAX.to_string() })).unwrap();
        assert_eq!(probe.value, u64::MAX);
    }

    #[test]
    fn rejects_negative_number() {
        assert!(serde_json::from_value::<Probe>(serde_json::json!({ "value": -1 })).is_err());
    }

    #[test]
    fn rejects_non_digit_string() {
        assert!(serde_json::from_value::<Probe>(serde_json::json!({ "value": "abc" })).is_err());
    }

    #[test]
    fn option_defaults_to_none_when_absent() {
        let probe: OptProbe = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(probe.value, None);
    }

    #[test]
    fn option_accepts_null() {
        let probe: OptProbe = serde_json::from_value(serde_json::json!({ "value": null })).unwrap();
        assert_eq!(probe.value, None);
    }

    #[test]
    fn option_accepts_string_above_2_pow_53() {
        let probe: OptProbe =
            serde_json::from_value(serde_json::json!({ "value": "9007199254740993" })).unwrap();
        assert_eq!(probe.value, Some(9_007_199_254_740_993));
    }
}
