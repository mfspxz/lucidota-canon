# RFC-002 — CANON / Engineering Authority Protocol

Status: Normative Engineering Specification
Date: 2026-10-08
System: CANON + PL
Purpose: Provide an authoritative, machine-readable engineering memory and evidence boundary for AI-assisted software development.

## 0. One-sentence definition

CANON is a deterministic, versioned engineering authority that records what a project must be, is intended to be, was decided, and what has been superseded, while PL records what has actually been observed and verified.

CANON does not remember for the AI. CANON prevents the AI from having to remember.

## 1. Problem

Long-running AI-assisted development has a structural failure mode: conversation → model context → partial recollection → implementation. The model may forget an invariant, confuse a proposal with a decision, revive superseded architecture, claim implementation without evidence, infer state from stale conversation, omit requirements from earlier sessions, silently alter semantics, or optimize for the immediate prompt instead of the governing contract. Therefore project authority MUST NOT reside in conversational context.

## 2. Authority hierarchy

CONSTITUTION → CONTRACT → DECISION → CLAIM → EVIDENCE → VERIFICATION → PL PROOF. No lower layer may silently override a higher layer.

## 3. Fundamental distinction

CANON: «What does the system say it is supposed to be?» PL: «What evidence establishes that it actually is?» Git: «What source changed?» Runtime: «What is happening now?» AI: «What should we investigate or build next?» These authorities MUST NOT be conflated.

## 4. Core epistemic model

CANON distinguishes: PROPOSED / DECIDED / ACTIVE / SUPERSEDED / REJECTED / UNKNOWN. PL distinguishes: UNKNOWN / ASSERTED / VERIFIED / FAILED / STALE. Different dimensions. CANON ACTIVE + PL FAILED = requirement stands, implementation does not satisfy it. CANON PROPOSED + PL VERIFIED is insufficient to establish an active requirement.

## 5-6. Constitution

Invariants governing the engineering system itself. Record: id, text, severity (advisory|required|non_negotiable), created_at, actor, status, supersedes, rationale. Minimum set:
I1 AI cannot self-certify. I2 UNKNOWN is the default. I3 Evidence is not proof. I4 Verification executes. I5 Corruption is visible. I6 History is immutable. I7 Supersession is explicit. I8 Source truth and runtime truth differ. I9 Deployment and implementation differ. I10 Shipped = implemented ∧ deployed ∧ proven (all VERIFIED). I11 Deterministic core: truth determination MUST NOT depend on generative AI. I12 Authority is explicit.

## 7-8. Contracts

Normative requirements for a subsystem/project version. Record: id (immutable), subject, text, status, created_at, actor, constitution refs, decision refs. Changing a requirement materially creates a new contract; old remains queryable (superseded_by links).

## 9-10. Decisions

Recorded architectural choices: what was chosen, why, rejected alternatives, constraints. Rejected designs SHOULD be recorded (prevents AI rediscovering old proposals as novel).

## 11-12. Supersession

Explicit supersession records (from → to + reason). Superseded records MUST NOT auto-resurrect on retrieval; queries return status.

## 13-14. Claims / evidence boundary

CANON may reference PL claims (claim_ref) but MUST NOT duplicate PL authority. CANON says C-123 exists; PL determines VERIFIED. CANON MUST NOT manufacture verification state.

## 15. Repository binding

CANON SHOULD identify repository + revision relevant to an engineering state.

## 16. Context receipt

Primary CANON↔AI interface. Every substantive AI engineering task SHOULD begin with a generated receipt: project, canon_version, repository, revision, working_tree, CONSTITUTION, ACTIVE CONTRACTS, ACTIVE DECISIONS, SUPERSEDED, CURRENT PL STATE, OPEN VIOLATIONS, REQUIRED NEXT ACTION, AUTHORITY. Deterministic; the model does not construct it.

## 17. Context freshness

Receipt identifies its revision; stale on material tree change; regenerate.

## 18-21. AI protocol

AI is non-authoritative: may propose/inspect/reason/implement/test/request verification/assert. Forbidden: declare VERIFIED, rewrite CANON, delete history, silently supersede, convert UNKNOWN→VERIFIED. Proposals become decisions only via explicit promotion (approve + actor). Canonical workflow: GET CONTEXT → INSPECT REPO → READ CONTRACTS → PROPOSE → IMPLEMENT → TEST → ASSERT → REQUEST VERIFICATION → PL EXECUTES → PL APPENDS → REGENERATE CONTEXT.

## 22. Storage

canon/constitution.jsonl, contracts.jsonl, decisions.jsonl, claims.jsonl, supersessions.jsonl, receipts/. pl/ledger.jsonl separate. Do not prematurely merge.

## 23-24. Serialization / integrity

Same canonical JSON as PL (UTF-8, sorted keys, schema, no whitespace). Append-only, prev_hash/hash chain, independently auditable. CANON and PL may share a ledger primitive.

## 25-27. Graphs

