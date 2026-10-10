# canon — organelle mapping decisions

Decomposition reviewed 2026-10-10 under the corrected doctrine
(decompose ACTUAL capabilities; map-or-design per unit, justified).

## What the code actually does

`canon` is the Canonical Engineering Authority (RFC-002): four append-only,
hash-chained JSONL stores (constitution, contracts, decisions,
supersessions) + a read API that verifies every chain on open (fail-closed)
+ BM25 search + canonical JSON + deploy helpers + a `canon` CLI whose
`add-*` commands are the only write path. The four stores are ONE authority —
reads and writes share the store core (`store::read_strict` /
`store::append` with `validate_new`, duplicate-id guard, file locking).
Splitting reads from writes, or one store per organelle, would duplicate the
chain-verification core or manufacture shared-state organelles. One organelle
(`ca`), six ops; per-op `mutation_class` + `required_capability` carry the
read/write seam (the same op-level decomposition the ledger-graph pilot used).

## Capability units → ops

| Unit | Source | Ops | Decision |
|---|---|---|---|
| Chain-verified read authority over the four stores | `api.rs` (Canon::open, heads, active, by_id, snapshot), `store.rs` (read_strict/verify_chain) | `verify`, `get`, `active`, `snapshot` (ReadOnly, Proven) | **Bespoke** (`ca`). Justification: the computation is: open, verify four hash chains, then project. `ref-ledger-append` appends blindly (opposite direction); `ref-json-validate` checks one schema (canon verifies chains + serves domain views). No ref organelle fits. |
| BM25 search across all stores | `bm25.rs` (tokenize/rank) + `main.rs` SEARCH_FIELDS | `search` (ReadOnly, Proven) | **Bespoke** (`ca`). Justification: pure BM25 ranking over canon's record schema. `ref-text-transform` transforms text; this ranks. Different computation. The doc-building mirrors `main.rs` SEARCH_FIELDS (documented, not duplicated logic — the ranker is the shared `bm25` module). |
| Fail-closed write path (the CLI `add-*` logic as a callable op) | `store.rs` (append → validate_new + dup-id guard + chain + file lock) | `add` (Write, `ca.write`) | **Bespoke** (`ca`). Justification: the write path's identity IS canon's validation (schema-2 `validate_new`, "supersede, do not mutate"). `ref-ledger-append` would bypass every semantic guard that makes canon canon. Adopting it would be the wrong layer — the exact failure mode the corrected doctrine warns about. |

## Deliberate exclusions

- `src/main.rs` (the `canon` CLI): human entrypoint; the organelle ops expose
  the same library functions the CLI calls. No new contract from a second
  binary surface.
- `src/deploy.rs`, `src/time.rs`, `src/context.rs`, `src/cjson.rs`:
  supporting modules (deploy helpers, clock, context, canonical JSON) — used
  by the ops, not independently callable capabilities.
- `tests/`: integration tests, not a capability.

## Reference-organelle check (all 12 considered)

- `ref-ledger-append`: blind appends; canon's write path is
  validate→dedup→chain→lock. Rejected (wrong layer — adopting it would drop
  the semantic guards).
- `ref-json-validate`: single-schema conformance; canon verifies four hash
  chains and serves authority views. Rejected.
- `ref-text-transform`, `ref-kv-store`, others: no overlap.
- Verdict: genuinely bespoke. Nothing reinvented that already existed.

## Execution facts

- Valence Proven (deterministic; chains verified on every open, fail-closed).
- fs: the canon store dir (`$CANON_DIR` else default), read-write for `add`,
  read-only otherwise. `add` takes the file lock via `store::append`.
- No network, no secrets. Stateless: suspend = no-op, kill = drop.
- Envelope: 1 cpu / 256 MB / 30 s wall-time.
- Receipts: content-addressed sha256, chained; `verify_chain` green in tests.
