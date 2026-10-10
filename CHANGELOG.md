# CHANGELOG — canon

## 2026-10-10 — rigor-audit rollout slice 3/8 (R-1..R-12 applied)

- **`docs/STORE-SCHEMA.md`** (new, R-9): frozen schema declaration for the four
  hash-chained stores — schema 1/2 frozen, status vocabulary frozen as
  `active | superseded | superseded_by_bypass` (one book for claims and
  defeats; a bypassed claim moves to `superseded_by_bypass`, never silently
  to `superseded`).
- **`docs/TEST-POLICY.md`** (new, R-7): property vs change-detection test
  separation; property-test expectations may never be rewritten to match a
  behavior change without adversarial sign-off. Test files labeled.
- **R-3:** out-of-band head copy at
  `~/workspace/dogfood/audit/oob-heads/canon.json` (seq + head_hash per store,
  git HEAD, binary sha256) — a store this repo never writes to. Documented
  honest limitation: same-VM; integrity is UNKNOWN-not-PROVEN without an
  auditor holding the head.
- **R-8:** deployment identity is ed25519-signed (TEST-KEY — NOT FOR
  PRODUCTION); context receipts are hash-bound but unsigned — declared
  honestly-unsigned; their verification power is change-detection against
  the OOB head, not provenance.
- **R-1:** fix-verification claims in this repo (freeze evidence, gap
  closures) are `instance-only` until a dedicated red-team wave signs off the
  class; bypasses are mirrored into the claim log as
  `superseded_by_bypass` — see AUDIT-APPLICATION.md.

## 2026-10-09 — BM25 search, semantic anchors, cjson hardening

- **`src/bm25.rs`** (depenaudit U-1): canon search now BM25-ranked
  (REAL-ALGORITHMS.md §5); replaces the naive substring scorer.
- **`tests/invariants.rs`** (wave-37): semantic-anchor technique — every
  enforcement citation resolves to a real line whose CONTENT matches the
  note, not just a line in range (independent re-verification of NIH-FIND-9).
- **`src/cjson.rs`** (XIMPL-2, wave-34): rejects u64 beyond the i64 interop
  range (fail-closed canonical encoding boundary).
- **Organelle migration** (tier-A): `organelles/canon.manifest.json`,
  `execution.json`, `MAPPING.md` added.

## 2026-10-08 — consumable interface + court integration (wave-4)

- **`src/api.rs`**: `Canon::open()` (open + verify all four hash chains,
  fail-closed), `active()` / `invariants()` / `by_id()` readers,
  `heads()`, `normative_basis()`, `snapshot()`. Frozen §56 stores are
  read-only through this API; writes stay on the CLI append path.
  Fixed during build: a real ordering bug (id-sorting the opened stores
  broke `heads()` — chain order is now preserved; sorting happens only
  in read views) and a wrong test assertion (`schema` is a JSON number,
  not a string).
- **`canon snapshot`**: chain-verified normative-state snapshot as
  canonical JSON (heads, active records, normative basis); exit 3 on any
  chain break. `CANON_DIR` redirects the stores.
- **Court integration** (`compute/court.py` v2): `derive_status` loads the
  snapshot, fails closed to UNKNOWN on unverifiable normative state, and
  records `canon: {context_hash, heads, normative_basis}` in every
  verdict. Normative basis cited from CANON (INV-01/02/03/06), not
  hardcoded. E2E: `compute/canon_court_e2e.py` — 13/13 green (bound
  evidence → ESTABLISHED; forged bare-True → UNKNOWN; tampered canon →
  UNKNOWN). Fixed a stale `derive_status(evidence, policy)` docstring.
- **Docs**: RFC-002 §59, README (CLI + consumable interface), this log.
- No changes to the frozen §56 stores: `canon audit` still green
  (50 records).

## 2026-10-08 — §56 freeze (gate 2 IMPLEMENTED)

- **Schema 2**: constitution records require ID/STATEMENT/SCOPE/AUTHORITY/
  ENFORCEMENT POINT/VIOLATION BEHAVIOR/EVIDENCE METHOD; contracts require
  inputs/outputs/errors/authorization/side effects/determinism/version/
  evidence requirements; decisions require question/alternatives/reason/
  evidence/author (+timestamp via `created_at`). Schema 1 grandfathered
  readable. Unknown fields rejected (fail-closed allowlist).
- **CLI hardening**: unknown `k=v` arguments rejected (exit 2), never
  silently dropped — found live during the freeze (`verified=true` was
  dropped), recorded as D-CANON-005, covered by
  `t_cli_rejects_unknown_args`.
- **Seeded §56 content**: `INV-01`..`INV-12` (machine-readable invariants,
  supersede schema-1 seeds `I1`..`I12`); contracts `C-CLI-001`,
  `C-STORE-001`, `C-CONTEXT-001`, `C-SEARCH-001`; decisions
  `D-CANON-002`..`D-CANON-006` (`D-CANON-001` superseded).
- **Hash-chain suite** `tests/hashchain.rs`: 12 cases — canonical
  encoding, prev-hash linkage, corruption, mid-file truncation,
  end-truncation boundary (documented: chain alone can't see a clean
  tail; detected via receipt heads), duplicate-id, concurrent append
  (8×25), malformed records, schema-2 validation, schema-1 readability,
  receipt determinism, CLI unknown-arg rejection.
- **Crate split**: `src/lib.rs` exposes modules for integration tests.
- **Docs**: `CANON-FREEZE-2026-10-08.md` (freeze evidence), `README.md`,
  `scripts/seed-56.py` (one-shot seeder, kept for auditability).
- `canon audit`: 50 records across 4 stores, exit 0.
- `canon context`: receipt surfaces enforcement points; superseded
  records tagged `[SUPERSEDED]`, never flattened (C12).

## 2026-10-08 (earlier) — initial PL-1-adjacent build (`efaa5b6`)

- Constitution `I1`..`I12` (prose, schema 1), `D-CANON-001`,
  probe-fixture contracts `PL-C-OLD`/`PL-C-NEW` + demo supersession,
  context receipt, strict append-only stores with hash chains.
