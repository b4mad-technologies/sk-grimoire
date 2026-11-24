//! Shared compiler-compatible hashing for bounded runtime document content.
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Hash every supplied field using UTF-16 object-key order and compact JSON.
/// Numbers must be unsigned JSON-safe integers (0 through 2^53 - 1), including
/// in nested arrays/objects. Returns a lowercase sha256: identity.
/// Callers remove envelope fields before hashing and enforce their own byte
/// bound, unique-key parsing, schema and publication/authority checks.
pub fn canonical_document_hash(value: &Value) -> Result<String, String> {
    let encoded = canonical(value)?;
    let digest = Sha256::digest(encoded.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("sha256:{digest}"))
}

fn canonical(value: &Value) -> Result<String, String> {
    Ok(match value {
        Value::Object(fields) => {
            let mut fields: Vec<_> = fields.iter().collect();
            fields.sort_by(|(left, _), (right, _)| left.encode_utf16().cmp(right.encode_utf16()));
            let fields = fields
                .into_iter()
                .map(|(key, value)| {
                    Ok(format!(
                        "{}:{}",
                        serde_json::to_string(key).map_err(|e| e.to_string())?,
                        canonical(value)?
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        Value::Number(number) => {
            let integer = number
                .as_u64()
                .filter(|n| *n <= 9_007_199_254_740_991)
                .ok_or("document number is not an unsigned JSON-safe integer")?;
            integer.to_string()
        }
        _ => serde_json::to_string(value).map_err(|e| e.to_string())?,
    })
}
