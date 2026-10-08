//! CANON deployment identity (DEPLOYMENT-IDENTITY.md).
//!
//! A deployment manifest binds a running deployment to the exact frozen
//! constitution it enforces: constitution commit + stable store heads +
//! snapshot hash, signed with ed25519 over the canonical bytes.
//!
//! KEY CLASS: every key this module will ever see in this lane is an
//! EPHEMERAL TEST KEY. Production custody is Zack-gated
//! (goal_b4eaabcc3b4d); see DEPLOYMENT-IDENTITY.md "Production key
//! custody". Manifests are stamped TEST-KEY — NOT FOR PRODUCTION.
use crate::{api::Canon, cjson, store, time};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::process::Command;

pub const KEY_CLASS_TEST: &str = "TEST-KEY — NOT FOR PRODUCTION";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("not hex".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// Parse a 32-byte ed25519 secret key from a hex file (whitespace trimmed).
/// The file must contain exactly 64 hex chars — nothing else is accepted.
pub fn load_signing_key(path: &str) -> Result<SigningKey, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("key file: {}", e))?;
    let bytes = unhex(&raw).map_err(|e| format!("key file: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!(
            "key file: expected 32 bytes (64 hex chars), got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(SigningKey::from_bytes(&arr))
}

fn git_head() -> String {
    let dir = store::canon_dir();
    Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn hostname() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn binary_sha256() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .map(|b| sha256_hex(&b))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Stable snapshot hash: the canonical bytes of Canon::snapshot() with
/// volatile fields stripped. `generated_at` changes every run, so it is
/// excluded — the hash must identify the frozen state, not the moment.
fn snapshot_hash(c: &Canon) -> Result<String, String> {
    let mut snap = c.snapshot().map_err(|e| e.to_string())?;
    if let Value::Object(ref mut m) = snap {
        m.remove("generated_at");
    }
    cjson::hash_record(&snap)
}

/// Build and sign a deployment manifest. Returns canonical JSON.
/// `operator_name` is operator-declared (never inferred).
pub fn issue_manifest(operator_name: &str, key: &SigningKey) -> Result<String, String> {
    if operator_name.trim().is_empty() {
        return Err("operator name must not be empty".into());
    }
    let canon = Canon::open().map_err(|e| e.to_string())?;
    let mut heads = Map::new();
    for (k, v) in canon.heads() {
        heads.insert(k, Value::String(v));
    }
    let verifying = key.verifying_key();
    let pub_hex = hex(&verifying.to_bytes());
    let key_id = &sha256_hex(&verifying.to_bytes())[..16];

    let mut build = Map::new();
    build.insert(
        "rustc_version".into(),
        Value::String(env!("CANON_RUSTC_VERSION").to_string()),
    );
    build.insert(
        "source_date_epoch".into(),
        Value::String(env!("CANON_SOURCE_DATE_EPOCH").to_string()),
    );
    let mut envm = Map::new();
    envm.insert("hostname".into(), Value::String(hostname()));
    envm.insert(
        "operator_name".into(),
        Value::String(operator_name.trim().to_string()),
    );

    let mut m = Map::new();
    m.insert(
        "canon_protocol".into(),
        Value::Number(store::PROTOCOL.into()),
    );
    m.insert("constitution_commit".into(), Value::String(git_head()));
    m.insert("canon_heads".into(), Value::Object(heads));
    m.insert(
        "snapshot_hash".into(),
        Value::String(snapshot_hash(&canon)?),
    );
    m.insert("canon_binary_sha256".into(), Value::String(binary_sha256()));
    m.insert("build".into(), Value::Object(build));
    m.insert("environment".into(), Value::Object(envm));
    m.insert("issued_at".into(), Value::String(time::now_iso()));
    m.insert("key_id".into(), Value::String(key_id.to_string()));
    m.insert(
        "key_class".into(),
        Value::String(KEY_CLASS_TEST.to_string()),
    );
    m.insert("public_key".into(), Value::String(pub_hex));

    // Sign the canonical bytes of everything above (signature covers the
    // embedded public key too — swapping the key breaks the signature).
    let payload = cjson::canonical(&Value::Object(m.clone()))?;
    let sig: Signature = key.sign(payload.as_bytes());
    m.insert("signature".into(), Value::String(hex(&sig.to_bytes())));
    cjson::canonical(&Value::Object(m)).map_err(|e| e.to_string())
}

/// Verification verdict. MATCH/MISMATCH are only reachable with a valid
/// signature; anything unattributable is UNKNOWN.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Match,
    Mismatch,
    Unknown,
}

pub struct VerifyReport {
    pub verdict: Verdict,
    pub detail: Vec<String>,
}

/// Verify a manifest against the local frozen CANON state.
pub fn verify_manifest(manifest_json: &str) -> VerifyReport {
    let mut detail = Vec::new();
    let fail = |reason: &str| VerifyReport {
        verdict: Verdict::Unknown,
        detail: vec![reason.to_string()],
    };
    let v: Value = match serde_json::from_str(manifest_json) {
        Ok(v) => v,
        Err(e) => return fail(&format!("manifest is not valid JSON: {}", e)),
    };
    let m = match v.as_object() {
        Some(m) => m,
        None => return fail("manifest top-level must be an object"),
    };
    let get = |k: &str| m.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let sig_hex = get("signature");
    let pub_hex = get("public_key");
    if sig_hex.is_empty() || pub_hex.is_empty() {
        return fail("manifest missing signature or public_key");
    }
    // Recompute the signed payload: canonical bytes without "signature".
    let mut payload_map = m.clone();
    payload_map.remove("signature");
    let payload = match cjson::canonical(&Value::Object(payload_map)) {
        Ok(p) => p,
        Err(e) => return fail(&format!("manifest payload not canonicalizable: {}", e)),
    };
    let sig_bytes = match unhex(&sig_hex) {
        Ok(b) if b.len() == 64 => {
            let mut a = [0u8; 64];
            a.copy_from_slice(&b);
            a
        }
        _ => return fail("signature is not 64 bytes of hex"),
    };
    let pub_bytes = match unhex(&pub_hex) {
        Ok(b) if b.len() == 32 => {
            let mut a = [0u8; 32];
            a.copy_from_slice(&b);
            a
        }
        _ => return fail("public_key is not 32 bytes of hex"),
    };
    let vk = match VerifyingKey::from_bytes(&pub_bytes) {
        Ok(v) => v,
        Err(_) => return fail("public_key is not a valid ed25519 point"),
    };
    // Defense in depth: the degenerate zero key must never verify anything,
    // even if a future crate version parses it.
    if pub_bytes == [0u8; 32] {
        return fail("public_key is the degenerate zero key");
    }
    let sig = Signature::from_bytes(&sig_bytes);
    if vk.verify(payload.as_bytes(), &sig).is_err() {
        return fail("UNKNOWN: signature does not verify (tampered manifest or unknown key)");
    }
    detail.push(format!("signature valid, key_id={}", get("key_id")));
    let key_class = get("key_class");
    detail.push(format!("key_class={}", key_class));
    if key_class != KEY_CLASS_TEST {
        detail.push(
            "WARNING: key_class is not the expected TEST-KEY stamp — treat with suspicion".into(),
        );
    }

    // Signature is good: now compare the constitution it names against
    // the local frozen state.
    let canon = match Canon::open() {
        Ok(c) => c,
        Err(e) => {
            return VerifyReport {
                verdict: Verdict::Unknown,
                detail: vec![format!(
                    "UNKNOWN: local CANON state failed to open ({}); cannot compare",
                    e
                )],
            };
        }
    };
    let local_heads = canon.heads();
    let mut mismatches = Vec::new();
    match m.get("canon_heads").and_then(|x| x.as_object()) {
        Some(mh) => {
            for (kind, local) in &local_heads {
                match mh.get(kind).and_then(|x| x.as_str()) {
                    Some(remote) if remote == local => {
                        detail.push(format!("head {} MATCH", kind));
                    }
                    Some(remote) => {
                        mismatches.push(format!(
                            "head {} differs: manifest={} local={}",
                            kind,
                            &remote[..16.min(remote.len())],
                            &local[..16.min(local.len())]
                        ));
                    }
                    None => mismatches.push(format!("head {} missing from manifest", kind)),
                }
            }
        }
        None => mismatches.push("manifest canon_heads is not an object".into()),
    }
    match snapshot_hash(&canon) {
        Ok(local_snap) => {
            let remote_snap = get("snapshot_hash");
            if remote_snap == local_snap {
                detail.push("snapshot_hash MATCH".into());
            } else {
                mismatches.push("snapshot_hash differs".into());
            }
        }
        Err(e) => {
            return VerifyReport {
                verdict: Verdict::Unknown,
                detail: vec![format!("UNKNOWN: local snapshot hash failed: {}", e)],
            };
        }
    }
    // Provenance fields are reported, never decided on.
    for k in [
        "constitution_commit",
        "canon_binary_sha256",
        "issued_at",
        "environment",
        "build",
    ] {
        if let Some(v) = m.get(k) {
            let s = match v {
                Value::String(x) => x.clone(),
                _ => serde_json::to_string(v).unwrap_or_default(),
            };
            detail.push(format!(
                "observed {}={}",
                k,
                s.chars().take(80).collect::<String>()
            ));
        }
    }
    if mismatches.is_empty() {
        detail.push("MATCH: manifest describes the local frozen constitution".into());
        VerifyReport {
            verdict: Verdict::Match,
            detail,
        }
    } else {
        detail.push("MISMATCH: manifest is authentic but names a different constitution".into());
        detail.extend(mismatches.into_iter().map(|x| format!("  - {}", x)));
        VerifyReport {
            verdict: Verdict::Mismatch,
            detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;

    fn test_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn key_file_roundtrip_strict() {
        let k = test_key();
        let hexs = hex(&k.to_bytes());
        let dir = std::env::temp_dir();
        let p = dir.join("canon-deploy-test.key");
        std::fs::write(&p, format!("{}\n", hexs)).unwrap();
        let loaded = load_signing_key(p.to_str().unwrap()).unwrap();
        assert_eq!(loaded.to_bytes(), k.to_bytes());
        std::fs::remove_file(&p).ok();
        // wrong length rejected
        let p2 = dir.join("canon-deploy-test-bad.key");
        std::fs::write(&p2, "abcd\n").unwrap();
        assert!(load_signing_key(p2.to_str().unwrap()).is_err());
        std::fs::remove_file(&p2).ok();
    }

    #[test]
    fn tampered_payload_is_unknown() {
        let k = test_key();
        let man = issue_manifest("test-deploy", &k).expect("issue");
        // Flip one byte inside the payload (not the signature field).
        let mut tampered = man.clone();
        let pos = tampered.find("constitution").expect("field present");
        let b = tampered.as_bytes()[pos];
        tampered.replace_range(
            pos..pos + 1,
            &[(b ^ 0x01) as char].iter().collect::<String>(),
        );
        let r = verify_manifest(&tampered);
        assert_eq!(r.verdict, Verdict::Unknown);
    }

    #[test]
    fn wrong_embedded_key_is_unknown() {
        let a = test_key();
        let b = test_key();
        let man = issue_manifest("test-deploy", &a).expect("issue");
        let mut v: Value = serde_json::from_str(&man).unwrap();
        // Swap the embedded public key for a different one; signature was
        // made by A, so verification against B must fail.
        v["public_key"] = Value::String(hex(&b.verifying_key().to_bytes()));
        let tampered = serde_json::to_string(&v).unwrap();
        let r = verify_manifest(&tampered);
        assert_eq!(r.verdict, Verdict::Unknown);
    }
}
