//! canon — Canonical Engineering Authority (RFC-002).
//! CANON records what a project must be / was decided / was superseded.
//! PL records what was observed and verified. Neither impersonates the other.
//! Exit codes: 0 ok, 2 usage, 3 integrity failure, 4 semantic failure.
use canon::{context, store, time};

mod bm25;

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

/// Fields matched by `canon search`. Includes schema-2 `statement`
/// (`statement=` wins over legacy `text=` per cmd_add_principle), so
/// schema-2 content is discoverable (SEARCH-STATEMENT-2026-10-08).
const SEARCH_FIELDS: &[&str] = &[
    "id", "text", "statement", "title", "subject", "decision", "from", "to", "reason",
];

/// Parse a list-valued field: JSON array string → array; anything else →
/// single-element array. Keeps alternatives/errors machine-readable.
fn as_list(v: &str) -> Value {
    let t = v.trim();
    if t.starts_with('[') {
        if let Ok(Value::Array(a)) = serde_json::from_str(t) {
            return Value::Array(a);
        }
    }
    json!([v])
}

/// Reject unknown k=v arguments: never silently dropped, never smuggled.
/// Unknown arguments are a usage error (exit 2); unknown FIELDS at the
/// store API level are a semantic error (exit 4) via validate_new.
fn reject_unknown(kv: &BTreeMap<String, String>, known: &[&str]) -> Result<(), ExitCode> {
    let unknown: Vec<&str> = kv
        .keys()
        .filter(|k| !known.contains(&k.as_str()))
        .map(|s| s.as_str())
        .collect();
    if !unknown.is_empty() {
        eprintln!(
            "unknown argument(s): {} — rejected, never silently dropped",
            unknown.join(", ")
        );
        return Err(ExitCode::from(2));
    }
    Ok(())
}

