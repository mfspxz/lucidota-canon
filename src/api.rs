//! Consumable library interface (wave-4): CANON as a library, not just a CLI.
//!
//! The frozen §56 stores are READ-ONLY through this API. No mutation surface
//! is added here — writes stay on the CLI append path (`canon add-*`),
//! which keeps the fail-closed validation in exactly one place.
//!
//! Store location follows `store::canon_dir()` (the `CANON_DIR` environment
//! variable, else the default workspace path), so the CLI and every library
//! consumer read the same state through the same code path.
//!
//! [`Canon::open`] verifies all four hash chains before returning: any
//! corruption or broken chain is an error, never a partial view.
use crate::store::{self, StoreError};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

fn strf(e: &Map<String, Value>, k: &str) -> String {
    e.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

/// An opened, chain-verified view of the four CANON stores.
pub struct Canon {
    constitution: Vec<Map<String, Value>>,
    contracts: Vec<Map<String, Value>>,
    decisions: Vec<Map<String, Value>>,
    supersessions: Vec<Map<String, Value>>,
}

impl Canon {
    /// Open the stores at `store::canon_dir()` and verify every hash chain.
    /// Fails closed on any corruption, broken chain, or unreadable store.
    ///
    /// Entries stay in file (chain) order — [`Canon::heads`] depends on it.
    /// [`Canon::active`] returns id-sorted views for deterministic consumers.
    pub fn open() -> Result<Canon, StoreError> {
        let constitution = store::read_strict("constitution")?;
        let contracts = store::read_strict("contracts")?;
        let decisions = store::read_strict("decisions")?;
        let supersessions = store::read_strict("supersessions")?;
        for (kind, entries) in [
            ("constitution", &constitution),
            ("contracts", &contracts),
            ("decisions", &decisions),
            ("supersessions", &supersessions),
        ] {
            store::verify_chain(kind, entries)?;
        }
        Ok(Canon { constitution, contracts, decisions, supersessions })
    }

    fn entries(&self, kind: &str) -> &[Map<String, Value>] {
        match kind {
            "constitution" => &self.constitution,
            "contracts" => &self.contracts,
            "decisions" => &self.decisions,
            "supersessions" => &self.supersessions,
            _ => &[],
        }
    }

    /// Effective status of a record id (SUPERSEDED wins over own status).
    pub fn status_of(&self, id: &str, own: Option<&str>) -> &'static str {
        store::effective_status(id, &self.supersessions, own)
    }

    /// Active records of a kind (constitution/contracts/decisions), sorted by id
    /// for deterministic consumers. Chain order is preserved in the stores;
    /// this view is for reading, not for head computation.
    pub fn active(&self, kind: &str) -> Vec<&Map<String, Value>> {
        let mut out: Vec<&Map<String, Value>> = self
            .entries(kind)
            .iter()
            .filter(|e| {
                let id = strf(e, "id");
                self.status_of(&id, e.get("status").and_then(|v| v.as_str())) == "ACTIVE"
            })
            .collect();
        out.sort_by(|a, b| strf(a, "id").cmp(&strf(b, "id")));
        out
    }

    /// Active schema-2 constitution invariants (the normative core).
    pub fn invariants(&self) -> Vec<&Map<String, Value>> {
        self.active("constitution")
            .into_iter()
            .filter(|e| e.get("schema").and_then(|v| v.as_i64()) == Some(2))
            .collect()
    }

    /// Look up any record by id across constitution/contracts/decisions.
    pub fn by_id(&self, id: &str) -> Option<(&str, &Map<String, Value>)> {
        for kind in ["constitution", "contracts", "decisions"] {
            for e in self.entries(kind) {
                if strf(e, "id") == id {
                    return Some((kind, e));
                }
            }
        }
        None
    }

    /// Head hash of each store's chain (stable normative-state identifier).
    pub fn heads(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for (kind, _) in store::KINDS {
            let h = self
                .entries(kind)
                .last()
                .and_then(|e| e.get("hash"))
                .and_then(|v| v.as_str())
                .unwrap_or(store::GENESIS)
                .to_string();
            out.insert(kind.to_string(), h);
        }
        out
    }

    /// IDs of ACTIVE invariants whose statement mentions any of `keywords`.
    /// Used by consumers (e.g. the court) to cite the normative basis of a
    /// policy they enforce — the invariant text stays authoritative in CANON.
    pub fn normative_basis(&self, keywords: &[&str]) -> Vec<String> {
        let mut ids: Vec<String> = self
            .invariants()
            .iter()
            .filter(|e| {
                let s = strf(e, "statement").to_lowercase();
                keywords.iter().any(|k| s.contains(k))
            })
            .map(|e| strf(e, "id"))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Machine-readable snapshot of the normative state: protocol version,
    /// per-store head hashes, active records, and the normative-basis index.
    /// `context_hash` comes from the deterministic context receipt (§44-45);
    /// it covers git revision, so treat `canon_heads` as the stable identity
    /// and `context_hash` as the full provenance fingerprint.
    pub fn snapshot(&self) -> Result<Value, StoreError> {
        let ctx = crate::context::build("canon").map(|r| r.hash).unwrap_or_default();
        let mut heads = Map::new();
        for (k, v) in self.heads() {
            heads.insert(k, Value::String(v));
        }
        let mut recs = Map::new();
        for kind in ["constitution", "contracts", "decisions"] {
            let arr: Vec<Value> = self
                .active(kind)
                .into_iter()
                .map(|e| Value::Object(e.clone()))
                .collect();
            recs.insert(kind.to_string(), Value::Array(arr));
        }
        let basis = self.normative_basis(&["evidence", "receipt", "proof", "assert"]);
        let mut top = Map::new();
        top.insert("canon_protocol".into(), Value::Number(store::PROTOCOL.into()));
        top.insert("canon_heads".into(), Value::Object(heads));
        top.insert("context_hash".into(), Value::String(ctx));
        top.insert(
            "normative_basis_evidence".into(),
            Value::Array(basis.into_iter().map(Value::String).collect()),
        );
        top.insert("records".into(), Value::Object(recs));
        top.insert(
            "generated_at".into(),
            Value::String(crate::time::now_iso()),
        );
        Ok(Value::Object(top))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_reads_default_dir() {
        // The frozen workspace stores must open and verify clean.
        let c = Canon::open().expect("frozen stores open");
        assert!(!c.heads()["constitution"].is_empty());
        assert_eq!(c.heads().len(), 4);
    }

    #[test]
    fn invariants_are_schema2_active() {
        let c = Canon::open().expect("open");
        let invs = c.invariants();
        assert_eq!(invs.len(), 12, "frozen §56: 12 active INV invariants");
        for e in &invs {
            // schema is a JSON number, not a string — check it directly.
            assert_eq!(e.get("schema").and_then(|v| v.as_i64()), Some(2));
            assert!(strf(e, "id").starts_with("INV-"));
        }
    }

    #[test]
    fn normative_basis_finds_evidence_invariants() {
        let c = Canon::open().expect("open");
        let b = c.normative_basis(&["evidence", "receipt", "proof", "assert"]);
        // INV-01 (assertion ≠ verification), INV-02 (UNKNOWN default),
        // INV-03 (evidence ≠ proof) are the evidence-binding core.
        for id in ["INV-01", "INV-02", "INV-03"] {
            assert!(b.contains(&id.to_string()), "basis should cite {}", id);
        }
    }

    #[test]
    fn snapshot_shape() {
        let c = Canon::open().expect("open");
        let s = c.snapshot().expect("snapshot");
        assert!(s.get("canon_heads").is_some());
        assert!(s.get("records").is_some());
        assert_eq!(s["records"]["constitution"].as_array().unwrap().len(), 12);
    }
}
