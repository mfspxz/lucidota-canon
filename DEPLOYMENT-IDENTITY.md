# CANON Deployment Identity

**Status:** implemented (wave-10). All artifacts in this document stamped
**TEST-KEY — NOT FOR PRODUCTION** unless the "Production key custody"
section says otherwise. Production deployment keys are Zack-gated
(authority-key custody decision, `goal_b4eaabcc3b4d`); nothing here
invents custody.

## Problem

CANON gate 3 (verifiers/GATE-MATRIX.md, row 5) requires: "A deployment
identity exists beyond the local build." Until now the answer was
honestly absent — CANON could prove what its frozen constitution *is*
(`canon snapshot`, hash-chained stores) but a running deployment had no
verifiable way to say "I am enforcing *this* constitution, built from
*this* binary, signed by *this* operator."

## Design

A **deployment manifest**: canonical-JSON (CANON cjson v1: UTF-8, sorted
keys, no whitespace, no floats) binding a running deployment to the
exact frozen constitution it enforces, signed with ed25519.

Manifest fields (all strings unless noted):

| field | meaning |
|---|---|
| `canon_protocol` | integer 1 (CANON protocol version) |
| `constitution_commit` | git HEAD of the canon repo at build/issue time |
| `canon_heads` | object: per-store head hashes (`Canon::heads`) — the stable normative-state identity |
| `snapshot_hash` | sha256 of the canonical bytes of `Canon::snapshot()` with volatile fields (`generated_at`) stripped — deterministic for a given frozen state |
| `canon_binary_sha256` | sha256 of the issuing `canon` binary (`std::env::current_exe`) |
| `build.rustc_version` | baked at compile time by build.rs |
| `build.source_date_epoch` | `"set"` or `"unset"` at compile time (presence only, not the value) |
| `environment.hostname` | machine hostname at issue time |
| `environment.operator_name` | operator-declared deployment name (CLI arg, never inferred) |
| `issued_at` | UTC ISO timestamp (non-authoritative metadata) |
| `key_id` | sha256 hex of the signing public key (first 16 chars shown in logs) |
| `key_class` | **always `"TEST-KEY — NOT FOR PRODUCTION"` in this lane** |
| `public_key` | ed25519 public key, hex (embedded so verification is self-contained) |
| `signature` | ed25519 signature, hex, over the canonical bytes of the manifest **without** the `signature` field |

Signing input = `cjson::canonical(manifest_without_signature)`. The
signature therefore covers every field above, including the embedded
public key (self-consistent: swapping the key breaks the signature).

### Why ed25519, why embedded key

ed25519 (`ed25519-dalek` 2.x, crates.io, $0) — deterministic signatures,
32-byte keys, no parameter negotiation. The public key is embedded in
the manifest so `verify-deployment` needs no keyring: trust comes from
the *custody ceremony* (see below), not from the manifest. A manifest
whose signature does not verify against its embedded key is UNKNOWN —
it cannot be attributed to anyone.

## Verification procedure

`canon verify-deployment manifest=<path>`:

1. Parse the manifest JSON. Malformed → exit 3 (integrity failure, not
   usage: a malformed manifest is untrusted input, per the CLI convention
   0 ok / 2 usage / 3 integrity failure / 4 semantic failure).
2. Split `signature` from the payload; recompute canonical bytes;
   `ed25519_dalek::VerifyingKey::verify`. Invalid → print
   `UNKNOWN` (tampered or unknown key) and exit 3. The constitution is
   *not* consulted — an unverifiable signature attributes nothing.
3. Open the local frozen state (`Canon::open()`, fails closed on chain
   damage → exit 3, verdict UNKNOWN).
4. Recompute `canon_heads` and `snapshot_hash` locally. Compare against
   the manifest:
   - all equal → `MATCH` (exit 0): this deployment enforces the same
     frozen constitution the manifest was issued for.
   - any differ → `MISMATCH` (exit 4): the manifest is authentic but
     describes a *different* constitution than the local one.
5. `canon_binary_sha256`, `build.*`, `environment.*`, `issued_at` are
   reported as observed provenance — they are NOT part of the
   MATCH/MISMATCH decision (the verifier's binary legitimately differs
   from the deployment's).

Key class is always surfaced: a manifest with
`key_class = "TEST-KEY — NOT FOR PRODUCTION"` verifies as MATCH only
*as a test artifact*. Production tooling MUST reject TEST-KEY manifests
for production decisions — this is enforced by documenting it here and
will be enforced in code when the production ceremony lands.

## CLI

```
canon deploy-identity name=<deployment-name> key=<secret-key-hex-file>
    → prints the signed manifest (canonical JSON) to stdout.

canon deploy-identity gen-key
    → generates an EPHEMERAL TEST KEYPAIR, prints secret+public hex
      to stdout, stamped TEST-KEY — NOT FOR PRODUCTION. The secret
      key is never written to disk by canon.

canon verify-deployment manifest=<path>
    → MATCH | MISMATCH | UNKNOWN, with per-field comparison detail.
```

Unknown `k=v` arguments are rejected (exit 2), per the existing CLI
discipline — never silently dropped.

## Test evidence

`tests/deploy_identity.rs` (ephemeral keys generated in-process, never
persisted):

1. `valid_manifest_verifies_match` — issue against the frozen stores,
   verify → MATCH.
2. `tampered_manifest_is_unknown` — flip one byte of the payload,
   keep the signature → UNKNOWN.
3. `wrong_key_is_unknown` — sign with key A, embed key B → UNKNOWN.
4. `different_constitution_is_mismatch` — sign a manifest whose heads
   name a constitution that is not the local frozen one → MISMATCH.
5. `tampered_store_is_unknown` — CANON_DIR pointed at a chain-broken
   copy → `Canon::open` fails closed → UNKNOWN.

## Production key custody (DEFERS TO ZACK)

**This section specifies the handoff; it does not invent custody.**

Production deployment keys are gated on Zack's authority-key custody
decision (`goal_b4eaabcc3b4d`). When that decision lands, the ceremony
is:

1. **Operator provides:** an ed25519 keypair generated under the
   custody arrangement Zack approves. Canon never generates production
   keys; `gen-key` stays test-only by code review, not just by policy.
2. **Issuance:** the operator (or their authorized deployment
   pipeline) runs `canon deploy-identity name=<prod-name>
   key=<path>` on the production build. The manifest's `key_class`
   must read `"PRODUCTION"` and `key_id` must be published through
   the custody-approved channel so verifiers can pin it.
3. **Verification:** `canon verify-deployment` gains a
   `--require-key-id <id>` / `--require-production` flag (not yet
   implemented — build it when custody is decided) so production
   consumers reject TEST-KEY manifests mechanically, not by reading
   docs.
4. **Rotation:** a new manifest issued under the new key; old
   manifests are not revoked by canon — revocation is a custody-layer
   concern (publish the retired `key_id`).

Until then: every manifest this lane can produce is `key_class =
"TEST-KEY — NOT FOR PRODUCTION"`, and the CANON gate matrix records
the PASS with that caveat. The gate is honestly "test identity
established, production identity awaits the operator's key decision" —
not "production identity established."
