//! canon as an organelle (Tier A native migration).
//!
//! One organelle (`ca`), six ops over the Canonical Engineering Authority:
//!   verify    — open all four stores, verify every hash chain (fail-closed)
//!   get       — find a record by id across all kinds
//!   search    — BM25 over SEARCH_FIELDS (mirrors main.rs cmd_search doc-building;
//!               the ranker is the shared `bm25` module, moved into the lib)
//!   active    — id-sorted active records of one kind
//!   add       — fail-closed append (validate_new + dup-id guard + chain + lock)
//!   snapshot  — full verified snapshot
//!
//! Execution personality: fs read-write on the canon store dir
//! ($CANON_DIR else default) — `add` takes the file lock via store::append.
//! No network, no secrets. Stateless: suspend = no-op, kill = drop.
//! Envelope: 1 cpu / 256 MB / 30 s.

use organelle_host::{CallContext, HostRef, Organelle, OrganelleError};
use organelle_manifest::{
    Authority, Contract, MutationClass, OrganelleManifest, ReceiptPolicy, Transport, Valence,
    MANIFEST_SCHEMA_VERSION, RECEIPT_ALGORITHM,
};
use serde_json::{Map, Value};
use std::sync::Arc;

use crate::{api::Canon, bm25, store};
use crate::store::StoreError;

/// Mirrors main.rs SEARCH_FIELDS (the fields `canon search` matches).
const SEARCH_FIELDS: &[&str] = &[
    "id", "text", "statement", "title", "subject", "decision", "from", "to", "reason",
];

fn strf(e: &Map<String, Value>, k: &str) -> String {
    e.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn str_in(v: &Value, key: &str) -> Result<String, OrganelleError> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| OrganelleError::failed(format!("missing string field {key:?}")))
}

fn store_err(e: StoreError) -> OrganelleError {
    OrganelleError::failed(format!("canon store: {e:?}"))
}

fn open_canon() -> Result<Canon, OrganelleError> {
    Canon::open().map_err(store_err)
}

fn contract(op: &str, description: &str, schema: Value) -> Contract {
    Contract { op: op.to_string(), description: description.to_string(), input_schema: schema, mutation_class: None, required_capability: None }
}

fn contract_gated(op: &str, description: &str, schema: Value, cap: &str) -> Contract {
    Contract { op: op.to_string(), description: description.to_string(), input_schema: schema, mutation_class: Some(MutationClass::Write), required_capability: Some(cap.to_string()) }
}

fn obj_schema(props: Value, required: &[&str]) -> Value {
    serde_json::json!({ "type": "object", "properties": props, "required": required })
}

fn kind_schema() -> Value {
    serde_json::json!({"type": "string", "enum": ["constitution", "contracts", "decisions", "supersessions"]})
}

fn manifest() -> OrganelleManifest {
    OrganelleManifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
        name: "canon".to_string(),
        codon: "ca".to_string(),
        version: "0.1.0".to_string(),
        description: "Canonical Engineering Authority: four hash-chained stores, chain-verified reads, BM25 search, fail-closed writes".to_string(),
        contracts: vec![
            contract("verify", "open all four stores, verify every hash chain, return heads + counts",
                obj_schema(serde_json::json!({}), &[])),
            contract("get", "find a record by id across all four stores",
                obj_schema(serde_json::json!({"id": {"type": "string"}}), &["id"])),
            contract("search", "BM25 search across all stores",
                obj_schema(serde_json::json!({"query": {"type": "string"}, "top_n": {"type": "integer"}}), &["query"])),
            contract("active", "id-sorted active (non-superseded) records of one kind",
                obj_schema(serde_json::json!({"kind": kind_schema()}), &["kind"])),
            contract_gated("add", "fail-closed append: validate_new + dup-id guard + chain + file lock",
                obj_schema(serde_json::json!({"kind": kind_schema(), "entry": {"type": "object"}}), &["kind", "entry"]),
                "ca.write"),
            contract("snapshot", "full verified snapshot of all four stores",
                obj_schema(serde_json::json!({}), &[])),
        ],
        authority: Authority {
            mutation_class: MutationClass::ReadOnly,
            declared_valence: Valence::Proven,
            required_capability: None,
            granted_capabilities: vec![],
        },
        transports: Transport::all().to_vec(),
        receipt_policy: ReceiptPolicy { content_addressed: true, algorithm: RECEIPT_ALGORITHM.to_string(), chain: true },
    }
}

pub struct CanonOrganelle {
    manifest: OrganelleManifest,
}

pub fn organelle() -> Arc<dyn Organelle> {
    Arc::new(CanonOrganelle { manifest: manifest() })
}

impl Organelle for CanonOrganelle {
    fn manifest(&self) -> OrganelleManifest {
        self.manifest.clone()
    }

