//! Wave-25: verification-depth gap battery for SPEC-CANON.
//!
//! Closes SPEC-GAP-V-01..V-06 (the six MEDIUM/LOW gaps filed by the
//! wave-23 spec-audit-canon-trace lane — invariants that were enforced
//! but only thinly tested):
//!
//! - V-01: search with no match prints the NO ACTIVE DECISION FOUND marker
//! - V-02: unavailable PL degrades to UNKNOWN, never invents state
//! - V-03: the CLI command set is closed (no update/delete can sneak in)
//! - V-04: supersede lifecycle — dangling endpoints rejected, SUPERSEDED
//!   surfaced in search, self-supersede rejected
//! - V-05: context receipt keeps revision:/working_tree:/PL-state as three
//!   separate fields (no flattening)
//! - V-06: dangling constitution/decision refs rejected (exit 4); every
//!   appended record carries an actor stamp
//!
//! Also re-verifies NIH-FIND-9 independently: every invariants.json
//! enforcement citation resolves to a real line whose CONTENT matches the
//! note (semantic anchors), not just a line in range.
//!
//! All cases run sequentially inside ONE #[test] (CANON_DIR / PL_BIN are
//! process-global; this binary is its own process). CLI cases spawn the
//! real `canon` binary via CARGO_BIN_EXE_canon — real behavior, no mocks.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn fresh_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("canon-verify-gaps-{}-{}", std::process::id(), tag));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn bin() -> String {
    env!("CARGO_BIN_EXE_canon").to_string()
}