fn cmd_add_principle(kv: &BTreeMap<String, String>) -> ExitCode {
    if let Err(e) = reject_unknown(
        kv,
        &[
            "id", "statement", "text", "scope", "authority", "enforcement_point",
            "violation_behavior", "evidence_method", "severity", "status",
            "rationale", "rfc_ref", "supersedes",
        ],
    ) {
        return e;
    }
    // §56: ID/STATEMENT/SCOPE/AUTHORITY/ENFORCEMENT POINT/VIOLATION
    // BEHAVIOR/EVIDENCE METHOD. `text=` is a legacy alias for `statement=`
    // (schema-1 seeds); `statement=` wins.
    let id = kv.get("id").cloned().unwrap_or_default();
    let statement = kv
        .get("statement")
        .or_else(|| kv.get("text"))
        .cloned()
        .unwrap_or_default();
    let scope = kv.get("scope").cloned().unwrap_or_default();
    let authority = kv.get("authority").cloned().unwrap_or_default();
    let enforcement_point = kv.get("enforcement_point").cloned().unwrap_or_default();
    let violation_behavior = kv.get("violation_behavior").cloned().unwrap_or_default();
    let evidence_method = kv.get("evidence_method").cloned().unwrap_or_default();
    let severity = kv.get("severity").cloned().unwrap_or_default();
    let missing: Vec<&str> = [
        ("id", &id),
        ("statement", &statement),
        ("scope", &scope),
        ("authority", &authority),
        ("enforcement_point", &enforcement_point),
        ("violation_behavior", &violation_behavior),
        ("evidence_method", &evidence_method),
        ("severity", &severity),
    ]
    .iter()
    .filter(|(_, v)| v.is_empty())
    .map(|(k, _)| *k)
    .collect();
    if !missing.is_empty() {
        eprintln!(
            "add-principle requires id= statement= scope= authority= enforcement_point= violation_behavior= evidence_method= severity=advisory|required|non_negotiable; missing: {}",
            missing.join(", ")
        );
        return ExitCode::from(2);
    }
    let mut m = Map::new();
    m.insert("kind".into(), json!("principle"));
    m.insert("id".into(), json!(id.clone()));
    m.insert("statement".into(), json!(statement));
    m.insert("scope".into(), json!(scope));
    m.insert("authority".into(), json!(authority));
    m.insert("enforcement_point".into(), json!(enforcement_point));
    m.insert("violation_behavior".into(), json!(violation_behavior));
    m.insert("evidence_method".into(), json!(evidence_method));
    m.insert("severity".into(), json!(severity));
    m.insert("status".into(), json!(kv.get("status").cloned().unwrap_or("active".into())));
    if let Some(r) = kv.get("rationale") {
        m.insert("rationale".into(), json!(r));
    }
    if let Some(r) = kv.get("rfc_ref") {
        m.insert("rfc_ref".into(), json!(r));
    }
    if let Some(s) = kv.get("supersedes") {
        m.insert("supersedes".into(), json!(s));
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
    if let Err(e) = reject_unknown(
        kv,
        &[
            "id", "subject", "text", "inputs", "outputs", "errors",
            "authorization", "side_effects", "determinism", "version",
            "evidence_requirements", "constitution", "decisions", "status",
        ],
    ) {
        return e;
    }
    // §56/A3: every externally meaningful interface gets input/output/
    // errors/authorization/side effects/determinism/version/evidence
    // requirements — machine-readable, not prose.
    let get = |k: &str| kv.get(k).cloned().unwrap_or_default();
    let id = get("id");
    let subject = get("subject");
    let text = get("text");
    let inputs = get("inputs");
    let outputs = get("outputs");
    let errors = get("errors");
    let authorization = get("authorization");
    let side_effects = get("side_effects");
    let determinism = get("determinism");
    let version = get("version");
    let evidence_requirements = get("evidence_requirements");
    let missing: Vec<&str> = [
        ("id", &id),
        ("subject", &subject),
        ("text", &text),
        ("inputs", &inputs),
        ("outputs", &outputs),
        ("errors", &errors),
        ("authorization", &authorization),
        ("side_effects", &side_effects),
        ("determinism", &determinism),
        ("version", &version),
        ("evidence_requirements", &evidence_requirements),
    ]
    .iter()
    .filter(|(_, v)| v.is_empty())
    .map(|(k, _)| *k)
    .collect();
    if !missing.is_empty() {
        eprintln!(
            "add-contract requires id= subject= text= inputs= outputs= errors= authorization= side_effects= determinism= version= evidence_requirements=; missing: {}",
            missing.join(", ")
        );
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
    // no dangling decision refs either
    let drefs: Vec<String> = kv
        .get("decisions")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    if !drefs.is_empty() {
        match store::read_strict("decisions") {
            Ok(decisions) => {
                let idx = store::index(&decisions);
                for r in &drefs {
                    if !idx.contains_key(r) {
                        eprintln!("dangling decision ref {:?} — no such decision", r);
                        return ExitCode::from(4);
                    }
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::from(3);
            }
        }
    }
    let mut m = Map::new();
    m.insert("kind".into(), json!("contract"));
    m.insert("id".into(), json!(id.clone()));
    m.insert("subject".into(), json!(subject));
    m.insert("text".into(), json!(text));
    m.insert("inputs".into(), json!(inputs));
    m.insert("outputs".into(), json!(outputs));
    m.insert("errors".into(), as_list(&errors));
    m.insert("authorization".into(), json!(authorization));
    m.insert("side_effects".into(), json!(side_effects));
    m.insert("determinism".into(), json!(determinism));
    m.insert("version".into(), json!(version));
    m.insert("evidence_requirements".into(), json!(evidence_requirements));
    m.insert("status".into(), json!(kv.get("status").cloned().unwrap_or("active".into())));
    if !refs.is_empty() {
        m.insert("constitution".into(), json!(refs));
    }
    if !drefs.is_empty() {
        m.insert("decisions".into(), json!(drefs));
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
    if let Err(e) = reject_unknown(
        kv,
        &[
            "id", "question", "title", "decision", "alternatives", "reason",
            "rationale", "evidence", "author", "status", "supersedes",
            "rejected_alternative",
        ],
    ) {
        return e;
    }
    // §56/A4: decision_id/question/decision/alternatives/reason/evidence/
    // author/timestamp. `rationale=` accepted as legacy alias for `reason=`;
    // `rejected_alternative=` folds into the alternatives list.
    let id = kv.get("id").cloned().unwrap_or_default();
    let question = kv.get("question").cloned().unwrap_or_default();
    let decision = kv.get("decision").cloned().unwrap_or_default();
    let reason = kv
        .get("reason")
        .or_else(|| kv.get("rationale"))
        .cloned()
        .unwrap_or_default();
    let evidence = kv.get("evidence").cloned().unwrap_or_default();
    let alt_raw = kv.get("alternatives").cloned().unwrap_or_default();
    let missing: Vec<&str> = [
        ("id", &id),
        ("question", &question),
        ("decision", &decision),
        ("alternatives", &alt_raw),
        ("reason", &reason),
        ("evidence", &evidence),
    ]
    .iter()
    .filter(|(_, v)| v.is_empty())
    .map(|(k, _)| *k)
    .collect();
    if !missing.is_empty() {
        eprintln!(
            "add-decision requires id= question= decision= alternatives= reason= evidence= [title= author= supersedes=]; missing: {}",
            missing.join(", ")
        );
        return ExitCode::from(2);
    }
    let mut alts: Vec<Value> = match as_list(&alt_raw) {
        Value::Array(a) => a,
        v => vec![v],
    };
    if let Some(r) = kv.get("rejected_alternative") {
        let s = json!(r);
        if !alts.contains(&s) {
            alts.push(s);
        }
    }
    let mut m = Map::new();
    m.insert("kind".into(), json!("decision"));
    m.insert("id".into(), json!(id.clone()));
    m.insert("question".into(), json!(question));
    m.insert("decision".into(), json!(decision));
    m.insert("alternatives".into(), Value::Array(alts));
    m.insert("reason".into(), json!(reason));
    m.insert("evidence".into(), json!(evidence));
    m.insert("status".into(), json!(kv.get("status").cloned().unwrap_or("active".into())));
    if let Some(t) = kv.get("title") {
        m.insert("title".into(), json!(t));
    }
    if let Some(s) = kv.get("supersedes") {
        m.insert("supersedes".into(), json!(s));
    }
    // author is explicit (§56); timestamp comes from created_at (UTC ISO).
    m.insert(
        "author".into(),
        json!(kv.get("author").cloned().unwrap_or("lucidota".into())),
    );
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
    if let Err(e) = reject_unknown(kv, &["from", "to", "reason"]) {
        return e;
    }
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
    // Depenaudit U-1: BM25 ranking replaces the O(n·m) substring scan.
    // Collect (kind, entry, tokens) across kinds, rank once, print by score.
    let mut docs: Vec<(String, Map<String, Value>, Vec<String>)> = Vec::new();
    for (kind, _) in store::KINDS {
        let entries = match store::read_strict(kind) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("{}", e);
                return ExitCode::from(3);
            }
        };
        for e in entries {
            let text: String = SEARCH_FIELDS
                .iter()
                .map(|k| strf(&e, k))
                .collect::<Vec<_>>()
                .join(" ");
            docs.push((kind.to_string(), e, bm25::tokenize(&text)));
        }
    }
    let sup = match store::read_strict("supersessions") {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(3);
        }
    };
    let token_docs: Vec<Vec<String>> = docs.iter().map(|(_, _, t)| t.clone()).collect();
    let ranked = bm25::rank(query, &token_docs);
    if ranked.is_empty() {
        println!("NO ACTIVE DECISION FOUND — no canonical record matches {:?}", query);
        return ExitCode::from(0);
    }
    for (idx, score) in ranked {
        let (kind, e, _) = &docs[idx];
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
        // C12: status is always surfaced — search never flattens it away.
        // Schema-2 `statement` is preferred for the snippet (statement=
        // wins over legacy text=).
        let snippet_src = [
            strf(e, "statement"),
            strf(e, "text"),
            strf(e, "title"),
            strf(e, "decision"),
            strf(e, "reason"),
        ]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default();
        let snippet: String = snippet_src.chars().take(100).collect();
        // C12: status is always surfaced — search never flattens it away
        println!("{} {} [{}] bm25={:.3} :: {}", kind.trim_end_matches('s'), label, status, score, snippet);
    }
    ExitCode::from(0)
}

fn cmd_context(kv: &BTreeMap<String, String>) -> ExitCode {
    if let Err(e) = reject_unknown(kv, &["project"]) {
        return e;
    }
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

fn cmd_snapshot() -> ExitCode {
    // Cross-process consumable interface: verified normative-state snapshot
    // as canonical JSON. Fails closed (exit 3) on any chain break — a
    // consumer must never derive authority from an unverified snapshot.
    match canon::api::Canon::open() {
        Ok(c) => match c.snapshot() {
            Ok(v) => match canon::cjson::canonical(&v) {
                Ok(s) => {
                    println!("{}", s);
                    ExitCode::from(0)
                }
                Err(e) => {
                    eprintln!("snapshot serialize: {}", e);
                    ExitCode::from(3)
                }
            },
            Err(e) => {
                eprintln!("snapshot: {}", e);
                ExitCode::from(3)
            }
        },
        Err(e) => {
            eprintln!("snapshot: {}", e);
            ExitCode::from(3)
        }
    }
}

fn cmd_deploy_identity(kv: &BTreeMap<String, String>, rest: &[String]) -> ExitCode {
    // canon deploy-identity gen-key
    // canon deploy-identity name=<deployment> key=<secret-key-hex-file>
    if rest.first().map(|s| s.as_str()) == Some("gen-key") {
        if let Err(e) = reject_unknown(kv, &[]) {
            return e;
        }
        use ed25519_dalek::SigningKey;
        use rand_core::OsRng;
        let sk = SigningKey::generate(&mut OsRng);
        let vk = sk.verifying_key();
        println!("TEST-KEY — NOT FOR PRODUCTION. Ephemeral only; never use for production.");
        println!(
            "secret={}",
            sk.to_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>()
        );
        println!(
            "public={}",
            vk.to_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>()
        );
        return ExitCode::from(0);
    }
    if let Err(e) = reject_unknown(kv, &["name", "key"]) {
        return e;
    }
    let name = kv.get("name").cloned().unwrap_or_default();
    let key_path = kv.get("key").cloned().unwrap_or_default();
    if name.is_empty() || key_path.is_empty() {
        eprintln!("deploy-identity requires name=<deployment> key=<secret-key-hex-file>, or: deploy-identity gen-key");
        return ExitCode::from(2);
    }
    let sk = match canon::deploy::load_signing_key(&key_path) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("deploy-identity: {}", e);
            return ExitCode::from(2);
        }
    };
    match canon::deploy::issue_manifest(&name, &sk) {
        Ok(manifest) => {
            println!("{}", manifest);
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("deploy-identity: {}", e);
            ExitCode::from(3)
        }
    }
}