    fn handle(&self, _ctx: &CallContext, _host: &dyn HostRef, op: &str, input: Value) -> Result<Value, OrganelleError> {
        match op {
            "verify" => {
                let c = open_canon()?;
                let mut counts = serde_json::Map::new();
                for (kind, _) in store::KINDS {
                    let n = store::read_strict(kind).map_err(store_err)?.len();
                    counts.insert(kind.to_string(), Value::from(n as u64));
                }
                Ok(serde_json::json!({"verified": true, "heads": c.heads(), "counts": counts}))
            }
            "get" => {
                let c = open_canon()?;
                let id = str_in(&input, "id")?;
                match c.by_id(&id) {
                    Some((kind, rec)) => Ok(serde_json::json!({"found": true, "kind": kind, "record": rec})),
                    None => Ok(serde_json::json!({"found": false, "id": id})),
                }
            }
            "search" => {
                let query = str_in(&input, "query")?;
                let top_n = input.get("top_n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let mut docs: Vec<(String, Map<String, Value>, Vec<String>)> = Vec::new();
                for (kind, _) in store::KINDS {
                    for e in store::read_strict(kind).map_err(store_err)? {
                        let text = SEARCH_FIELDS.iter().map(|k| strf(&e, k)).collect::<Vec<_>>().join(" ");
                        docs.push((kind.to_string(), e, bm25::tokenize(&text)));
                    }
                }
                let token_docs: Vec<Vec<String>> = docs.iter().map(|(_, _, t)| t.clone()).collect();
                let ranked = bm25::rank(&query, &token_docs);
                let hits: Vec<Value> = ranked.into_iter().take(top_n).map(|(idx, score)| {
                    let (kind, e, _) = &docs[idx];
                    serde_json::json!({"kind": kind, "id": strf(e, "id"), "score": score, "record": e})
                }).collect();
                Ok(serde_json::json!({"query": query, "hits": hits}))
            }
            "active" => {
                let c = open_canon()?;
                let kind = str_in(&input, "kind")?;
                if !store::KINDS.iter().any(|(k, _)| *k == kind) {
                    return Err(OrganelleError::failed(format!("unknown kind {kind:?}")));
                }
                let recs: Vec<&Map<String, Value>> = c.active(&kind);
                Ok(serde_json::to_value(recs).map_err(|e| OrganelleError::failed(e.to_string()))?)
            }
            "add" => {
                let kind = str_in(&input, "kind")?;
                let entry = input.get("entry").and_then(|v| v.as_object()).cloned()
                    .ok_or_else(|| OrganelleError::failed("entry must be an object"))?;
                store::append(&kind, entry).map_err(store_err)?;
                Ok(serde_json::json!({"appended": true, "kind": kind}))
            }
            "snapshot" => {
                let c = open_canon()?;
                c.snapshot().map_err(store_err)
            }
            _ => Err(OrganelleError::not_found(format!("unknown op {op}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use organelle_host::Host;
    use organelle_manifest::validate_manifest_json;
    use std::sync::OnceLock;

    static TEST_DIR: OnceLock<tempfile::TempDir> = OnceLock::new();

    fn use_test_store() {
        let dir = TEST_DIR.get_or_init(|| tempfile::tempdir().expect("tempdir"));
        // SAFETY: tests in this module run in one thread context for env setup;
        // the dir is initialized once before any canon call.
        unsafe { std::env::set_var("CANON_DIR", dir.path()) };
    }

    fn manifest_json() -> Value {
        let txt = include_str!("../organelles/canon.manifest.json");
        serde_json::from_str(txt).expect("manifest JSON parses")
    }

    #[test]
    fn static_manifest_validates() {
        let m = validate_manifest_json(&manifest_json()).expect("static manifest must validate");
        assert_eq!(m.name, "canon");
        assert_eq!(m.codon, "ca");
    }

    #[test]
    fn rust_manifest_matches_static_json() {
        let from_rust = serde_json::to_value(manifest()).unwrap();
        assert_eq!(from_rust, manifest_json(), "Rust manifest() and organelles/*.json must agree");
    }

    #[test]
    fn registers_and_serves_through_host_with_receipts() {
        use_test_store();
        let host = Host::new();
        host.register(organelle()).expect("registration must succeed");
        let ctx = CallContext::new("ca-test", vec!["ca.write".to_string()]);

        // verify against an empty store: chains trivially hold
        let v = host.call(&ctx, "ca", "verify", serde_json::json!({})).expect("verify");
        assert_eq!(v.output["verified"], true);

        // fail-closed write then read-back (real store::append path, schema-2 valid)
        let entry = serde_json::json!({
            "kind": "decision", "id": "TEST-1",
            "question": "does the organelle serve", "decision": "yes",
            "alternatives": ["no"], "reason": "tests green",
            "evidence": ["cargo test"], "status": "active",
        });
        // If validation rejects, the test surfaces the real schema demand.
        let add_res = host.call(&ctx, "ca", "add",
            serde_json::json!({"kind": "decisions", "entry": entry}));
        match add_res {
            Ok(r) => {
                assert_eq!(r.output["appended"], true);
                let g = host.call(&ctx, "ca", "get", serde_json::json!({"id": "TEST-1"})).expect("get");
                assert_eq!(g.output["found"], true);
                assert_eq!(g.output["kind"], "decisions");
                let s = host.call(&ctx, "ca", "search", serde_json::json!({"query": "green"})).expect("search");
                assert!(s.output["hits"].as_array().unwrap().len() >= 1);
            }
            Err(e) => {
                // schema-2 validation is strict; record the real demand, don't fake it
                panic!("add rejected by real validation: {e}");
            }
        }

        // unknown id -> clean miss, still receipted
        let miss = host.call(&ctx, "ca", "get", serde_json::json!({"id": "NOPE"})).expect("miss");
        assert_eq!(miss.output["found"], false);

        let n = host.verify_chain().expect("receipt chain must verify");
        assert!(n >= 5, "expected >=5 receipts, got {n}");
    }
}
