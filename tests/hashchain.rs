//! PROPERTY TESTS — §56 / work-order A5 — hash-chain correctness suite.
//!
//! Covers: canonical encoding, prev-hash linkage, corruption detection,
//! mid-file truncation detection, end-truncation detection boundary
//! (via context-receipt head comparison — the chain alone cannot see a
//! cleanly truncated tail, and the suite asserts exactly that boundary),
//! duplicate detection, concurrent append, malformed-record behavior,
//! schema-2 validation, schema-1 grandfathered readability, determinism.
//!
//! All cases run sequentially inside ONE #[test] (CANON_DIR is
//! process-global; each case gets its own directory).

use canon::{cjson, context, store};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn base_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "canon-hashchain-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&d).unwrap();
    d
}

fn case(name: &str) -> PathBuf {
    let d = base_dir().join(name);
    fs::create_dir_all(&d).unwrap();
    // SAFETY: single #[test] runs sequentially; no other test in this
    // binary touches the process environment concurrently.
    unsafe { std::env::set_var("CANON_DIR", &d) };
    d
}

fn principle(id: &str, statement: &str) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("kind".into(), json!("principle"));
    m.insert("id".into(), json!(id));
    m.insert("statement".into(), json!(statement));
    m.insert("scope".into(), json!("test scope"));
    m.insert("authority".into(), json!("test authority"));
    m.insert("enforcement_point".into(), json!("test enforcement"));
    m.insert("violation_behavior".into(), json!("test violation behavior"));
    m.insert("evidence_method".into(), json!("test evidence method"));
    m.insert("severity".into(), json!("required"));
    m.insert("status".into(), json!("active"));
    m
}

fn file_lines(dir: &PathBuf, name: &str) -> Vec<String> {
    fs::read_to_string(dir.join(name))
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect()
}

// --- 1. canonical encoding ------------------------------------------------
fn t_canonical_encoding() {
    // sorted keys, no whitespace, nested sort
    let v = json!({"z":1,"a":{"y":[3,2,1],"b":true},"m":"x"});
    assert_eq!(
        cjson::canonical(&v).unwrap(),
        r#"{"a":{"b":true,"y":[3,2,1]},"m":"x","z":1}"#
    );
    // unicode passes through untouched (UTF-8, no escaping beyond JSON)
    let u = json!({"k":"héllo→"});
    assert!(cjson::canonical(&u).unwrap().contains("héllo→"));
    // floats are rejected — no formatting ambiguity in the chain
    assert!(cjson::canonical(&json!({"x":1.5})).is_err());
    assert!(cjson::canonical(&json!({"x":[1.0]})).is_err());
    // integers (incl. negative, large) are fine
    assert_eq!(cjson::canonical(&json!({"n":-9223372036854775808i64})).unwrap(), r#"{"n":-9223372036854775808}"#);
    // non-object top level rejected
    assert!(cjson::canonical(&json!([1,2])).is_err());
    // hash is stable across calls
    let h1 = cjson::hash_record(&v).unwrap();
    let h2 = cjson::hash_record(&v).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

// --- 2. prev-hash linkage --------------------------------------------------
fn t_prev_hash_linkage() {
    let dir = case("linkage");
    for i in 1..=3 {
        store::append("constitution", principle(&format!("T{}", i), &format!("stmt {}", i))).unwrap();
    }
    let entries = store::read_strict("constitution").unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].get("prev_hash").unwrap(), "GENESIS");
    let h = |e: &Map<String, Value>| e.get("hash").unwrap().as_str().unwrap().to_string();
    assert_eq!(entries[1].get("prev_hash").unwrap().as_str().unwrap(), h(&entries[0]));
    assert_eq!(entries[2].get("prev_hash").unwrap().as_str().unwrap(), h(&entries[1]));
    assert_eq!(store::verify_chain("constitution", &entries).unwrap(), 3);
    // each stored hash recomputes from the record minus "hash"
    for e in &entries {
        let mut no_hash = e.clone();
        let stored = no_hash.remove("hash").unwrap().as_str().unwrap().to_string();
        assert_eq!(cjson::hash_record(&Value::Object(no_hash)).unwrap(), stored);
    }
    let _ = dir;
}

// --- 3. corruption detection ------------------------------------------------
fn t_corruption_detection() {
    let dir = case("corrupt");
    for i in 1..=3 {
        store::append("constitution", principle(&format!("C{}", i), &format!("stmt {}", i))).unwrap();
    }
    let path = dir.join("constitution.jsonl");
    // flip one byte inside record 2's statement
    let mut lines = file_lines(&dir, "constitution.jsonl");
    lines[1] = lines[1].replacen("stmt 2", "stmt X", 1);
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    let entries = store::read_strict("constitution").unwrap();
    let err = store::verify_chain("constitution", &entries).unwrap_err();
    assert!(format!("{}", err).contains("hash mismatch"), "got: {}", err);
    // corrupt a prev_hash link instead
    let mut lines = file_lines(&dir, "constitution.jsonl");
    // restore record 2, break record 3's prev_hash
    lines[1] = lines[1].replacen("stmt X", "stmt 2", 1);
    lines[2] = lines[2].replacen(
        &entries[1].get("hash").unwrap().as_str().unwrap()[..16],
        "deadbeefdeadbeef",
        1,
    );
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    let entries = store::read_strict("constitution").unwrap();
    let err = store::verify_chain("constitution", &entries).unwrap_err();
    assert!(format!("{}", err).contains("prev_hash mismatch"), "got: {}", err);
}

