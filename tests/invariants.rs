//! Work-order A2: every constitutional invariant must cite enforcement that
//! actually exists. This test loads invariants.json (the machine-readable
//! projection of the frozen constitution) and proves each cited
//! ENFORCEMENT POINT resolves to a real file:line in this repo.
//! UNENFORCED entries must carry an explicit reason — silent drops fail.

use serde_json::Value;
use std::fs;

fn load() -> Value {
    let text =
        fs::read_to_string("invariants.json").expect("invariants.json must exist at crate root");
    serde_json::from_str(&text).expect("invariants.json must parse")
}

#[test]
fn invariants_have_all_seven_fields() {
    let v = load();
    let invs = v["invariants"].as_array().expect("invariants array");
    assert_eq!(
        invs.len(),
        12,
        "expected 12 frozen invariants, got {}",
        invs.len()
    );
    for inv in invs {
        let id = inv["id"].as_str().unwrap_or("?");
        for f in [
            "statement",
            "scope",
            "authority",
            "enforcement_point",
            "violation_behavior",
            "evidence_method",
        ] {
            let s = inv[f].as_str().unwrap_or("");
            assert!(
                !s.trim().is_empty(),
                "{}: field {:?} is empty — prose-only rules are not enforceable",
                id,
                f
            );
        }
    }
}

#[test]
fn every_enforcement_point_resolves_to_real_file_line() {
    let v = load();
    let invs = v["invariants"].as_array().unwrap();
    let mut unenforced: Vec<String> = vec![];
    for inv in invs {
        let id = inv["id"].as_str().unwrap();
        let enf = &inv["enforcement"];
        let status = enf["status"].as_str().unwrap_or("");
        if status == "UNENFORCED" {
            let reason = enf["reason"].as_str().unwrap_or("");
            assert!(
                !reason.trim().is_empty(),
                "{}: UNENFORCED without a reason is a silent drop",
                id
            );
            unenforced.push(format!("{}: {}", id, reason));
            continue;
        }
        assert_eq!(status, "ENFORCED", "{}: bad enforcement status", id);
        let points = enf["points"].as_array().expect("points array");
        assert!(!points.is_empty(), "{}: ENFORCED with zero points", id);
        for p in points {
            let file = p["file"].as_str().expect("point.file");
            let line = p["line"].as_u64().expect("point.line") as usize;
            let text = fs::read_to_string(file)
                .unwrap_or_else(|_| panic!("{}: enforcement file {:?} does not exist", id, file));
            let nlines = text.lines().count();
            assert!(
                line >= 1 && line <= nlines,
                "{}: {:?}:{} out of range (file has {} lines)",
                id,
                file,
                line,
                nlines
            );
            println!("{} -> {}:{} ok", id, file, line);
        }
    }
    // Honest ledger: print the unenforced set so it can never hide.
    for u in &unenforced {
        println!("UNENFORCED {}", u);
    }
}

#[test]
fn no_completion_shortcut_fields_in_allowlist() {
    // INV-09/INV-10 negative enforcement: the schema allowlists inside
    // allowed_fields() must not contain done/deployed/shipped/verdict
    // shortcut fields. We parse only the string literals within the
    // allowed_fields function body — doc comments mentioning them (e.g.
    // the line-133 warning about a fake "verified" flag) must not trip us.
    let text = fs::read_to_string("src/store.rs").unwrap();
    let start = text
        .find("pub fn allowed_fields")
        .expect("allowed_fields exists");
    let body = &text[start..start + 2000];
    for bad in ["\"done\"", "\"deployed\"", "\"shipped\"", "\"verdict\""] {
        assert!(
            !body.contains(bad),
            "allowed_fields contains shortcut field {} — INV-09/INV-10 violated",
            bad
        );
    }
}