Relationships as first-class edges (contract→constitution→claim→revision→verification). Dependency declarations permitted; PL-2 MAY introduce dependency-aware invalidation (proof becomes STALE on dependency change, deterministic rule).

## 28-29. No vector DB as authority

Semantic retrieval MAY exist for convenience; MUST NOT be authoritative. Canonical records win conflicts. Search returns record + status + authority + supersession.

## 30-31. MCP interface

canon.context/get/search/invariants/contracts/decisions/claim/history/dependencies/diff/propose/approve; pl.assert/verify/status/history/audit/checkpoint; repo.status/diff/read/symbol/references/tree. Read broad, propose narrower, approve may require human.

## 32. Authority matrix

AI proposes; CANON records architecture; PL verifies; Git records source. AI cannot declare VERIFIED, delete history, or convert UNKNOWN→VERIFIED. Only PL execution creates VERIFIED.

## 33. Forbidden shortcuts

AI "done" → CANON implemented (no). AI "tested" → PL VERIFIED (no). Old decision retrieved → treated as current (no). Missing contract → invented (no). Failed verification → silent reset (no). Superseded → resurrected (no).

## 34-35. Contradictions / missing authority

Surface conflicts (D-020 ↔ D-031), never silently resolve. No decision → "NO ACTIVE DECISION FOUND", never fabricated history.

## 36-38. Context minimization / tasks

Receipts in full/task/surgical sizes. Tasks have identity, contracts, claims; states OPEN/IN_PROGRESS/BLOCKED/IMPLEMENTED/VERIFIED/FAILED/SUPERSEDED. IMPLEMENTED ≠ VERIFIED.

## 39-41. Receipts / checkpoints / rollback

CANON mutations produce receipts. Signed checkpoints + external witness for rollback protection (same limitation as PL: local attacker can rewrite; witness establishes existence ≤ T).

## 42-46. Versioning / migration / determinism

Version protocol, schema, constitution, receipts. Explicit migrations. context(A)==context(A) testable. Receipts carry context_hash; AI reports the hash it operated against. Agent sessions SHOULD record provenance (agent_id, model, session_id, context_hash, task_id, revision, tools, proposals, actions).

## 47-49. Agent action ledger / security / threats

AI's own READ/PROPOSE/WRITE/TEST/ASSERT/REQUEST stream can become append-only. Assume AI hallucinates/forgets/misunderstands; do not assume malice; make misbehavior non-authoritative by construction. Threats T1-T14 (stale context, hallucinated requirement, resurrected decision, false claim, fake verification, deleted history, rollback, concurrent writes, repo mismatch, dependency mutation, compromised probe/host, malicious agent, compromised MCP).

## 50. Serena integration

Serena MAY provide repo intelligence; CANON stays above. Muse → CANON context → Serena/repo → implement → PL verify. Serena is code understanding, not proof.

## 51-53. Organelles

Decomposable: canon-parser/store/query/context/graph/policy, pl-ledger/verifier/auditor, repo-inspector, checkpoint-signer, mcp-host. Each: local API, MCP interface, optional REST, typed schemas, deterministic. Transport independence: in-process/Unix socket/MCP/REST; byte-equivalence across transports.

## 54-55. Final architecture

MUSE (what should we do?) → CANON (what are we required to do?) → GIT (what source exists?) → PL (what has been demonstrated?) → RUNTIME (what is happening now?) → WITNESS (prove state existed at T). «AI may propose reality. CANON may define intended reality. Git records source reality. PL establishes evidenced reality. Runtime observation establishes current operational reality.» No layer impersonates another.

## 56. Minimal implementation (BUILD ORDER)

canon/constitution.jsonl, contracts.jsonl, decisions.jsonl, supersessions.jsonl; pl/ledger.jsonl. Rust binary: canon context/search/add-contract/add-decision/supersede; pl assert/verify/status/audit. Then MCP, then signed checkpoints, then relationship graph, then dependency invalidation, then agent session provenance.

## 57. CANON invariants

C1 No implicit authority (no record → UNKNOWN). C2 No silent mutation. C3 Supersession explicit. C4 AI proposals are not decisions. C5 CANON does not create proof. C6 PL does not create architecture. C7 Git does not create proof. C8 Search does not create authority. C9 Context reproducible. C10 Context revision-bound. C11 Contradictions surfaced. C12 Superseded material cannot silently resurrect.

## 58. Strategic naming

Not "memory" — authority. Memory can be stale/probabilistic/incomplete. CANON is small, explicit, versioned, inspectable, hostile to ambiguity. Gives Muse somewhere to forget to, rather than somewhere to remember from.

## Deployment constraint (operator, 2026-10-08)

CANON/PL consulting layer (MCP/bot queries) runs on Muse servers (local VM), NOT Cloudflare Workers — do not burn CF free tier on constant codebase reads. Codebase mirrored to Cloudflare via cheap storage (R2), not Workers, until paid tier. Recorded as decision D-CANON-001.