/// Run the real CLI in `dir` with a deliberately-unusable PL binary
/// (hermetic: never touches the real pl binary).
fn run<S: AsRef<std::ffi::OsStr>>(dir: &PathBuf, args: &[S]) -> Output {
    Command::new(bin())
        .env("CANON_DIR", dir)
        .env("PL_BIN", dir.join("no-such-pl-binary"))
        .args(args)
        .output()
        .expect("failed to spawn canon binary")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn principle_args(id: &str, statement: &str) -> Vec<String> {
    vec![
        "add-principle".into(),
        format!("id={}", id),
        format!("statement={}", statement),
        "scope=test scope".into(),
        "authority=test authority".into(),
        "enforcement_point=test enforcement".into(),
        "violation_behavior=test violation".into(),
        "evidence_method=test evidence".into(),
        "severity=required".into(),
    ]
}

fn decision_args(id: &str, question: &str) -> Vec<String> {
    vec![
        "add-decision".into(),
        format!("id={}", id),
        format!("question={}", question),
        "decision=decided-yes".into(),
        "alternatives=no".into(),
        "reason=tested".into(),
        "evidence=gap-battery".into(),
    ]
}

fn contract_args(id: &str, extra: &[(&str, &str)]) -> Vec<String> {
    let mut v = vec![
        "add-contract".into(),
        format!("id={}", id),
        "subject=test subject".into(),
        "text=test text".into(),
        "inputs=x".into(),
        "outputs=y".into(),
        "errors=none".into(),
        "authorization=test".into(),
        "side_effects=none".into(),
        "determinism=yes".into(),
        "version=1".into(),
        "evidence_requirements=test".into(),
    ];
    for (k, val) in extra {
        v.push(format!("{}={}", k, val));
    }
    v
}

const FROZEN_COMMANDS: &[&str] = &[
    "add-principle",
    "add-contract",
    "add-decision",
    "supersede",
    "search",
    "context",
    "audit",
    "snapshot",
    "deploy-identity",
    "verify-deployment",
];

// --- V-01: search-no-match marker -------------------------------------------
fn v01_search_no_match() {
    let dir = fresh_dir("v01");
    // positive marker case: empty store, no match
    let o = run(&dir, &["search", "zxqwv-gibberish-000"]);
    assert_eq!(o.status.code(), Some(0), "search exit: {}", err(&o));
    assert!(
        out(&o).contains("NO ACTIVE DECISION FOUND"),
        "missing no-match marker: {}",
        out(&o)
    );
    // adversarial: a real match must NOT print the marker
    let pa_owned = principle_args("SEARCHP1", "unique marker zebra fish");
    let pa: Vec<&str> = pa_owned.iter().map(|s| s.as_str()).collect();
    let o = run(&dir, &pa);
    assert_eq!(o.status.code(), Some(0), "add-principle: {}", err(&o));
    let o = run(&dir, &["search", "zebra"]);
    assert_eq!(o.status.code(), Some(0), "search hit: {}", err(&o));
    assert!(
        !out(&o).contains("NO ACTIVE DECISION FOUND"),
        "marker must not print on a hit: {}",
        out(&o)
    );
    assert!(out(&o).contains("SEARCHP1"), "hit id missing: {}", out(&o));
    // adversarial: empty query is usage error, not a silent empty result
    let o = run(&dir, &["search"]);
    assert_eq!(o.status.code(), Some(2), "empty query must be exit 2");
}

// --- V-02: unavailable-PL degradation (library-level) ------------------------
fn v02_pl_unavailable() {
    let dir = fresh_dir("v02");
    // SAFETY: single #[test] runs sequentially; no other test in this
    // binary touches the process environment.
    unsafe {
        std::env::set_var("CANON_DIR", &dir);
        std::env::set_var("PL_BIN", dir.join("no-such-pl-binary"));
    }
    let r = canon::context::build("v02-probe").expect("context build failed");
    // the UNKNOWN-not-assumed marker must be present ...
    assert!(
        r.body
            .contains("(unavailable: pl binary missing or failed — PL state UNKNOWN, not assumed)"),
        "missing PL-unavailable marker:\n{}",
        r.body
    );
    // ... and no PL state may be invented (adversarial: a naive fallback
    // could print an empty/zeroed row)
    assert!(
        !r.body.contains("implemented="),
        "PL rows must not be fabricated when the binary is missing:\n{}",
        r.body
    );
    assert!(
        r.body
            .contains("PL state unavailable — verify pl binary, then re-run context."),
        "required-next-action must degrade honestly:\n{}",
        r.body
    );
}

// --- V-03: closed command set ------------------------------------------------
fn v03_closed_command_set() {
    let dir = fresh_dir("v03");
    // unknown commands rejected, never dispatched
    for cmd in ["update-foo", "delete", "remove", "edit"] {
        let o = run(&dir, &[cmd, "x=1"]);
        assert_eq!(
            o.status.code(),
            Some(2),
            "{:?} must exit 2, got {:?}: {}",
            cmd,
            o.status.code(),
            err(&o)
        );
        assert!(
            err(&o).contains("unknown command"),
            "{:?} must say 'unknown command': {}",
            cmd,
            err(&o)
        );
    }
    // the usage string enumerates exactly the frozen set — a new command
    // that isn't in usage (or one added to usage silently) fails here
    let o = run::<&str>(&dir, &[]);
    assert_eq!(o.status.code(), Some(2));
    let usage = err(&o);
    assert!(usage.contains("usage: canon"), "no usage line: {}", usage);
    let list = usage
        .split("usage: canon")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    let mut got: Vec<&str> = list.split('|').collect();
    got.sort_unstable();
    let mut want: Vec<&str> = FROZEN_COMMANDS.to_vec();
    want.sort_unstable();
    assert_eq!(got, want, "command set drifted from frozen list");
    for forbidden in ["update", "delete", "remove", "edit", "drop", "rewrite"] {
        assert!(
            !list.split('|').any(|c| c == forbidden),
            "forbidden command {:?} in usage",
            forbidden
        );
    }
}

// --- V-04: supersede lifecycle ------------------------------------------------
fn v04_supersede_lifecycle() {
    let dir = fresh_dir("v04");
    let pa_owned = principle_args("SUPP1", "lifecycle principle");
    let pa: Vec<&str> = pa_owned.iter().map(|s| s.as_str()).collect();
    assert_eq!(run(&dir, &pa).status.code(), Some(0));
    for (id, q) in [("SUPSD1", "first question"), ("SUPSD2", "second question")] {
        let da_owned = decision_args(id, q);
        let da: Vec<&str> = da_owned.iter().map(|s| s.as_str()).collect();
        let o = run(&dir, &da);
        assert_eq!(o.status.code(), Some(0), "add-decision {}: {}", id, err(&o));
    }
    // positive: valid supersede lands
    let o = run(
        &dir,
        &[
            "supersede",
            "from=SUPSD1",
            "to=SUPSD2",
            "reason=lifecycle-test",
        ],
    );
    assert_eq!(o.status.code(), Some(0), "supersede: {}", err(&o));
    assert!(out(&o).contains("superseded by"), "{}", out(&o));
    // C12: superseded status surfaces in search — never flattened away
    let o = run(&dir, &["search", "SUPSD1"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(
        out(&o).contains("SUPERSEDED"),
        "SUPSD1 must surface SUPERSEDED: {}",
        out(&o)
    );
    // adversarial: the successor must NOT be flattened to SUPERSEDED too
    let o = run(&dir, &["search", "SUPSD2"]);
    assert_eq!(o.status.code(), Some(0));
    let line = out(&o)
        .lines()
        .find(|l| l.contains("SUPSD2"))
        .unwrap_or("")
        .to_string();
    assert!(
        !line.contains("SUPERSEDED"),
        "SUPSD2 wrongly superseded: {}",
        line
    );
    // adversarial: dangling from-endpoint rejected (exit 4)
    let o = run(&dir, &["supersede", "from=NOPE", "to=SUPSD2", "reason=x"]);
    assert_eq!(o.status.code(), Some(4), "dangling from must exit 4");
    assert!(err(&o).contains("does not exist"), "{}", err(&o));
    // adversarial: dangling to-endpoint rejected (exit 4)
    let o = run(&dir, &["supersede", "from=SUPSD2", "to=NOPE", "reason=x"]);
    assert_eq!(o.status.code(), Some(4), "dangling to must exit 4");
    // adversarial: self-supersede rejected (exit 2)
    let o = run(&dir, &["supersede", "from=SUPSD2", "to=SUPSD2", "reason=x"]);
    assert_eq!(o.status.code(), Some(2), "self-supersede must exit 2");
    // adversarial: missing reason rejected (exit 2)
    let o = run(&dir, &["supersede", "from=SUPSD2", "to=SUPSD1"]);
    assert_eq!(o.status.code(), Some(2), "missing reason must exit 2");
}

// --- V-05: receipt three-field separation (library-level) ----------------------
fn v05_three_field_separation() {
    let dir = fresh_dir("v05");
    // SAFETY: sequential within this one #[test].
    unsafe {
        std::env::set_var("CANON_DIR", &dir);
        std::env::set_var("PL_BIN", dir.join("no-such-pl-binary"));
    }
    let r = canon::context::build("v05-probe").expect("context build failed");
    let lines: Vec<&str> = r.body.lines().collect();
    let rev = lines
        .iter()
        .position(|l| l.starts_with("revision: "))
        .expect("revision: line missing");
    let wt = lines
        .iter()
        .position(|l| l.starts_with("working_tree: "))
        .expect("working_tree: line missing");
    let pl = lines
        .iter()
        .position(|l| *l == "CURRENT PL STATE")
        .expect("CURRENT PL STATE section missing");
    // three separate fields, in fixed order
    assert!(rev < wt && wt < pl, "field order broken:\n{}", r.body);
    assert_ne!(
        lines[rev], lines[wt],
        "revision and working_tree must not be flattened together"
    );
    // adversarial: exactly one of each — a duplicated/merged field fails
    assert_eq!(
        lines.iter().filter(|l| l.starts_with("revision: ")).count(),
        1
    );
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.starts_with("working_tree: "))
            .count(),
        1
    );
}

// --- V-06: dangling refs + actor stamp -----------------------------------------
fn v06_dangling_ref_actor_stamp() {
    let dir = fresh_dir("v06");
    let pa_owned = principle_args("REFP1", "reference principle");
    let pa: Vec<&str> = pa_owned.iter().map(|s| s.as_str()).collect();
    assert_eq!(run(&dir, &pa).status.code(), Some(0));
    // adversarial: dangling constitution ref rejected (exit 4)
    let ca_owned = contract_args("REFC1", &[("constitution", "BOGUS")]);
    let ca: Vec<&str> = ca_owned.iter().map(|s| s.as_str()).collect();
    let o = run(&dir, &ca);
    assert_eq!(o.status.code(), Some(4), "dangling constitution ref");
    assert!(err(&o).contains("dangling constitution ref"), "{}", err(&o));
    // positive: valid constitution ref lands
    let ca_owned = contract_args("REFC1", &[("constitution", "REFP1")]);
    let ca: Vec<&str> = ca_owned.iter().map(|s| s.as_str()).collect();
    let o = run(&dir, &ca);
    assert_eq!(o.status.code(), Some(0), "valid contract: {}", err(&o));
    // adversarial: dangling decision ref rejected (exit 4)
    let ca_owned = contract_args("REFC2", &[("decisions", "NOPE")]);
    let ca: Vec<&str> = ca_owned.iter().map(|s| s.as_str()).collect();
    let o = run(&dir, &ca);
    assert_eq!(o.status.code(), Some(4), "dangling decision ref");
    assert!(err(&o).contains("dangling decision ref"), "{}", err(&o));
    // actor stamp: every appended record carries a non-empty actor
    for name in ["constitution.jsonl", "contracts.jsonl"] {
        let text = fs::read_to_string(dir.join(name)).expect("store file missing");
        assert!(!text.trim().is_empty(), "{} must have records", name);
        for (i, line) in text.lines().enumerate() {
            let v: Value = serde_json::from_str(line).unwrap();
            let actor = v.get("actor").and_then(|a| a.as_str()).unwrap_or("");
            assert!(
                !actor.is_empty(),
                "{} line {}: anonymous record (actor stamp missing)",
                name,
                i + 1
            );
        }
    }
}

// --- NIH-FIND-9 re-verification: semantic anchors -------------------------------
// (inv_id, file, line, anchor substring that must appear on the cited line.
// Anchors verified against the tree at write time; drift fails the build.)
const ANCHORS: &[(&str, &str, u32, &str)] = &[
    ("INV-01", "src/main.rs", 695, "usage: canon"),
    ("INV-01", "src/main.rs", 715, "\"context\" | \"audit\""),
    ("INV-01", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-02", "src/main.rs", 483, "NO ACTIVE DECISION FOUND"),
    (
        "INV-02",
        "src/context.rs",
        252,
        "PL state UNKNOWN, not assumed",
    ),
    ("INV-03", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-03", "src/store.rs", 169, "pub fn validate_new"),
    ("INV-03", "src/main.rs", 56, "fn reject_unknown"),
    (
        "INV-04",
        "src/context.rs",
        252,
        "PL state UNKNOWN, not assumed",
    ),
    ("INV-05", "src/store.rs", 67, "pub fn read_strict"),
    ("INV-05", "src/store.rs", 298, "pub fn verify_chain"),
    ("INV-06", "src/store.rs", 221, "pub fn append"),
    ("INV-06", "src/main.rs", 695, "usage: canon"),
    ("INV-06", "src/store.rs", 228, "duplicate-id guard"),
    ("INV-07", "src/main.rs", 369, "fn cmd_supersede"),
    ("INV-07", "src/main.rs", 384, "both endpoints must exist"),
    ("INV-08", "src/context.rs", 161, "revision:"),
    ("INV-08", "src/context.rs", 162, "working_tree:"),
    ("INV-09", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-10", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-10", "src/main.rs", 695, "usage: canon"),
    (
        "INV-11",
        "src/context.rs",
        280,
        "fn receipt_deterministic_given_inputs",
    ),
    ("INV-11", "Cargo.toml", 9, "sha2"),
    ("INV-12", "src/store.rs", 265, "actor"),
    ("INV-12", "src/main.rs", 214, "dangling constitution ref {:?}"),
    ("INV-12", "src/main.rs", 235, "dangling decision ref {:?}"),
];

fn nih9_reverify() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("invariants.json")).expect("invariants.json missing");
    let v: Value = serde_json::from_str(&text).expect("invariants.json must parse");
    let invs = v["invariants"].as_array().expect("invariants array");
    assert_eq!(invs.len(), 12, "expected 12 frozen invariants");
    let mut covered = 0usize;
    for inv in invs {
        let id = inv["id"].as_str().unwrap();
        for p in inv["enforcement"]["points"].as_array().unwrap() {
            let file = p["file"].as_str().unwrap();
            let line = p["line"].as_u64().unwrap() as u32;
            let (aid, afile, aline, anchor) = ANCHORS
                .iter()
                .find(|(a, f, l, _)| *a == id && *f == file && *l == line)
                .unwrap_or_else(|| panic!("no semantic anchor for {} {}:{}", id, file, line));
            let _ = (aid, afile, aline);
            let src = fs::read_to_string(root.join(file))
                .unwrap_or_else(|_| panic!("cited file missing: {}", file));
            let lines: Vec<&str> = src.lines().collect();
            assert!(
                (line as usize) >= 1 && (line as usize) <= lines.len(),
                "{} {}:{} out of range ({} lines)",
                id,
                file,
                line,
                lines.len()
            );
            let content = lines[line as usize - 1];
            assert!(
                content.contains(anchor),
                "{} {}:{} content drift — expected anchor {:?}, got {:?}",
                id,
                file,
                line,
                anchor,
                content.trim()
            );
            covered += 1;
        }
    }
    assert_eq!(
        covered,
        ANCHORS.len(),
        "anchor table covers {} citations but {} found",
        ANCHORS.len(),
        covered
    );
}

#[test]
fn spec_gap_v_battery() {
    v01_search_no_match();
    v02_pl_unavailable();
    v03_closed_command_set();
    v04_supersede_lifecycle();
    v05_three_field_separation();
    v06_dangling_ref_actor_stamp();
    nih9_reverify();
}
