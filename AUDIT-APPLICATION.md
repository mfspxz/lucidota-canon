# AUDIT-APPLICATION — canon

**Slice:** brutal-audit rollout 3/8. **Date:** 2026-10-10. **Repo:** ~/workspace/canon.
**Audited at:** `345354f`. **This pass:** TBD (filled after commit).

## What this is

Deterministic engineering authority: a frozen §56 machine-readable constitution,
contracts, decisions, and supersessions in four hash-chained JSONL stores, with a
Rust CLI/API (`canon`), context receipts, and an ed25519-signed deployment
manifest. CANON records what must be; PL (separate system) records what was
verified.

## Per-finding application

- **R-1 (instance-only fix claims, one book):** APPLIED. Freeze evidence and
  gap-closure claims are `instance-only` until a dedicated red-team wave signs
  off the class. The defeat log is already one book with the claim log:
  `supersessions.jsonl` links every supersession `from` → `to` with a reason,
  and the status vocabulary now includes `superseded_by_bypass` for
  bypass-mirrored defeats (docs/STORE-SCHEMA.md).
- **R-2 (honest guardrail bounds):** APPLIED (was already strong). The hashchain
  suite asserts the exact tamper-evidence boundary (clean-tail truncation NOT
  detectable by the chain alone — detected via context-receipt head comparison);
  INV-08/INV-09 separate source/runtime truth and deployment/implementation;
  docs/STORE-SCHEMA.md states the tamper-evidence bound in one place.
- **R-3 (out-of-band head copy):** APPLIED.
  `~/workspace/dogfood/audit/oob-heads/canon.json` — per-store `seq` +
  `head_hash` (constitution 24 / af86f715d52b…, contracts 6 / 9d4226538aef…,
  decisions 6 / ea14bf401555…, supersessions 14 / 43959c8ed5f3…), git HEAD,
  issuing binary sha256, recorded 2026-10-10T22:41:56Z. Honest limitation
  documented in the file: same-VM; integrity is UNKNOWN-not-PROVEN without an
  auditor holding the head.
- **R-4 (backlog triage):** one standing item — see Wont-fix list. No other
  backlog: the repo has no defect register and no standing HIGHs of its own.
- **R-5 (signature games):** N/A. No signature/denylist/strip security machinery
  (BM25 is a ranking algorithm, not a denylist).
- **R-6 (detector + consumer + gate):** N/A as a gap. `canon audit` is the
  detector and it has named consumers: `Canon::open()` verifies all four chains
  fail-closed; court `derive_status` fails closed to UNKNOWN on unverifiable
  normative state.
- **R-7 (property vs change-detection):** APPLIED. New docs/TEST-POLICY.md;
  `tests/hashchain.rs`, `tests/invariants.rs`, `tests/verify_gaps.rs`,
  `tests/deploy_identity.rs` labeled PROPERTY; `src/store.rs` unit tests
  labeled CHANGE-DETECTION. Policy: property-test expectations are never
  rewritten to match a behavior change without adversarial sign-off.
- **R-8 (unsigned provenance):** APPLIED. Context receipt declared
  honestly-unsigned in `src/context.rs` (hash-bound; change-detection power
  against the OOB head, not provenance). Deployment manifest is ed25519-signed
  but TEST-KEY — NOT FOR PRODUCTION (DEPLOYMENT-IDENTITY.md, unchanged).
- **R-9 (frozen claim schema):** APPLIED. docs/STORE-SCHEMA.md freezes schema
  1/2 and the status enum `active | superseded | superseded_by_bypass`; prose
  statuses are rejected by `store::validate_new`.
- **R-10 (record-keeping in the same motion):** APPLIED. CHANGELOG.md updated
  this motion: added the 2026-10-10 rigor entry and a 2026-10-09 entry (BM25
  search, semantic anchors in invariants.rs, cjson u64 rejection, organelle
  migration) that had been missing. OOB head copy carries its own update
  procedure.
- **R-11:** explicit UNKNOWN list below.
- **R-12 (no unsourced footnotes):** APPLIED. No unsourced oral footnotes found.
  RFC-002 §149 documents the CANON_DIR-redirection attack together with the
  fail-closed mitigation (`derive_status` → UNKNOWN).

## Commits

This pass (`TBD`): CHANGELOG.md, docs/STORE-SCHEMA.md (new),
docs/TEST-POLICY.md (new), src/context.rs, src/store.rs,
tests/hashchain.rs, tests/invariants.rs, tests/verify_gaps.rs,
tests/deploy_identity.rs — docs + doc-comments only. Plus this file.

## Test results

Measured 2026-10-10, full suite green: **35/35**.
- lib unit tests: 24/24; bin unit tests: 4/4; deploy_identity: 1/1;
  hashchain: 1/1 (12 cases); invariants: 4/4; verify_gaps: 1/1.
- The suite initially failed to build on the pre-existing `organelle-host`
  dependency conflict (hmac 0.13.0 vs sha2 0.10/digest 0.10; E0599/E0277 —
  measured). During this pass the organelles slice owner pinned
  `hmac = "=0.12.1"` in `organelle-host/Cargo.toml`; the build then succeeded
  and all 35 tests passed. Canon-side change for this: none.
- `canon audit` — measured OK 2026-10-10: 50 records across the four stores;
  `verify_chain` fail-closed, exit 3 on chain break. (Binary:
  `target/debug/canon`, sha256 in the OOB head file.)

**R-7 worked example (this pass):** my first `src/context.rs` docstring edit
added 6 lines at the top of the file, shifting the frozen line-number
citations in `invariants.json` (the INV-02 anchor moved 252 → 257). The
`every_enforcement_point_resolves_to_real_file_line` property test caught it
immediately. Per the policy I had just written, I fixed the CODE, not the
test: reverted the top edit and appended the R-8 note at end-of-file (no
citation moves). The frozen constitution was not touched.

## Wont-fix list (R-4)

1. **`cargo test` build failure** (was the only standing item — RESOLVED
   DURING THIS PASS by its owner). Cause: `f727745` (2026-10-10,
   organelle-migration tier-A) added a path dependency on
   `../organelles/crates/organelle-host`, whose `hmac = "0.13.0"` required
   digest 0.11's `EagerHash` while the organelles workspace pins
   `sha2 = "0.10"` (digest 0.10). Measured E0599/E0277 2026-10-10. The
   organelles slice owner pinned `hmac = "=0.12.1"` in
   `organelle-host/Cargo.toml` the same day; canon's suite then built and
   passed 35/35. **Decision:** no canon-side action (budget 0 — the triage
   call was correct; the owner fixed it). `Cargo.lock` left at HEAD; local
   builds re-resolve against the fixed organelles tree.

## Explicit UNKNOWN list (R-11)

1. Production deployment-key custody — Zack-gated (`goal_b4eaabcc3b4d`),
   undecided; the manifest stays TEST-KEY until then.
2. PL ledger state referenced by context receipts — PL is a separate system;
   canon never caches its verdicts (INV-03).

## Top critical issues

1. **Cross-VM integrity not established.** The OOB head copy is same-VM;
   without an independent auditor holding the head, chain integrity is
   UNKNOWN, not PROVEN (R-3 honest bound — documented, not fixed).
2. **Deployment identity is not production-safe.** Manifest signed TEST-KEY;
   production custody undecided (Zack-gated).
3. **Frozen line-number citations are brittle.** The R-7 incident this pass
   (my docstring edit moved a cited line; the property test caught it) shows
   the mechanism works, but every future edit near a cited line risks the
   same trip. Citations are pinned to content AND line; content-anchored
   citations would remove the line-number fragility.
