# CANON test policy — property tests vs change-detection tests (R-7)

**Rule:** property tests and change-detection tests are separated by file and
labeled in their module docstrings. A property test's expectation may NEVER be
rewritten to match a behavior change without adversarial sign-off (a red-team
wave or an explicit review that names the new behavior as intended). A
change-detection test exists to say "the behavior moved" — nothing more.

## Classification

| File | Class | What it proves |
|---|---|---|
| `tests/hashchain.rs` | **property** | Security invariants of the append-only chain: corruption is detected, mid-file truncation is detected, clean-tail truncation is NOT detectable by the chain alone (asserted boundary), forged links break verification. Stated without implementation terms (no `verify_chain` internals named). |
| `tests/invariants.rs` | **property** | Every frozen invariant cites an enforcement point that resolves to a real file:line whose content matches the citation (semantic anchors). UNENFORCED entries must carry an explicit reason. |
| `tests/verify_gaps.rs` | **property** | Wave-23 spec gaps V-01..V-06: fail-closed behaviors of the CLI/API (unknown command rejected, dangling refs exit 4, PL-unavailable → UNKNOWN, etc.). Stated as observable behavior, not internals. |
| `tests/deploy_identity.rs` | **property** | Deployment manifest binds constitution heads + binary hash + operator name, verifies under ed25519, and rejects tampered manifests. The test key is explicitly TEST-KEY — NOT FOR PRODUCTION. |
| `src/store.rs` unit tests (`kinds_have_files`, `superseded_wins`) | **change-detection** | Assert current read-view semantics (e.g. superseded-wins ordering). Labeled as such in the docstring. |

## How to add a test

1. Decide the class FIRST. If the test would have to change when the
   implementation changes but the security property doesn't, it is a
   change-detection test — put it in `src/` unit tests or a
   `tests/change_detection_*.rs` file and label it.
2. If it states a security invariant ("a tampered record is never attested
   clean", "an unavailable PL never becomes positive state"), it is a
   property test — put it in `tests/` with a PROPERTY header and no
   implementation terms in the assertions.
3. Property-test expectation changes require adversarial sign-off: name the
   wave/review and the reasoning in the commit message.

## Worked example (2026-10-10)

A docstring addition at the top of `src/context.rs` shifted frozen
line-number citations in `invariants.json` (INV-02 anchor 252 → 257). The
`every_enforcement_point_resolves_to_real_file_line` property test failed
immediately. The fix went into the CODE (reverted the top edit; appended the
note at end-of-file so no citation moved) — the test and the frozen
constitution were not touched. That is the policy: the test is the control;
code moves, tests don't.
