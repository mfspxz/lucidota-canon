#!/usr/bin/env python3
"""§56 freeze seeding for CANON (one-shot, 2026-10-08).

Supersedes the prose-only schema-1 seeds (I1..I12, D-CANON-001) with
machine-readable schema-2 records via the canon CLI — exercising the real
append/supersede path, so the migration itself is hash-chained evidence.
Idempotent-ish: fails loudly if any record already exists.
"""
import subprocess, sys, json

BIN = "/home/hatch/workspace/canon/target/release/canon"

def run(*args):
    p = subprocess.run([BIN, *args], capture_output=True, text=True)
    if p.returncode != 0:
        print(f"FAIL [{' '.join(args[:2])}]: rc={p.returncode}\n{p.stderr}", file=sys.stderr)
        sys.exit(1)
    print(p.stdout.strip())
    return p

INV = [
 ("INV-01","I1","non_negotiable",
  "AI agents cannot self-certify. An AI assertion cannot create PL VERIFIED.",
  "All verification and proof claims in CANON/PL workflows.",
  "CANON constitution (RFC-002 §6 I1); enforced by the PL verifier, never by the AI.",
  "PL verify pipeline: only PL execution may emit VERIFIED. The canon CLI has no command that writes a verification verdict.",
  "Fail-closed: any record attempting to assert verification state is rejected (exit 4); existing claims are untouched; the attempt is ledgered.",
  "Attempt `canon add-principle ... verified=true` → rejected as unknown field; audit PL code for absence of any mark_proven path."),
 ("INV-02","I2","non_negotiable",
  "UNKNOWN is the default. Missing evidence never becomes positive state through inference.",
  "All CANON queries, context receipts, and status derivations.",
  "CANON constitution (RFC-002 §6 I2); enforced by canon search/context/audit.",
  "canon search with no match prints an explicit NO ACTIVE DECISION FOUND marker; context receipt marks unavailable PL state as UNKNOWN, never blank.",
  "Fail-closed: queries return an explicit UNKNOWN marker; no empty-success path exists.",
  "Run `canon search <gibberish>` → exit 0 with NO ACTIVE DECISION FOUND marker on stdout."),
 ("INV-03","I3","non_negotiable",
  "Evidence is not proof. Evidence exists independently of verification.",
  "The CANON↔PL boundary; all claim/evidence references.",
  "CANON constitution (RFC-002 §6 I3, §13-14).",
  "Schema allowlist: CANON records have no verification-verdict field; context receipt queries PL live instead of caching verdicts.",
  "Fail-closed: append with a verdict-like field is rejected as an unknown field (exit 4).",
  "Attempt `canon add-principle ... verified=true` → exit 2, argument rejected and never silently dropped; store-level unknown fields → exit 4."),
 ("INV-04","I4","non_negotiable",
  "Verification executes. VERIFIED requires execution of an authorized verification procedure.",
  "PL verification lifecycle; CANON's surfacing of PL state.",
  "CANON constitution (RFC-002 §6 I4); procedure owned by the PL lane.",
  "Context receipt surfaces PL FAILED dimensions as violations (V2); CANON never emits VERIFIED itself.",
  "Degrade-open on read, closed on write: receipt marks PL state UNKNOWN when the pl binary is unavailable; no verdict is invented.",
  "Rename the pl binary away → `canon context` prints the unavailable-PL UNKNOWN marker."),
 ("INV-05","I5","required",
  "Corruption is visible. Malformed canonical state cannot silently disappear.",
  "All CANON store files (constitution/contracts/decisions/supersessions).",
  "CANON constitution (RFC-002 §6 I5, §23-24); enforced by canon audit.",
  "canon audit: read_strict fails on any malformed line; verify_chain fails on any hash/prev_hash mismatch (exit 3).",
  "Fail-closed: audit exits 3; no partial or skipped-and-continued results are trusted.",
  "Hash-chain suite (tests/hashchain.rs): corruption, mid-file truncation, duplicate, malformed-record cases all green."),
 ("INV-06","I6","non_negotiable",
  "History is immutable. Historical decisions and evidence are not rewritten.",
  "All CANON stores and their git history.",
  "CANON constitution (RFC-002 §6 I6).",
  "Append-only CLI surface (no update/delete command exists); duplicate-id rejection; fs2 exclusive lock serializes writers; fsync before success.",
  "No CLI path can mutate history; direct file edits are detected by audit (exit 3).",
  "Enumerate CLI commands — no update/delete; tamper test → `canon audit` FAIL exit 3."),
 ("INV-07","I7","required",
  "Supersession is explicit. Replacing a requirement creates a new record linked to the old one.",
  "Record lifecycle for constitution, contracts, decisions.",
  "CANON constitution (RFC-002 §6 I7, §11-12).",
  "canon supersede requires both endpoint ids to exist plus a reason; effective_status surfaces SUPERSEDED in search and context receipts.",
  "Dangling endpoints rejected (exit 4); superseded records are never returned as active (C12).",
  "`canon supersede from=NOPE to=X reason=y` → exit 4; search output always carries the [STATUS] tag."),
 ("INV-08","I8","non_negotiable",
  "Source truth and runtime truth differ. A source artifact cannot prove runtime behavior.",
  "Context receipts; any reasoning about deployed behavior.",
  "CANON constitution (RFC-002 §6 I8; RFC-002 §15).",
  "Receipt records git revision AND working_tree state AND live PL state as three separate fields.",
  "Receipt never conflates them: revision, DIRTY/CLEAN, and PL dimensions are printed on separate lines.",
  "`canon context` output contains distinct `revision:`, `working_tree:`, and CURRENT PL STATE sections."),
 ("INV-09","I9","required",
  "Deployment and implementation differ. Compilation cannot prove deployment.",
  "Thesis-defense gate model; release decisions.",
  "CANON constitution (RFC-002 §6 I9).",
  "Receipt PL STATE section separates implemented/deployed/proven per claim; no single done field exists in any schema.",
  "Schema allowlist contains no done/deployed shortcut fields; any such field is rejected on append.",
  "Inspect schema-2 allowlists (store.rs allowed_fields) — no completion-shortcut fields present."),
 ("INV-10","I10","required",
  "Shipped has a formal definition: implemented VERIFIED and deployed VERIFIED and proven VERIFIED.",
  "Release gating across the thesis-defense work order.",
  "CANON constitution (RFC-002 §6 I10); work-order §B7. Computed by the PL lane.",
  "CANON records the rule; PL computes overall=shipped. Receipt REQUIRED NEXT ACTION names the blocking dimension.",
  "CANON cannot declare shipped — no command or field exists for it; receipt degrades to naming the next action.",
  "pl status --json overall field transitions only via PL verification events, never via CANON writes."),
 ("INV-11","I11","non_negotiable",
  "Deterministic core. Truth determination MUST NOT depend on generative AI.",
  "canon binary, audit, context receipts, hashing, tests.",
  "CANON constitution (RFC-002 §6 I11).",
  "Pure Rust, zero network, zero model calls; context(A)==context(A) is testable; receipt carries context_hash.",
  "Any nondeterminism is a defect: receipt determinism is covered by an automated test.",
  "cargo test: receipt determinism + canonical-encoding determinism cases green; two `canon context` runs → identical context_hash."),
 ("INV-12","I12","required",
  "Authority is explicit. Every authoritative record has an identifiable origin.",
  "All CANON records and their references.",
  "CANON constitution (RFC-002 §6 I12; RFC-002 §32 authority matrix).",
  "Every record carries actor + created_at (UTC ISO); contracts validate constitution/decision refs on append; audit re-checks dangling refs.",
  "Dangling constitution ref → exit 4; anonymous mutation via the API is impossible (actor always stamped).",
  "`canon add-contract ... constitution=BOGUS` → exit 4 dangling-ref rejection."),
]

