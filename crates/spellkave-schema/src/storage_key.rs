//! Storage keys (ADR-0146 D-1): a table whose natural key spans several
//! columns stores it as one `key: String` primary key holding the canonical
//! JSON array of the key values. Numbers stay numbers and strings are
//! JSON-escaped, so no separator can collide with an id. The server tables,
//! the manifest commit path and the row identity build the key through this
//! one function.

use serde_json::Value;

use crate::row_identity::canonical_json;

/// The storage key for the given key values, in key-column order.
pub fn storage_key(values: &[Value]) -> String {
    canonical_json(&Value::Array(values.to_vec()))
}

/// [`storage_key`] for a key made only of string columns.
pub fn storage_key_strs(values: &[&str]) -> String {
    let values: Vec<Value> = values.iter().map(|v| Value::from(*v)).collect();
    storage_key(&values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_a_canonical_json_array() {
        assert_eq!(
            storage_key_strs(&["srd/item/assassins-blood", "srd/cost_cp"]),
            r#"["srd/item/assassins-blood","srd/cost_cp"]"#
        );
    }

    #[test]
    fn separators_inside_ids_cannot_collide() {
        assert_ne!(
            storage_key_strs(&["a\",\"b", "c"]),
            storage_key_strs(&["a", "b\",\"c"])
        );
    }

    #[test]
    fn numbers_stay_numbers() {
        assert_eq!(
            storage_key(&[Value::from(4711_u64), Value::from("srd/hp")]),
            r#"[4711,"srd/hp"]"#
        );
    }
}