// --- 4. mid-file truncation -------------------------------------------------
fn t_truncation_midfile() {
    let dir = case("trunc-mid");
    for i in 1..=3 {
        store::append("constitution", principle(&format!("M{}", i), &format!("stmt {}", i))).unwrap();
    }
    let mut lines = file_lines(&dir, "constitution.jsonl");
    lines.remove(1); // drop the middle record
    fs::write(dir.join("constitution.jsonl"), lines.join("\n") + "\n").unwrap();
    let entries = store::read_strict("constitution").unwrap();
    assert_eq!(entries.len(), 2);
    let err = store::verify_chain("constitution", &entries).unwrap_err();
    assert!(format!("{}", err).contains("prev_hash mismatch"), "got: {}", err);
}

// --- 5. end-truncation: the honest boundary --------------------------------
fn t_truncation_end_boundary() {
    let dir = case("trunc-end");
    for i in 1..=3 {
        store::append("constitution", principle(&format!("E{}", i), &format!("stmt {}", i))).unwrap();
    }
    let r1 = context::build("trunc-test").unwrap();
    assert!(r1.body.contains("canon_heads:"));
    // cleanly truncate the tail: the chain over the REMAINDER still verifies.
    // This is the documented boundary — end-truncation is NOT visible to the
    // chain alone.
    let mut lines = file_lines(&dir, "constitution.jsonl");
    lines.pop();
    fs::write(dir.join("constitution.jsonl"), lines.join("\n") + "\n").unwrap();
    let entries = store::read_strict("constitution").unwrap();
    assert!(store::verify_chain("constitution", &entries).is_ok());
    // ...but it IS visible against a prior context receipt's head hashes.
    let r2 = context::build("trunc-test").unwrap();
    assert_ne!(r1.hash, r2.hash, "receipt must change when the tail is truncated");
    let heads = |b: &str| {
        b.lines()
            .find(|l| l.starts_with("canon_heads:"))
            .unwrap()
            .to_string()
    };
    assert_ne!(heads(&r1.body), heads(&r2.body));
}

// --- 6. duplicate detection -------------------------------------------------
fn t_duplicate_detection() {
    let _dir = case("dup");
    store::append("constitution", principle("DUP", "first")).unwrap();
    let err = store::append("constitution", principle("DUP", "second")).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("already recorded"), "got: {}", msg);
    // chain still valid with exactly one record
    let entries = store::read_strict("constitution").unwrap();
    assert_eq!(entries.len(), 1);
    assert!(store::verify_chain("constitution", &entries).is_ok());
}

// --- 7. concurrent append ---------------------------------------------------
fn t_concurrent_append() {
    let dir = case("concurrent");
    let dir_s = dir.to_string_lossy().to_string();
    let mut handles = Vec::new();
    for t in 0..8 {
        let d = dir_s.clone();
        handles.push(std::thread::spawn(move || {
            unsafe { std::env::set_var("CANON_DIR", &d) };
            for i in 0..25 {
                let id = format!("W{}-{:03}", t, i);
                store::append("constitution", principle(&id, &format!("worker {} rec {}", t, i)))
                    .expect("concurrent append must succeed");
            }
        }));
    }
    for h in handles {
        h.join().expect("worker thread panicked");
    }
    unsafe { std::env::set_var("CANON_DIR", &dir_s) };
    let entries = store::read_strict("constitution").unwrap();
    assert_eq!(entries.len(), 200);
    // every expected id present exactly once
    let mut ids = BTreeSet::new();
    for e in &entries {
        let id = e.get("id").unwrap().as_str().unwrap().to_string();
        assert!(ids.insert(id), "duplicate id after concurrent append");
    }
    assert_eq!(ids.len(), 200);
    // chain intact end to end
    assert_eq!(store::verify_chain("constitution", &entries).unwrap(), 200);
}

