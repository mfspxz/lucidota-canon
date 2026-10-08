//! canon — Canonical Engineering Authority (RFC-002).
//! CANON records what a project must be / was decided / was superseded.
//! PL records what was observed and verified. Neither impersonates the other.
//! Exit codes: 0 ok, 2 usage, 3 integrity failure, 4 semantic failure.
mod cjson;
mod context;
mod store;
mod time;

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::process::ExitCode;
use store::StoreError;

fn kv(args: &[String]) -> BTreeMap<String, String> {
    args.iter()
        .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

fn strf(e: &Map<String, Value>, k: &str) -> String {
    e.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

/// Find a record by id across all kinds. Returns (kind, record).
fn find_id(id: &str) -> Result<Option<(String, Map<String, Value>)>, StoreError> {
    for (kind, _) in store::KINDS {
        for e in store::read_strict(kind)? {
            if e.get("id").and_then(|v| v.as_str()) == Some(id) {
                return Ok(Some((kind.to_string(), e)));
            }
        }
    }
    Ok(None)
}

fn cmd_add_principle(kv: &BTreeMap<String, String>) -> ExitCode {
    let id = kv.get("id").cloned().unwrap_or_default();
    let text = kv.get("text").cloned().unwrap_or_default();
    let severity = kv.get("severity").cloned().unwrap_or_default();
    if id.is_empty() || text.is_empty() {
        eprintln!("add-principle requires id= text= severity=advisory|required|non_negotiable");
        return ExitCode::from(2);
    }
    if !["advisory", "required", "non_negotiable"].contains(&severity.as_str()) {
        eprintln!("severity must be advisory|required|non_negotiable");
        return ExitCode::from(2);
    }
    let mut m = Map::new();
    m.insert("kind".into(), json!("principle"));
    m.insert("id".into(), json!(id.clone()));
    m.insert("text".into(), json!(text));
    m.insert("severity".into(), json!(severity));
    m.insert("status".into(), json!(kv.get("status").cloned().unwrap_or("active".into())));
    if let Some(r) = kv.get("rationale") {
        m.insert("rationale".into(), json!(r));
    }
    match store::append("constitution", m) {
        Ok(()) => {
            println!("principle {} recorded", id);
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(match e {
                StoreError::Semantic(_) => 4,
                _ => 3,
            })
        }
    }
}

fn cmd_add_contract(kv: &BTreeMap<String, String>) -> ExitCode {
    let id = kv.get("id").cloned().unwrap_or_default();
    let subject = kv.get("subject").cloned().unwrap_or_default();
    let text = kv.get("text").cloned().unwrap_or_default();
    if id.is_empty() || subject.is_empty() || text.is_empty() {
        eprintln!("add-contract requires id= subject= text=");
        return ExitCode::from(2);
    }
    // no dangling constitution refs (C1: no implicit authority)
    let refs: Vec<String> = kv
        .get("constitution")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    match store::read_strict("constitution") {
        Ok(principles) => {
            let idx = store::index(&principles);
            for r in &refs {
                if !idx.contains_key(r) {
                    eprintln!("dangling constitution ref {:?} — no such principle", r);
                    return ExitCode::from(4);
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(3);
        }
    }
    let mut m = Map::new();
    m.insert("kind".into(), json!("contract"));
    m.insert("id".into(), json!(id.clone()));
    m.insert("subject".into(), json!(subject));
    m.insert("text".into(), json!(text));
    m.insert("status".into(), json!(kv.get("status").cloned().unwrap_or("active".into())));
    if !refs.is_empty() {
        m.insert("constitution".into(), json!(refs));
    }
    match store::append("contracts", m) {
        Ok(()) => {
            println!("contract {} recorded", id);
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(match e {
                StoreError::Semantic(_) => 4,
                _ => 3,
            })
        }
    }
}

fn cmd_add_decision(kv: &BTreeMap<String, String>) -> ExitCode {
    let id = kv.get("id").cloned().unwrap_or_default();
    let title = kv.get("title").cloned().unwrap_or_default();
    let decision = kv.get("decision").cloned().unwrap_or_default();
    let rationale = kv.get("rationale").cloned().unwrap_or_default();
    if id.is_empty() || title.is_empty() || decision.is_empty() || rationale.is_empty() {
        eprintln!("add-decision requires id= title= decision= rationale=");
        return ExitCode::from(2);
    }
    let mut m = Map::new();
    m.insert("kind".into(), json!("decision"));
    m.insert("id".into(), json!(id.clone()));
    m.insert("title".into(), json!(title));
    m.insert("decision".into(), json!(decision));
    m.insert("rationale".into(), json!(rationale));
    m.insert("status".into(), json!(kv.get("status").cloned().unwrap_or("active".into())));
    if let Some(r) = kv.get("rejected_alternative") {
        m.insert("rejected_alternative".into(), json!(r));
    }
    match store::append("decisions", m) {
        Ok(()) => {
            println!("decision {} recorded", id);
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(match e {
                StoreError::Semantic(_) => 4,
                _ => 3,
            })
        }
    }
}

fn cmd_supersede(kv: &BTreeMap<String, String>) -> ExitCode {
    let from = kv.get("from").cloned().unwrap_or_default();
    let to = kv.get("to").cloned().unwrap_or_default();
    let reason = kv.get("reason").cloned().unwrap_or_default();
    if from.is_empty() || to.is_empty() || reason.is_empty() {
        eprintln!("supersede requires from= to= reason=");
        return ExitCode::from(2);
    }
    if from == to {
        eprintln!("cannot supersede a record with itself");
        return ExitCode::from(2);
    }
    // both endpoints must exist — no dangling refs
    let (f_kind, _) = match find_id(&from) {
        Ok(Some(x)) => x,
        Ok(None) => {
            eprintln!("supersede: {:?} does not exist", from);
            return ExitCode::from(4);
        }
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(3);
        }
    };
    let (t_kind, _) = match find_id(&to) {
        Ok(Some(x)) => x,
        Ok(None) => {
            eprintln!("supersede: {:?} does not exist", to);
            return ExitCode::from(4);
        }
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(3);
        }
    };
    let _ = (f_kind, t_kind);
    let mut m = Map::new();
    m.insert("kind".into(), json!("supersession"));
    m.insert("from".into(), json!(from.clone()));
    m.insert("to".into(), json!(to.clone()));
    m.insert("reason".into(), json!(reason));
    match store::append("supersessions", m) {
        Ok(()) => {
            println!("{} superseded by {}", from, to);
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(3)
        }
    }
}

fn cmd_search(query: &str) -> ExitCode {
    if query.is_empty() {
        eprintln!("search requires a query");
        return ExitCode::from(2);
    }
    let q = query.to_lowercase();
    let mut hits = 0;
    for (kind, _) in store::KINDS {
        let entries = match store::read_strict(kind) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::from(3);
            }
        };
        let sup = match store::read_strict("supersessions") {
            Ok(e) => e,
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::from(3);
            }
        };
        for e in &entries {
            // supersession records match on from/to/reason
            let hay: Vec<String> = ["id", "text", "title", "subject", "decision", "from", "to", "reason"]
                .iter()
                .map(|k| strf(e, k))
                .collect();
            if !hay.iter().any(|h| h.to_lowercase().contains(&q)) {
                continue;
            }
            hits += 1;
            let id = strf(e, "id");
            let status = if id.is_empty() {
                "EVENT".to_string()
            } else {
                store::effective_status(&id, &sup, Some(&strf(e, "status"))).to_string()
            };
            let label = if id.is_empty() {
                format!("{} -> {}", strf(e, "from"), strf(e, "to"))
            } else {
                id
            };
            let snippet_src = [strf(e, "text"), strf(e, "title"), strf(e, "decision"), strf(e, "reason")]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();
            let snippet: String = snippet_src.chars().take(100).collect();
            // C12: status is always surfaced — search never flattens it away
            println!("{} {} [{}] :: {}", kind.trim_end_matches('s'), label, status, snippet);
        }
    }
    if hits == 0 {
        println!("NO ACTIVE DECISION FOUND — no canonical record matches {:?}", query);
    }
    ExitCode::from(0)
}

