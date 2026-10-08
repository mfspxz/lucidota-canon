# CANON §56 FREEZE — 2026-10-08

Lane: CANON (thesis-defense work order, phase 3, workstream A).
Source: `~/workspace/canon`, git `efaa5b6` → freeze commit (this doc's commit).
Status: **§56 minimum FROZEN.** Gate claim: **gate 2 IMPLEMENTED**
(specified + implemented + self-tested). Gate 3+ (deployed/proven/
reproducible/operable) explicitly NOT claimed — deployment identity and
independent behavioral proof are downstream work.

## What §56 requires, and where it lives

| §56 item | Location | State |
|---|---|---|
| Machine-readable constitution: every invariant has ID / STATEMENT / SCOPE / AUTHORITY / ENFORCEMENT POINT / VIOLATION BEHAVIOR / EVIDENCE METHOD | `constitution.jsonl`, records `INV-01`..`INV-12` (schema 2) | FROZEN |
| No prose-only rules claimed as enforced | schema-1 seeds `I1`..`I12` superseded → `INV-01`..`INV-12`; new appends require the full field set (fail-closed) | FROZEN |
| Contracts for every externally meaningful interface | `contracts.jsonl`: `C-CLI-001` (CLI surface), `C-STORE-001` (store format), `C-CONTEXT-001` (receipt), `C-SEARCH-001` (search) — each with inputs, outputs, errors, authorization, side effects, determinism, version, evidence requirements | FROZEN |
| Decisions log: decision_id / question / decision / alternatives / reason / evidence / author / timestamp | `decisions.jsonl`: `D-CANON-001` (superseded) + `D-CANON-002`..`D-CANON-006` (schema 2) | FROZEN |
| Hash-chain correctness tests | `tests/hashchain.rs`: 12 cases (see below) | GREEN |
| Canon context receipts | `src/context.rs`; `canon context project=canon` | WORKING |

## Freeze evidence (2026-10-08, local VM, $0)

- `cargo test`: **8 unit tests + `hashchain_suite` (12 cases) — all green.**
  Cases: canonical encoding · prev-hash linkage · corruption detection ·
  mid-file truncation detection · end-truncation boundary (chain alone
  cannot see a clean tail — detected via context-receipt head comparison;
  the suite asserts exactly this boundary) · duplicate-id detection ·
  concurrent append (8 writers × 25 records, chain intact) ·
  malformed-record behavior (non-JSON / non-object / missing schema /
  unknown schema / blank-line tolerance) · schema-2 validation
  (missing, empty, unknown, wrong-kind, bad-severity) · schema-1 grandfathered
  readability · receipt determinism · CLI unknown-argument rejection.
- `canon audit`: **OK — 50 records across 4 stores, exit 0.**
  (24 constitution: 12 schema-1 seeds SUPERSEDED + 12 INV ACTIVE;
  6 contracts: 2 legacy probe fixtures + 4 real; 6 decisions;
  14 supersessions.)
- `canon context project=canon`:
  `context_hash: 13f779d52b28e42835bcf7b8da5b5e6b99739ff66f96267c3922abed585a5a88`
  `canon_heads: constitution=af86f715d52b contracts=9d4226538aef decisions=ba5def1b68c3 supersessions=43959c8ed5f3`
- Negative-path probes (all fail-closed, verified by hand + in-suite):
  missing required field → exit 2; unknown CLI argument (`verified=true`)
  → exit 2 "rejected, never silently dropped"; unknown store field →
  exit 4; dangling constitution/decision ref → exit 4; duplicate id →
  exit 4; tampered byte / removed line → `audit` exit 3.

## Decisions in this freeze

- **D-CANON-003** — migration strategy: supersede, never rewrite
  (schema-1 stays readable; schema-2 required for new appends).
- **D-CANON-005** — unknown CLI arguments rejected (exit 2), never
  silently dropped. Found live during this freeze: `verified=true` was
  silently dropped before the fix; the fix is covered by
  `t_cli_rejects_unknown_args`.
- **D-CANON-006** — this freeze itself.

## Known imperfections (honest, not hidden)

1. The two `PL-C-OLD`/`PL-C-NEW` contracts and their supersession are
   scaffolding fixtures from the initial build, not real contracts.
   They are inert and clearly labeled; left in place to preserve chain
   history.
2. `canon` has no `update`/`delete` by design (INV-06); the schema-1
   seeds are therefore permanently SUPERSEDED-but-present. This is
   doctrine, not a bug.
3. End-truncation of a store tail is invisible to `verify_chain` alone;
   detection requires a prior context receipt's head hashes (§5th test
   case documents the boundary). Signed checkpoints (RFC-002 §39-41)
   would close this; explicitly out of scope for §56.
4. One pre-existing compiler warning (`unnecessary unsafe block` on the
   fsync binding, `src/store.rs`); harmless, left untouched.

## Reproduce

```
cd ~/workspace/canon
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release && cargo test          # 8 + 12 green
./target/release/canon audit                 # exit 0
./target/release/canon context project=canon # deterministic receipt
python3 scripts/seed-56.py                   # one-shot §56 seeding (already applied)
```

## What is NOT claimed

- Gate 3 DEPLOYED: the `canon` binary is a local build, no deployment
  identity beyond this VM.
- Gate 4 BEHAVIORALLY PROVEN: tests are self-authored; no independent
  verifier has run them.
- PL-lane adoption: `lucidota-receipt/1` emission from CANON is not
  required by §56 and not done here.
- Cloudflare: untouched per directive (CF down, local-first).