// --- 8. malformed records ---------------------------------------------------
fn t_malformed_records() {
    let dir = case("malformed");
    let path = dir.join("constitution.jsonl");
    // not JSON at all
    fs::write(&path, "this is not json\n").unwrap();
    assert!(format!("{}", store::read_strict("constitution").unwrap_err()).contains("invalid JSON"));
    // valid JSON but not an object
    fs::write(&path, "[1,2,3]\n").unwrap();
    assert!(format!("{}", store::read_strict("constitution").unwrap_err()).contains("must be an object"));
    // object without schema
    fs::write(&path, "{\"id\":\"X\"}\n").unwrap();
    assert!(format!("{}", store::read_strict("constitution").unwrap_err()).contains("unsupported schema"));
    // unknown schema version
    fs::write(&path, "{\"id\":\"X\",\"schema\":99}\n").unwrap();
    assert!(format!("{}", store::read_strict("constitution").unwrap_err()).contains("unsupported schema"));
    // blank lines are skipped, not fatal
    fs::write(&path, "\n   \n{\"id\":\"X\",\"schema\":1}\n\n").unwrap();
    let entries = store::read_strict("constitution").unwrap();
    assert_eq!(entries.len(), 1);
    // missing file is an empty store, not corruption
    fs::remove_file(&path).unwrap();
    assert!(store::read_strict("constitution").unwrap().is_empty());
}

// --- 9. schema-2 validation -------------------------------------------------
fn t_schema_validation() {
    // missing enforcement_point → rejected
    let mut bad = principle("V1", "x");
    bad.remove("enforcement_point");
    assert!(format!("{}", store::validate_new("constitution", &bad).unwrap_err()).contains("enforcement_point"));
    // unknown field → rejected (no smuggled semantics, e.g. "verified")
    let mut sneaky = principle("V2", "x");
    sneaky.insert("verified".into(), json!(true));
    assert!(format!("{}", store::validate_new("constitution", &sneaky).unwrap_err()).contains("unknown field"));
    // wrong kind tag → rejected
    let mut wrong = principle("V3", "x");
    wrong.insert("kind".into(), json!("contract"));
    assert!(format!("{}", store::validate_new("constitution", &wrong).unwrap_err()).contains("kind tag"));
    // bad severity enum → rejected
    let mut sev = principle("V4", "x");
    sev.insert("severity".into(), json!("mandatory"));
    assert!(format!("{}", store::validate_new("constitution", &sev).unwrap_err()).contains("severity"));
    // empty required string → rejected
    let mut empty = principle("V5", "x");
    empty.insert("scope".into(), json!("  "));
    assert!(format!("{}", store::validate_new("constitution", &empty).unwrap_err()).contains("scope"));
    // the full record passes
    assert!(store::validate_new("constitution", &principle("V6", "x")).is_ok());
    // append() enforces validation too
    let _dir = case("validate-append");
    let mut bad2 = principle("V7", "x");
    bad2.remove("authority");
    assert!(store::append("constitution", bad2).is_err());
    assert!(store::read_strict("constitution").unwrap().is_empty());
}

// --- 10. schema-1 grandfathered readability ---------------------------------
fn t_schema1_readable() {
    let dir = case("schema1");
    fs::write(
        dir.join("constitution.jsonl"),
        "{\"id\":\"I9\",\"kind\":\"principle\",\"schema\":1,\"text\":\"legacy prose\"}\n",
    )
    .unwrap();
    let entries = store::read_strict("constitution").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get("id").unwrap(), "I9");
}

// --- 11. receipt determinism -----------------------------------------------
fn t_receipt_determinism() {
    let _dir = case("receipt-det");
    store::append("constitution", principle("R1", "stmt")).unwrap();
    let a = context::build("det").unwrap();
    let b = context::build("det").unwrap();
    // same CANON state → byte-identical body and hash (modulo git/revision
    // and PL state, which are environment, not CANON content)
    assert_eq!(a.body, b.body);
    assert_eq!(a.hash, b.hash);
}

// --- 12. CLI rejects unknown arguments (D-CANON-005) ------------------------
fn t_cli_rejects_unknown_args() {
    let dir = case("cli-unknown-arg");
    let bin = env!("CARGO_BIN_EXE_canon");
    let out = std::process::Command::new(bin)
        .env("CANON_DIR", &dir)
        .args([
            "add-principle",
            "id=CLI1",
            "statement=s",
            "scope=s",
            "authority=a",
            "enforcement_point=e",
            "violation_behavior=v",
            "evidence_method=m",
            "severity=required",
            "verified=true", // unknown: must be rejected, never dropped
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "unknown arg must exit 2");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("never silently dropped"), "got: {}", stderr);
    // nothing was written
    assert!(store::read_strict("constitution").unwrap().is_empty());
}

#[test]
fn hashchain_suite() {
    // one case per line so a failure names the case in the panic message
    t_canonical_encoding();
    t_prev_hash_linkage();
    t_corruption_detection();
    t_truncation_midfile();
    t_truncation_end_boundary();
    t_duplicate_detection();
    t_concurrent_append();
    t_malformed_records();
    t_schema_validation();
    t_schema1_readable();
    t_receipt_determinism();
    t_cli_rejects_unknown_args();
    // clean up the temp tree
    let _ = fs::remove_dir_all(std::env::temp_dir().join(format!(
        "canon-hashchain-{}",
        std::process::id()
    )));
}
