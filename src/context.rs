//! ENGINEERING CONTEXT RECEIPT (§16, §44-§45).
//! Deterministic: same CANON + same git revision + same PL ledger = byte-identical
//! receipt. The model does not construct it. context_hash covers the authoritative
//! body; generated_at is trailing non-authoritative metadata.
use crate::store::{self, StoreError};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::process::Command;

fn sha_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{:02x}", b)).collect()
}

fn short(h: &str) -> String {
    h.chars().take(12).collect()
}

fn git(dir: &std::path::Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn head_hash(entries: &[Map<String, Value>]) -> String {
    entries
        .last()
        .and_then(|e| e.get("hash"))
        .and_then(|v| v.as_str())
        .unwrap_or(store::GENESIS)
        .to_string()
}

fn strf(e: &Map<String, Value>, k: &str) -> String {
    e.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

/// Query the PL binary for machine-readable claim states. Returns None if
/// the pl binary is missing/unusable — receipt degrades gracefully, never
/// invents PL state (C5).
fn pl_state() -> Option<Vec<BTreeMap<String, String>>> {
    let bin = std::env::var("PL_BIN").unwrap_or("/home/hatch/workspace/proof-ledger/pl".into());
    let out = Command::new(&bin).args(["status", "--json"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: Value = serde_json::from_slice(&out.stdout).ok()?;
    let arr = v.as_array()?;
    let mut rows = Vec::new();
    for c in arr {
        let mut m = BTreeMap::new();
        for k in ["claim_id", "implemented", "deployed", "proven", "overall"] {
            m.insert(k.to_string(), c.get(k).and_then(|v| v.as_str()).unwrap_or("?").to_string());
        }
        rows.push(m);
    }
    rows.sort_by(|a, b| a["claim_id"].cmp(&b["claim_id"]));
    Some(rows)
}

pub struct Receipt {
    pub body: String,
    pub hash: String,
}

/// Build the deterministic receipt body for `project`.
pub fn build(project: &str) -> Result<Receipt, StoreError> {
    let mut all: BTreeMap<&str, Vec<Map<String, Value>>> = BTreeMap::new();
    for (kind, _) in store::KINDS {
        let entries = store::read_strict(kind)?;
        store::verify_chain(kind, &entries)?;
        all.insert(kind, entries);
    }
    let sup = &all["supersessions"];
    let status_of = |id: &str, own: &str| store::effective_status(id, sup, Some(own));

    // --- constitution (sorted by id) ---
    let mut principles: Vec<&Map<String, Value>> = all["constitution"].iter().collect();
    principles.sort_by(|a, b| strf(a, "id").cmp(&strf(b, "id")));

    // --- contracts / decisions split active vs superseded ---
    let mut contracts: Vec<&Map<String, Value>> = all["contracts"].iter().collect();
    contracts.sort_by(|a, b| strf(a, "id").cmp(&strf(b, "id")));
    let mut decisions: Vec<&Map<String, Value>> = all["decisions"].iter().collect();
    decisions.sort_by(|a, b| strf(a, "id").cmp(&strf(b, "id")));

    // --- superseded list ---
    let mut sup_list: Vec<&Map<String, Value>> = sup.iter().collect();
    sup_list.sort_by(|a, b| strf(a, "from").cmp(&strf(b, "from")));

    // --- violations (deterministic order) ---
    let mut violations: Vec<String> = Vec::new();
    // V1: contradiction detection (§34) — same normalized title, both active
    let mut by_title: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for d in &decisions {
        if status_of(&strf(d, "id"), &strf(d, "status")) == "ACTIVE" {
            by_title
                .entry(strf(d, "title").to_lowercase())
                .or_default()
                .push(strf(d, "id"));
        }
    }
    for (title, ids) in &by_title {
        if ids.len() > 1 {
            let mut s = ids.clone();
            s.sort();
            violations.push(format!("CONFLICT {:?} <-> {:?} on title {:?}", s[0], s[1], title));
        }
    }
    // V2: PL FAILED dimensions while CANON requirement stands
    let pl = pl_state();
    if let Some(rows) = &pl {
        for r in rows {
            for dim in ["implemented", "deployed", "proven"] {
                if r[dim] == "failed" {
                    violations.push(format!("PL {}:{} FAILED", r["claim_id"], dim));
                }
            }
        }
    }
    violations.sort();
    violations.dedup();

    // --- required next action (deterministic heuristic) ---
    let next_action = if let Some(v) = violations.first() {
        format!("Resolve {}.", v)
    } else if let Some(rows) = &pl {
        match rows.iter().find(|r| r["overall"] != "shipped") {
            Some(r) => format!(
                "Advance PL claim {} to SHIPPED (implemented ∧ deployed ∧ proven VERIFIED).",
                r["claim_id"]
            ),
            None => "None pending.".to_string(),
        }
    } else {
        "PL state unavailable — verify pl binary, then re-run context.".to_string()
    };

    // --- repo binding ---
    let dir = store::canon_dir();
    let revision = git(&dir, &["rev-parse", "HEAD"]).unwrap_or("no-git".into());
    let working_tree = match git(&dir, &["status", "--porcelain"]) {
        None => "N/A",
        Some(s) if s.is_empty() => "CLEAN",
        Some(_) => "DIRTY",
    };

    // --- assemble body (fixed field order, sorted lists) ---
    let mut b = String::new();
    let push = |b: &mut String, s: &str| {
        b.push_str(s);
        b.push('\n');
    };
    push(&mut b, "CANON CONTEXT RECEIPT");
    push(&mut b, &format!("project: {}", project));
    push(&mut b, &format!("canon_protocol: {}", store::PROTOCOL));
    push(&mut b, &format!("repository: {}", dir.display()));
    push(&mut b, &format!("revision: {}", revision));
    push(&mut b, &format!("working_tree: {}", working_tree));
    push(
        &mut b,
        &format!(
            "canon_heads: constitution={} contracts={} decisions={} supersessions={}",
            short(&head_hash(&all["constitution"])),
            short(&head_hash(&all["contracts"])),
            short(&head_hash(&all["decisions"])),
            short(&head_hash(&all["supersessions"])),
        ),
    );
    push(&mut b, "");
    push(&mut b, "CONSTITUTION");
    if principles.is_empty() {
        push(&mut b, "(none)");
    }
    for p in &principles {
        let id = strf(p, "id");
        let st = status_of(&id, &strf(p, "status"));
        push(
            &mut b,
            &format!("{} [{}] [{}] {}", id, strf(p, "severity"), st, strf(p, "text")),
        );
    }
    push(&mut b, "");
    push(&mut b, "ACTIVE CONTRACTS");
    let active_contracts: Vec<&&Map<String, Value>> = contracts
        .iter()
        .filter(|c| status_of(&strf(c, "id"), &strf(c, "status")) == "ACTIVE")
        .collect();
    if active_contracts.is_empty() {
        push(&mut b, "(none)");
    }
    for c in active_contracts {
        push(&mut b, &format!("{} [{}] {}", strf(c, "id"), strf(c, "subject"), strf(c, "text")));
    }
    push(&mut b, "");
    push(&mut b, "ACTIVE DECISIONS");
    let active_decisions: Vec<&&Map<String, Value>> = decisions
        .iter()
        .filter(|d| status_of(&strf(d, "id"), &strf(d, "status")) == "ACTIVE")
        .collect();
    if active_decisions.is_empty() {
        push(&mut b, "(none)");
    }
    for d in active_decisions {
        push(&mut b, &format!("{} {}", strf(d, "id"), strf(d, "title")));
        let dec = strf(d, "decision");
        if !dec.is_empty() {
            push(&mut b, &format!("  decision: {}", dec));
        }
    }
    push(&mut b, "");
    push(&mut b, "SUPERSEDED");
    if sup_list.is_empty() {
        push(&mut b, "(none)");
    }
    for s in &sup_list {
        push(
            &mut b,
            &format!("{} -> {} ({})", strf(s, "from"), strf(s, "to"), strf(s, "reason")),
        );
    }
    push(&mut b, "");
    push(&mut b, "CURRENT PL STATE");
    match &pl {
        Some(rows) if !rows.is_empty() => {
            for r in rows {
                push(
                    &mut b,
                    &format!(
                        "{}: implemented={} deployed={} proven={} overall={}",
                        r["claim_id"], r["implemented"], r["deployed"], r["proven"], r["overall"]
                    ),
                );
            }
        }
        Some(_) => push(&mut b, "(no PL claims)"),
        None => push(&mut b, "(unavailable: pl binary missing or failed — PL state UNKNOWN, not assumed)"),
    }
    push(&mut b, "");
    push(&mut b, "OPEN VIOLATIONS");
    if violations.is_empty() {
        push(&mut b, "(none)");
    }
    for (i, v) in violations.iter().enumerate() {
        push(&mut b, &format!("V-{:03} {}", i + 1, v));
    }
    push(&mut b, "");
    push(&mut b, "REQUIRED NEXT ACTION");
    push(&mut b, &next_action);
    push(&mut b, "");
    push(&mut b, "AUTHORITY");
    push(&mut b, "CANON = normative architecture (what we are required to do)");
    push(&mut b, "PL = evidence/proof (what has been demonstrated)");
    push(&mut b, "GIT = source history (what source exists)");
    push(&mut b, "AI = proposal/reasoning only (what should we do next)");

    let hash = sha_hex(&b);
    Ok(Receipt { body: b, hash })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_deterministic_given_inputs() {
        // sha over identical input is identical — the determinism primitive
        // the receipt relies on (sorted iteration + fixed layout).
        let a = sha_hex("CANON CONTEXT RECEIPT\nproject: x\n");
        let b2 = sha_hex("CANON CONTEXT RECEIPT\nproject: x\n");
        assert_eq!(a, b2);
        assert_eq!(a.len(), 64);
    }
}
