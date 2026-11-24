//! Pure grant composition. Hosts inject observations and caller-owned apply ports.
use serde::{Deserialize, Serialize};
use spellkave_schema::{Manifest, MetadataValue, RetractionRow, RoleBindingRow};

pub const MAX_BINDING_KEY_BYTES: usize = 2048;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantInput {
    pub role: String,
    pub subject: String,
    pub extent_kind: String,
    pub extent: String,
    #[serde(default)]
    pub bundle: String,
    #[serde(default, deserialize_with = "spellkave_schema::option_u64_flexible")]
    pub expires_at_tick: Option<u64>,
    #[serde(default)]
    pub description: String,
}
fn base64url(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(CHARS[(n & 63) as usize] as char);
        }
    }
    out
}
pub fn compose(
    input: &GrantInput,
    identity: &str,
    existing: &[RoleBindingRow],
) -> Result<RoleBindingRow, String> {
    let role = input.role.trim();
    let subject = if input.subject.trim() == "$sender" {
        identity
    } else {
        input.subject.trim()
    };
    let kind = if input.extent_kind.trim().is_empty() {
        "place_graph"
    } else {
        input.extent_kind.trim()
    };
    let reference = input.extent.trim();
    if [role, subject, reference].contains(&"") || reference == "*" {
        return Err("grant requires a concrete role, subject and extent".into());
    }
    let subject = if subject.len() == 64 && subject.bytes().all(|b| b.is_ascii_hexdigit()) {
        subject.to_lowercase()
    } else {
        subject.to_owned()
    };
    let mut tuple = Vec::new();
    for part in [role, &subject, kind, reference] {
        let count = u32::try_from(part.len()).map_err(|_| "grant key input too long")?;
        tuple.extend(count.to_be_bytes());
        tuple.extend(part.as_bytes());
    }
    let binding_id = format!("grants/v1/{}", base64url(&tuple));
    if binding_id.len() > MAX_BINDING_KEY_BYTES {
        return Err("grant key exceeds 2048 bytes; refusing truncation".into());
    }
    let row = RoleBindingRow {
        binding_id,
        bundle_id: input.bundle.clone(),
        subject,
        role_id: role.into(),
        extent_kind_ref: kind.into(),
        extent_ref: reference.into(),
        expires_at_tick: input.expires_at_tick,
        description: input.description.clone(),
        handover_note: None,
    };
    if existing.iter().any(|old| {
        old.binding_id == row.binding_id
            && (old.role_id != row.role_id
                || old.subject != row.subject
                || old.extent_kind_ref != row.extent_kind_ref
                || old.extent_ref != row.extent_ref
                || old.bundle_id != row.bundle_id)
    }) {
        return Err("binding id collision; refusing takeover".into());
    }
    Ok(row)
}
pub fn delta(
    rows: Vec<RoleBindingRow>,
    revoke: Vec<String>,
    instance: Option<&str>,
) -> Result<String, String> {
    let mut manifest = Manifest {
        role_bindings: rows,
        retractions: revoke
            .into_iter()
            .map(|authored_key| RetractionRow {
                row_kind: "role_binding".into(),
                authored_key,
            })
            .collect(),
        ..Default::default()
    };
    if let Some(instance) = instance {
        manifest.metadata.insert(
            "expected_world_instance_id".into(),
            MetadataValue::String(instance.into()),
        );
    }
    let bytes = serde_json::to_vec(&manifest)
        .map_err(|e| e.to_string())?
        .len();
    if manifest.role_bindings.len() + manifest.retractions.len() > 128 || bytes > 262144 {
        return Err("binding batch exceeds 128 mutations or 262144 canonical bytes".into());
    }
    spellkave_format::render_manifest(&manifest).map_err(|e| e.to_string())
}
pub fn restored_expiry(
    expiry: Option<u64>,
    source_tick: u64,
    destination_tick: u64,
    same_epoch: bool,
    rebase: bool,
) -> Result<Option<u64>, String> {
    if !same_epoch && !rebase {
        return Err("cross-epoch restore requires explicit expiry rebase".into());
    }
    let Some(expiry) = expiry else {
        return Ok(None);
    };
    let remaining = expiry
        .checked_sub(source_tick)
        .filter(|n| *n > 0)
        .ok_or("source grant expired; cannot reactivate")?;
    if same_epoch {
        Ok(Some(expiry))
    } else {
        destination_tick
            .checked_add(remaining)
            .map(Some)
            .ok_or("restored expiry overflows u64".into())
    }
}
#[derive(Deserialize)]
struct Request {
    input: GrantInput,
    identity: String,
    #[serde(default)]
    existing: Vec<RoleBindingRow>,
}
pub fn compose_json(json: &str) -> Result<String, String> {
    let r: Request = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let row = compose(&r.input, &r.identity, &r.existing)?;
    let mut value = serde_json::to_value(&row).map_err(|e| e.to_string())?;
    value["expires_at_tick"] = row
        .expires_at_tick
        .map(|t| serde_json::Value::String(t.to_string()))
        .unwrap_or(serde_json::Value::Null);
    serde_json::to_string(&value).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> GrantInput {
        GrantInput {
            role: "a/b".into(),
            subject: "c".into(),
            extent_kind: "place_graph".into(),
            extent: "forest".into(),
            bundle: "".into(),
            expires_at_tick: None,
            description: "".into(),
        }
    }
    #[test]
    fn injective_keys_retry_unicode_and_collision() {
        let a = compose(&input(), "identity", &[]).unwrap();
        assert_eq!(
            a,
            compose(&input(), "identity", std::slice::from_ref(&a)).unwrap()
        );
        let mut b = input();
        b.role = "a".into();
        b.subject = "b/c".into();
        assert_ne!(
            a.binding_id,
            compose(&b, "identity", &[]).unwrap().binding_id
        );
        b.extent_kind = "party".into();
        assert_ne!(
            a.binding_id,
            compose(&b, "identity", &[]).unwrap().binding_id
        );
        b.role = "é/🧙".into();
        assert!(compose(&b, "identity", &[]).is_ok());
        let mut collision = a.clone();
        collision.subject = "other".into();
        assert!(compose(&input(), "identity", &[collision]).is_err());
        b.role = "a".repeat(2048);
        assert!(compose(&b, "identity", &[]).is_err());
    }
    #[test]
    fn recovery_never_renews_expired_grants() {
        assert_eq!(restored_expiry(Some(100), 90, 3, false, true), Ok(Some(13)));
        assert_eq!(
            restored_expiry(Some(100), 90, 3, true, false),
            Ok(Some(100))
        );
        assert_eq!(restored_expiry(None, 90, 3, false, true), Ok(None));
        assert!(restored_expiry(Some(90), 90, 3, false, true).is_err());
        assert!(restored_expiry(Some(100), 90, 3, false, false).is_err());
    }
}

#[derive(Deserialize)]
struct DeltaInput {
    #[serde(default)]
    rows: Vec<RoleBindingRow>,
    #[serde(default)]
    revoke: Vec<String>,
    instance: Option<String>,
}
pub fn delta_json(json: &str) -> Result<String, String> {
    let r: DeltaInput = serde_json::from_str(json).map_err(|e| e.to_string())?;
    delta(r.rows, r.revoke, r.instance.as_deref())
}
