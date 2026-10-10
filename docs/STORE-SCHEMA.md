# CANON store schema — FROZEN (R-9)

**Frozen:** 2026-10-08 (§56 freeze). **Changed by:** this file's own supersession
record in `supersessions.jsonl` only. No other change path exists — appends are
validated by `store::validate_new` and unknown schema numbers are rejected.

## Stores (newline JSON, one record per line, canonical JSON encoding)

| Store | File | Record kinds |
|---|---|---|
| constitution | `constitution.jsonl` | principle (schema 1, grandfathered, superseded) / invariant (schema 2) |
| contracts | `contracts.jsonl` | contract |
| decisions | `decisions.jsonl` | decision |
| supersessions | `supersessions.jsonl` | supersession |

## Schema versions (frozen)

- **schema 1:** prose-only seed records (constitution `I1`..`I12`). Readable,
  never writable — all schema-1 records are SUPERSEDED; new appends require
  schema 2 and are rejected otherwise.
- **schema 2:** machine-readable records. Constitution invariants carry the full
  field set: `id`, `statement`, `scope`, `authority`, `enforcement_point`,
  `violation_behavior`, `evidence_method` (+ `kind`, `schema`, `status`,
  `severity`, `actor`, `created_at`, `rationale`, `hash`, `prev_hash`).

## Status vocabulary (frozen enum — no prose statuses)

`active` | `superseded` | `superseded_by_bypass`

- `active` — the current normative record.
- `superseded` — replaced by an explicit supersession record (`supersessions.jsonl`
  links `from` → `to` with a `reason`). The original record is never deleted or
  edited.
- `superseded_by_bypass` — the record's claim was defeated by a bypass
  (R-1/R-9). Filed in the SAME log as the original claim (one book), linked
  to the defeating evidence. A claim that was `active` and later bypassed
  MUST move to `superseded_by_bypass`, never silently to `superseded`.

## Hash chain (frozen)

Every record carries `hash` = sha256 of the canonical-JSON bytes of the record
**excluding** the `hash` field itself, chained over `prev_hash` (first record:
`GENESIS`). `store::verify_chain` re-checks every link and every recomputation;
`canon audit` fails closed (exit 3) on any mismatch.

## Tamper-evidence bound (R-2/R-3 honesty)

The chain detects **corruption, truncation (mid-file), and link forgery** —
proven by `tests/hashchain.rs` cases. It does **NOT** detect a cleanly
truncated tail (the chain alone cannot see it; detection requires comparing
against a context-receipt head — the suite asserts exactly this boundary),
nor a **full rewrite by a writer with write access** (dual rewrite is silent
to in-VM tooling). Out-of-band head copies live at
`~/workspace/dogfood/audit/oob-heads/canon.json` — a store this repo never
writes to. Until such a copy exists and is current, nothing here is called
tamper-evident; integrity is UNKNOWN, not PROVEN.
