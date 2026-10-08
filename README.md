# CANON — Canonical Engineering Authority (RFC-002)

CANON records what a project **must be** / **was decided** / **was superseded**.
PL records what was **observed and verified**. Git records what **source exists**.
AI proposes. No layer impersonates another.

§56 minimum **FROZEN** 2026-10-08 — see `CANON-FREEZE-2026-10-08.md`
for the evidence. Spec: `RFC-002-canon.md`.

## Build / test / run

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
cargo test                      # 8 unit + hashchain_suite (12 cases)
./target/release/canon audit    # integrity + semantic audit, exit 0 on green
```

## CLI

```sh
canon add-principle  id=INV-99 statement=... scope=... authority=... \
    enforcement_point=... violation_behavior=... evidence_method=... \
    severity=non_negotiable
canon add-contract   id=C-... subject=... text=... inputs=... outputs=... \
    errors=... authorization=... side_effects=... determinism=... \
    version=... evidence_requirements=... [constitution=INV-01,INV-02]
canon add-decision    id=D-... question=... decision=... \
    alternatives='["a","b"]' reason=... evidence=... author=...
canon supersede from=<id> to=<id> reason=...
canon search <query>
canon context project=<name>
canon audit
```

Exit codes: `0` ok · `2` usage (incl. unknown arguments — rejected, never
silently dropped) · `3` integrity failure · `4` semantic failure.

## Stores

One hash-chained append-only JSONL per kind: `constitution.jsonl`,
`contracts.jsonl`, `decisions.jsonl`, `supersessions.jsonl`.
Canonical JSON (UTF-8, sorted keys, no insignificant whitespace, integers
only); `hash = SHA-256(canonical bytes without "hash")`; `prev_hash`
links to the previous record (`GENESIS` for the first). Schema 2 is
required for new records; schema 1 is grandfathered readable.
`CANON_DIR` env overrides the store directory (tests use temp dirs).

## Layout

- `src/` — `main.rs` (CLI), `store.rs` (hash-chained stores + schema-2
  validation), `cjson.rs` (canonical JSON), `context.rs` (deterministic
  context receipts), `time.rs`, `lib.rs`
- `tests/hashchain.rs` — hash-chain correctness suite (12 cases)
- `scripts/seed-56.py` — one-shot §56 seeding (already applied; kept for
  auditability)
- `RFC-002-canon.md` — normative spec
- `CHANGELOG.md` — history