fn cmd_verify_deployment(kv: &BTreeMap<String, String>) -> ExitCode {
    if let Err(e) = reject_unknown(kv, &["manifest"]) {
        return e;
    }
    let path = kv.get("manifest").cloned().unwrap_or_default();
    if path.is_empty() {
        eprintln!("verify-deployment requires manifest=<path>");
        return ExitCode::from(2);
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("verify-deployment: cannot read manifest: {}", e);
            return ExitCode::from(2);
        }
    };
    let report = canon::deploy::verify_manifest(&text);
    for line in &report.detail {
        println!("{}", line);
    }
    match report.verdict {
        canon::deploy::Verdict::Match => {
            println!("verdict: MATCH");
            ExitCode::from(0)
        }
        canon::deploy::Verdict::Mismatch => {
            println!("verdict: MISMATCH");
            ExitCode::from(4)
        }
        canon::deploy::Verdict::Unknown => {
            println!("verdict: UNKNOWN");
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
        eprintln!("usage: canon add-principle|add-contract|add-decision|supersede|search|context|audit|snapshot|deploy-identity|verify-deployment k=v...");
        return ExitCode::from(2);
    }
    let cmd = args[0].as_str();
    match cmd {
        "search" => {
            let q = args.get(1).cloned().unwrap_or_default();
            cmd_search(&q)
        }
        "deploy-identity" => {
            // deploy-identity gen-key | deploy-identity name= key=
            // ("gen-key" is a bare word, not k=v, so split it out of kv.)
            let rest: Vec<String> = args[1..]
                .iter()
                .filter(|a| !a.contains('='))
                .cloned()
                .collect();
            let kv = kv(&args[1..]);
            cmd_deploy_identity(&kv, &rest)
        }
        "context" | "audit" | "snapshot" | "verify-deployment" | "add-principle" | "add-contract" | "add-decision" | "supersede" => {
            let kv = kv(&args[1..]);
            match cmd {
                "context" => cmd_context(&kv),
                "audit" => cmd_audit(),
                "snapshot" => cmd_snapshot(),
                "verify-deployment" => cmd_verify_deployment(&kv),
                "add-principle" => cmd_add_principle(&kv),
                "add-contract" => cmd_add_contract(&kv),
                "add-decision" => cmd_add_decision(&kv),
                "supersede" => cmd_supersede(&kv),
                _ => unreachable!(),
            }
        }
        _ => {
            eprintln!("unknown command. try: add-principle add-contract add-decision supersede search context audit snapshot deploy-identity verify-deployment");
            ExitCode::from(2)
        }
    }
}
