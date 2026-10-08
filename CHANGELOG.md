# CHANGELOG — canon

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
