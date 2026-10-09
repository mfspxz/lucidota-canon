//! Shared semantic-anchor table for canon enforcement-citation tests.
//!
//! Included via `#[path = "common/mod.rs"] mod common;` from each integration
//! test binary (each tests/*.rs file compiles as its own crate, so this
//! module is the single canonical copy — no drift between duplicates).
//!
//! Technique (wave-25, `verify_gaps.rs` NIH-FIND-9 re-verification): every
//! `invariants.json` enforcement citation `(id, file, line)` is bound to a
//! stable code-content marker. The test asserts the cited line's CONTENT
//! contains the anchor substring — so a citation that drifts to an unrelated
//! line (or to a line whose content changed) fails the build, instead of
//! silently passing a mere line-range check.

use std::fs;
use std::path::Path;

/// (invariant id, file, 1-based line, anchor substring that must appear on
/// the cited line). Anchors verified against the tree at write time; drift
/// fails the build.
pub const ANCHORS: &[(&str, &str, u32, &str)] = &[
    ("INV-01", "src/main.rs", 705, "usage: canon"),
    ("INV-01", "src/main.rs", 725, "\"context\" | \"audit\""),
    ("INV-01", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-02", "src/main.rs", 462, "NO ACTIVE DECISION FOUND"),
    (
        "INV-02",
        "src/context.rs",
        252,
        "PL state UNKNOWN, not assumed",
    ),
    ("INV-03", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-03", "src/store.rs", 169, "pub fn validate_new"),
    ("INV-03", "src/main.rs", 58, "fn reject_unknown"),
    (
        "INV-04",
        "src/context.rs",
        252,
        "PL state UNKNOWN, not assumed",
    ),
    ("INV-05", "src/store.rs", 67, "pub fn read_strict"),
    ("INV-05", "src/store.rs", 298, "pub fn verify_chain"),
    ("INV-06", "src/store.rs", 221, "pub fn append"),
    ("INV-06", "src/main.rs", 705, "usage: canon"),
    ("INV-06", "src/store.rs", 228, "duplicate-id guard"),
    ("INV-07", "src/main.rs", 371, "fn cmd_supersede"),
    ("INV-07", "src/main.rs", 386, "both endpoints must exist"),
    ("INV-08", "src/context.rs", 161, "revision:"),
    ("INV-08", "src/context.rs", 162, "working_tree:"),
    ("INV-09", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-10", "src/store.rs", 134, "pub fn allowed_fields"),
    ("INV-10", "src/main.rs", 705, "usage: canon"),
    (
        "INV-11",
        "src/context.rs",
        280,
        "fn receipt_deterministic_given_inputs",
    ),
    ("INV-11", "Cargo.toml", 9, "sha2"),
    ("INV-12", "src/store.rs", 265, "actor"),
    (
        "INV-12",
        "src/main.rs",
        216,
        "dangling constitution ref {:?}",
    ),
    ("INV-12", "src/main.rs", 237, "dangling decision ref {:?}"),
];

/// Look up the anchor substring for one enforcement citation.
pub fn anchor_for(id: &str, file: &str, line: u32) -> Option<&'static str> {
    ANCHORS
        .iter()
        .find(|(a, f, l, _)| *a == id && *f == file && *l == line)
        .map(|(_, _, _, anchor)| *anchor)
}

/// Check that `root/file`'s 1-based `line` exists and contains `anchor`.
/// Returns Err with a diagnostic instead of panicking, so callers can write
/// non-vacuous negative tests against it.
pub fn check_anchor(
    root: &Path,
    id: &str,
    file: &str,
    line: u32,
    anchor: &str,
) -> Result<(), String> {
    let path = root.join(file);
    let src = fs::read_to_string(&path)
        .map_err(|e| format!("{}: cited file {:?} unreadable: {}", id, file, e))?;
    let lines: Vec<&str> = src.lines().collect();
    if line < 1 || (line as usize) > lines.len() {
        return Err(format!(
            "{}: {:?}:{} out of range (file has {} lines)",
            id,
            file,
            line,
            lines.len()
        ));
    }
    let content = lines[line as usize - 1];
    if !content.contains(anchor) {
        return Err(format!(
            "{}: {:?}:{} content drift — expected anchor {:?}, got {:?}",
            id,
            file,
            line,
            anchor,
            content.trim()
        ));
    }
    Ok(())
}

/// Assert one citation resolves to a real line whose content matches its
/// semantic anchor. Panics with a precise diagnostic on any failure —
/// including the case where invariants.json cites a point with NO anchor
/// in the table (which forces the table to stay in sync with the JSON).
pub fn assert_anchor(root: &Path, id: &str, file: &str, line: u32) {
    let anchor = anchor_for(id, file, line).unwrap_or_else(|| {
        panic!(
            "{}: no semantic anchor for {}:{} — add one to tests/common/mod.rs \
             (every enforcement citation must be content-bound, not line-range-only)",
            id, file, line
        )
    });
    if let Err(e) = check_anchor(root, id, file, line, anchor) {
        panic!("{}", e);
    }
}