fn cmd_context(kv: &BTreeMap<String, String>) -> ExitCode {
    let project = kv.get("project").cloned().unwrap_or("canon".into());
    match context::build(&project) {
        Ok(r) => {
            print!("{}", r.body);
            println!("context_hash: {}", r.hash);
            // non-authoritative metadata, outside the hashed body (§45)
            println!("generated_at: {} (non-authoritative metadata)", time::now_iso());
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(3)
        }
    }
}

fn cmd_audit() -> ExitCode {
    let mut failures: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut checked = 0;
    let mut all: BTreeMap<&str, Vec<Map<String, Value>>> = BTreeMap::new();
    for (kind, _) in store::KINDS {
        match store::read_strict(kind) {
            Ok(entries) => {
                checked += entries.len();
                match store::verify_chain(kind, &entries) {
                    Ok(_) => {}
                    Err(e) => failures.push(format!("{}", e)),
                }
                // duplicate ids (defense in depth — append() also rejects)
                let idx = store::index(&entries);
                if idx.len() != entries.iter().filter(|e| e.get("id").is_some()).count() {
                    failures.push(format!("{}: duplicate ids", kind));
                }
                all.insert(kind, entries);
            }
            Err(e) => failures.push(format!("{}", e)),
        }
    }
    // supersession endpoints must exist (no dangling refs)
    if failures.is_empty() {
        let mut known: BTreeMap<String, &str> = BTreeMap::new();
        for (kind, entries) in &all {
            for e in entries {
                if let Some(id) = e.get("id").and_then(|v| v.as_str()) {
                    known.insert(id.to_string(), kind);
                }
            }
        }
        for s in &all["supersessions"] {
            for end in ["from", "to"] {
                let v = strf(s, end);
                if !known.contains_key(&v) {
                    failures.push(format!("supersession: {:?} endpoint {:?} does not exist", end, v));
                }
            }
        }
        // contradiction surface (§34): same normalized title, both ACTIVE
        let sup = &all["supersessions"];
        let mut by_title: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for d in &all["decisions"] {
            let id = strf(d, "id");
            if store::effective_status(&id, sup, Some(&strf(d, "status"))) == "ACTIVE" {
                by_title.entry(strf(d, "title").to_lowercase()).or_default().push(id);
            }
        }
        for (title, ids) in &by_title {
            if ids.len() > 1 {
                warnings.push(format!("CANON CONFLICT on title {:?}: {:?}", title, ids));
            }
        }
    }
    for w in &warnings {
        println!("WARN {}", w);
    }
    if failures.is_empty() {
        println!("audit OK: {} records across {} stores", checked, store::KINDS.len());
        ExitCode::from(0)
    } else {
        for f in &failures {
            eprintln!("FAIL {}", f);
        }
        ExitCode::from(if failures.iter().any(|f| f.contains("AUDIT FAIL")) { 3 } else { 4 })
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: canon add-principle|add-contract|add-decision|supersede|search|context|audit k=v...");
        return ExitCode::from(2);
    }
    let cmd = args[0].as_str();
    match cmd {
        "search" => {
            let q = args.get(1).cloned().unwrap_or_default();
            cmd_search(&q)
        }
        "context" | "audit" | "add-principle" | "add-contract" | "add-decision" | "supersede" => {
            let kv = kv(&args[1..]);
            match cmd {
                "context" => cmd_context(&kv),
                "audit" => cmd_audit(),
                "add-principle" => cmd_add_principle(&kv),
                "add-contract" => cmd_add_contract(&kv),
                "add-decision" => cmd_add_decision(&kv),
                "supersede" => cmd_supersede(&kv),
                _ => unreachable!(),
            }
        }
        _ => {
            eprintln!("unknown command. try: add-principle add-contract add-decision supersede search context audit");
            ExitCode::from(2)
        }
    }
}
