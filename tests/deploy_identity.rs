//! Deployment identity integration tests (DEPLOYMENT-IDENTITY.md).
//!
//! All keys are EPHEMERAL TEST KEYS generated in-process — never persisted,
//! never production. TEST-KEY — NOT FOR PRODUCTION.
//!
//! All cases run sequentially inside ONE #[test]: CANON_DIR is process-global.
use canon::deploy::{self, Verdict};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use serde_json::Value;
use std::collections::HashMap;

fn test_key() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{:02x}", b)).collect()
}

fn sign_map(map: &serde_json::Map<String, Value>, key: &SigningKey) -> String {
    use ed25519_dalek::Signer;
    let payload = canon::cjson::canonical(&Value::Object(map.clone())).unwrap();
    let sig = key.sign(payload.as_bytes());
    let mut m = map.clone();
    m.insert("signature".into(), Value::String(hex(&sig.to_bytes())));
    canon::cjson::canonical(&Value::Object(m)).unwrap()
}

#[test]
fn deploy_identity_suite() {
    // Save the ambient CANON_DIR so we can restore it.
    let saved: HashMap<String, String> = std::env::vars().collect();
    let saved_canon_dir = saved.get("CANON_DIR").cloned();

    // ---- 1. valid manifest verifies → MATCH ----
    let k = test_key();
    let manifest = deploy::issue_manifest("test-deploy-1", &k).expect("issue");
    let v: Value = serde_json::from_str(&manifest).expect("manifest is JSON");
    assert_eq!(
        v["key_class"].as_str().unwrap(),
        "TEST-KEY — NOT FOR PRODUCTION",
        "every manifest must carry the test-key stamp"
    );
    let r = deploy::verify_manifest(&manifest);
    assert_eq!(r.verdict, Verdict::Match, "detail: {:?}", r.detail);

    // ---- 2. tampered manifest (flip one byte) → UNKNOWN ----
    let mut tampered = manifest.clone();
    let pos = tampered.find("\"issued_at\"").expect("issued_at present");
    // flip a byte inside the timestamp value, a few chars after the key
    let flip_at = pos + 14;
    let b = tampered.as_bytes()[flip_at];
    assert!(b.is_ascii_alphanumeric(), "flipping a timestamp char");
    let c = (b ^ 0x01) as char;
    assert!(c.is_ascii_alphanumeric());
    tampered.replace_range(flip_at..flip_at + 1, &c.to_string());
    let r = deploy::verify_manifest(&tampered);
    assert_eq!(r.verdict, Verdict::Unknown, "detail: {:?}", r.detail);

    // ---- 3. wrong-key manifest → UNKNOWN ----
    let other = test_key();
    let mut mv: Value = serde_json::from_str(&manifest).unwrap();
    mv["public_key"] = Value::String(hex(&other.verifying_key().to_bytes()));
    // NOTE: do NOT re-sign — the signature was made by k, the embedded key
    // is now other's. Verification must fail closed.
    let wrong_key = serde_json::to_string(&mv).unwrap();
    let r = deploy::verify_manifest(&wrong_key);
    assert_eq!(r.verdict, Verdict::Unknown, "detail: {:?}", r.detail);

    // ---- 4. authentic manifest, different constitution → MISMATCH ----
    let mut mm = mv.as_object().unwrap().clone();
    mm.remove("signature");
    mm.remove("public_key"); // will re-add below
    let mut heads = serde_json::Map::new();
    for kind in ["constitution", "contracts", "decisions", "supersessions"] {
        heads.insert(kind.into(), Value::String("0".repeat(64)));
    }
    mm.insert("canon_heads".into(), Value::Object(heads));
    mm.insert("snapshot_hash".into(), Value::String("1".repeat(64)));
    mm.insert(
        "public_key".into(),
        Value::String(hex(&k.verifying_key().to_bytes())),
    );
    let mismatched = sign_map(&mm, &k);
    let r = deploy::verify_manifest(&mismatched);
    assert_eq!(r.verdict, Verdict::Mismatch, "detail: {:?}", r.detail);

    // ---- 5. tampered local store → UNKNOWN (fail closed) ----
    let tmp = std::env::temp_dir().join("canon-deploy-test-stores");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let canon_dir = std::env::var("CANON_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/home/hatch/workspace/canon"));
    for f in [
        "constitution.jsonl",
        "contracts.jsonl",
        "decisions.jsonl",
        "supersessions.jsonl",
    ] {
        std::fs::copy(canon_dir.join(f), tmp.join(f)).unwrap();
    }
    // Corrupt one record hash in the copy.
    let cp = tmp.join("constitution.jsonl");
    let text = std::fs::read_to_string(&cp).unwrap();
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    let li = lines
        .iter()
        .position(|l| l.contains("\"hash\""))
        .expect("hash field");
    let hp = lines[li].find("\"hash\":\"").unwrap() + 8;
    let hb = lines[li].as_bytes()[hp];
    lines[li].replace_range(hp..hp + 1, &((hb ^ 0x01) as char).to_string());
    std::fs::write(&cp, lines.join("\n") + "\n").unwrap();

    unsafe { std::env::set_var("CANON_DIR", &tmp) };
    let r = deploy::verify_manifest(&manifest);
    assert_eq!(r.verdict, Verdict::Unknown, "detail: {:?}", r.detail);

    // Restore environment.
    unsafe {
        match saved_canon_dir {
            Some(v) => std::env::set_var("CANON_DIR", v),
            None => std::env::remove_var("CANON_DIR"),
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);

    // Sanity: the untouched manifest still MATCHes after restore.
    let r = deploy::verify_manifest(&manifest);
    assert_eq!(r.verdict, Verdict::Match, "detail: {:?}", r.detail);
}