for (iid, rfc, sev, stmt, scope, auth, enf, viol, ev) in INV:
    run("add-principle", f"id={iid}", f"statement={stmt}", f"scope={scope}",
        f"authority={auth}", f"enforcement_point={enf}",
        f"violation_behavior={viol}", f"evidence_method={ev}",
        f"severity={sev}", "status=active",
        f"rationale=RFC-002 section 6; §56 freeze 2026-10-08", f"rfc_ref={rfc}",
        f"supersedes=I{rfc[1:]}")

# supersede the prose seeds -> machine-readable invariants
for n in range(1, 13):
    run("supersede", f"from=I{n}", f"to=INV-{n:02d}",
        "reason=§56 freeze: prose-only schema-1 seed superseded by machine-readable invariant with ID/STATEMENT/SCOPE/AUTHORITY/ENFORCEMENT POINT/VIOLATION BEHAVIOR/EVIDENCE METHOD")

CONTRACTS = [
 ("C-CLI-001","canon CLI surface",
  "The canon command-line interface is the sole write path into CANON stores.",
  "argv: add-principle|add-contract|add-decision|supersede|search|context|audit plus k=v arguments",
  "stdout records / receipts; stderr diagnostics; exit codes 0 ok / 2 usage / 3 integrity / 4 semantic",
  '["exit 2: missing/invalid arguments","exit 3: integrity failure (corrupt store, broken chain)","exit 4: semantic failure (dangling ref, duplicate id, schema violation)"]',
  "local OS user only — no network, no remote callers; single-machine trust boundary",
  "appends exactly one hash-chained record per mutating command (fsync before success); search/context/audit are read-only",
  "fully deterministic: same store state + same argv → byte-identical output (modulo created_at timestamps and git/PL environment in receipts)",
  "canon 0.1.0 / protocol 1 / store schema 2",
  "cargo test green (unit + hashchain suite); `canon audit` exit 0 on the frozen stores",
  "INV-05,INV-06,INV-11"),
 ("C-STORE-001","hash-chained JSONL store format",
  "One append-only JSONL file per record kind; canonical JSON; SHA-256 chain over prev_hash.",
  "schema-2 record maps (validated against the kind allowlist before write)",
  "durable file append + fsync; exclusive fs2 lock serializing concurrent writers",
  '["Corrupt: malformed line / bad schema → read fails","Chain: hash or prev_hash mismatch → verify fails","Semantic: duplicate id / unknown field / missing required field → append rejected"]',
  "local OS user (filesystem owner); no network writers",
  "file append + fsync only; no compaction, no rewrite, no delete path",
  "canonical JSON (UTF-8, sorted keys, no insignificant whitespace, integers only); hash = SHA-256(canonical bytes without the hash field)",
  "store schema 2 (schema 1 grandfathered readable)",
  "tests/hashchain.rs: canonical encoding, prev-hash linkage, corruption, truncation (mid + end boundary), duplicate, concurrent append (8x25), malformed records",
  "INV-05,INV-06,INV-11"),
 ("C-CONTEXT-001","engineering context receipt",
  "Deterministic CANON→AI interface: every substantive task begins with a generated receipt (RFC-002 §16).",
  "project name",
  "receipt body (fixed field order, sorted lists) + context_hash (SHA-256 of body); generated_at is trailing non-authoritative metadata",
  '["exit 3: any store fails strict read or chain verification"]',
  "read-only: any local caller; queries the pl binary read-only for PL state, degrades to UNKNOWN if unavailable",
  "none — pure read path; never writes to CANON stores",
  "byte-identical given identical CANON stores + git revision + PL state (tested)",
  "canon protocol 1",
  "receipt determinism test green; context_hash recomputation matches",
  "INV-02,INV-08,INV-11"),
 ("C-SEARCH-001","search semantics",
  "Substring search across all kinds; status always surfaced, never flattened (C12).",
  "query string",
  "matching records as `kind id [EFFECTIVE_STATUS] :: snippet`, sorted deterministically; no match → explicit NO ACTIVE DECISION FOUND marker",
  '["none: empty result is a marked outcome, not an error"]',
  "read-only: any local caller",
  "none",
  "deterministic ordering (kind order, then id); same store → same output",
  "canon protocol 1",
  "manual probe: search for superseded id shows [SUPERSEDED]; gibberish query shows the explicit marker",
  "INV-02,INV-07"),
]

for (cid, subj, text, inputs, outputs, errors, authz, side, det, ver, evreq, const_refs) in CONTRACTS:
    run("add-contract", f"id={cid}", f"subject={subj}", f"text={text}",
        f"inputs={inputs}", f"outputs={outputs}", f"errors={errors}",
        f"authorization={authz}", f"side_effects={side}", f"determinism={det}",
        f"version={ver}", f"evidence_requirements={evreq}",
        f"constitution={const_refs}", "status=active")

DECISIONS = [
 ("D-CANON-002","Where does the CANON/PL consulting layer run?",
  "CANON/PL serving stays on Muse servers (local VM), not Cloudflare Workers",
  "The CANON/PL consulting layer (MCP/bot queries against the codebase) runs on Muse servers (local VM), not Cloudflare Workers. Codebase mirrored to Cloudflare via cheap storage (R2), not Workers, until paid tier.",
  '["Serve codebase reads through Cloudflare Workers","Run the consulting layer on Zackary\'s own server","No consulting layer at all"]',
  "Do not burn Cloudflare free-tier read quota on constant codebase reads until the operator can pay.",
  "RFC-002 deployment constraint, operator directive 2026-10-08 (Zackary); ledger decision record.",
  "zackary","CANON/PL serving stays on Muse servers, not Cloudflare Workers","D-CANON-001"),
 ("D-CANON-003","How do prose-only seed records become §56 machine-readable invariants without violating append-only history?",
  "Supersede, don't rewrite: schema-1 seeds stay in the chain (grandfathered readable); INV-01..INV-12 carry the full §56 field set; new appends require full fields (schema 2).",
  "Explicit supersession of I1..I12 → INV-01..INV-12; schema-1 remains readable (grandfathered); new appends require full fields (schema 2).",
  '["In-place rewrite of the JSONL files","Same-id migration records with a migration_of link","Keep the prose seeds as the enforced constitution"]',
  "Append-only history is non-negotiable (INV-06); explicit supersession dogfoods the machinery the thesis defends; prose-only rules must never be claimed as enforced (§56).",
  "This freeze: 12 supersession records I1..I12 → INV-01..INV-12 in supersessions.jsonl; store.rs SCHEMA=2 with schema-1 read acceptance; tests/hashchain.rs t_schema1_readable.",
  "lucidota","§56 freeze migration strategy",""),
 ("D-CANON-005","What happens when a caller passes an argument the canon CLI does not define?",
  "Unknown CLI arguments are rejected (exit 2), never silently dropped and never smuggled into records.",
  "Unknown k=v arguments are rejected with exit 2 (usage error); unknown fields at the store API level are rejected with exit 4 (semantic error) by the schema-2 allowlist.",
  '["Silently ignore unknown arguments","Fold unknown arguments into the record"]',
  "Silent dropping lets a caller believe they set something they did not (false authority); smuggling lets callers invent fields (fake verified flags). Found live during the §56 freeze: verified=true was silently dropped before this fix.",
  "Negative-path probe 2026-10-08: `canon add-principle ... verified=true` → exit 2 with 'unknown argument(s): verified — rejected, never silently dropped'; store-level probe via tests/hashchain.rs t_schema_validation.",
  "lucidota","Unknown CLI arguments are rejected, never dropped",""),
 ("D-CANON-004","What does 'hash chains proven' mean for the §56 freeze?",
  "Hash-chain test scope for the §56 freeze",
  "Hash-chain correctness is established by tests/hashchain.rs: canonical encoding, prev-hash linkage, corruption detection, mid-file truncation, end-truncation boundary via receipt heads, duplicate detection, concurrent append, malformed-record behavior, schema-2 validation, schema-1 readability, receipt determinism.",
  '["Unit tests only, no adversarial cases","External audit instead of self-tests"]',
  "Corruption must be detected by the machinery itself, not asserted; the end-truncation boundary must be stated honestly (the chain alone cannot see a cleanly truncated tail).",
  "cargo test: 8 unit + hashchain_suite (11 cases) green 2026-10-08; canon audit exit 0.",
  "lucidota","Hash-chain test scope for the §56 freeze",""),
]

for (did, q, title, dec, alts, reason, ev, author, t, sup) in DECISIONS:
    args = ["add-decision", f"id={did}", f"question={q}", f"title={title}",
            f"decision={dec}", f"alternatives={alts}", f"reason={reason}",
            f"evidence={ev}", f"author={author}", "status=active"]
    if sup:
        args.append(f"supersedes={sup}")
    run(*args)

run("supersede", "from=D-CANON-001", "to=D-CANON-002",
    "reason=§56 freeze: decision re-recorded with question/alternatives/reason/evidence/author/timestamp")

print("SEED-56 COMPLETE")
