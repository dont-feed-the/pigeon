# Joining/signing review: exchange lifetimes

This is a partial M7 review checkpoint for ADRs 0007 and 0075. It does not
close the security-review gate, all planned-law/trace coverage, or any
elapsed release gate. The separately recorded schema/proof checkpoint in
[join schemas](join-schema-7.md) remains relevant.

## Scope and trust

The transport authenticates the remote Ed25519 device identity. Invitation
admission separately requires the group's secret, with HMAC bound to the
new and admitting device IDs. A transport identity alone is not group
membership. The invitation is intentionally a bearer capability: its
holder may join an existing person's identity, including an admin, after
confirmation. That is ADR 0075's decision, not an authentication defect.
A removed device cannot rejoin as that device; refounding changes the
invitation. Stolen unlocked signing keys remain outside ADR 0007's model.

The review follows `net/serve.rs`, `net/join.rs`, `invite.rs`, `join.rs`,
`join_attempt.rs`, `verify.rs`, the recovery exchange and the signature
adapter. `pigeon-core::sig::verify` requires one SSHSIG, the `git` namespace
and an Ed25519 key before verifying the exact payload. Authorization is
then judged against parent rosters. `verify::bootstrap` checks the expected
genesis, welcome ancestry, full accepted history, and membership at both
welcome and tip before adoption. These source observations are not a
comprehensive independent cryptographic audit.

## Concrete lifetime defect and correction

The old invitation dial checks its ten-minute deadline only between
iterations. Its individual `Node::join` awaits connection, bidirectional
stream, write and reply without a shared deadline. A peer that receives
the request but never replies prevents the loop reaching its deadline or
trying the next admitter. The server also waits without a deadline for a
join connection to open its first stream, despite bounding the subsequent
header read to ten seconds.

The real QUIC silent-admitter regression first fails after its four-second
outer safety bound: the old exchange never returns. The corrected exchange
uses a single 30-second production deadline across connection, stream,
write and reply. The invitation dial separately applies its existing
absolute ten-minute deadline to each await, and bounds its retry sleep by
that same deadline. A slow admitter may still have signed an idempotent
welcome before the next attempt; the existing UUID/identity replay checks
remain unchanged.

An owned connection guard closes the established QUIC connection on
success, timeout, other error or caller cancellation. The server now bounds
the initial stream wait by the existing ten-second header budget and closes
an idle join connection. No join schema, proof, signing rule or refusal is
relaxed.

## Validation

The silent-admitter case passes after the correction in 2.021 seconds
using a two-second test exchange budget. It establishes a real QUIC
connection, reads the complete request, deliberately retains the reply
half without responding, and checks both timeout and remote closure.
Separate native cases check closure after caller cancellation and the
server's actual ten-second idle-stream limit. All 169 daemon-library cases
pass in 17.941 seconds.

After rebuilding the CLI, daemon and helper, all five invitation and retry
cases pass in 4.819 seconds, including removed/refounded invitations,
unknown versions, all altered HMAC positions, idempotent identity/UUID
admission and failure cleanup followed by retry. An initial local invocation
used `/tmp`, an unsupported tmpfs, and all five stopped at group creation
before any join; correcting TMPDIR to the existing native fixture location
gives the recorded passing run. Those creation refusals are not a network
regression or a passing join result.

## Findings still requiring work

The original finding about unauthenticated requests queuing on a busy group
is addressed by the [authentication checkpoint](join-authentication-security-7.md),
with refusal-before-wait and revocation-during-wait regressions. The wider
review remains open:

- The accept loop spawns per-connection work without an explicit application
  concurrency cap. Transport limits and timeouts need to be assessed together
  before claiming resistance to an unauthenticated connection flood.
- The [backup cleanup checkpoint](backup-join-preservation-security-7.md)
  reproduces deletion of concurrent additions and retains incomplete backup
  repositories for retry. Laptop rollback ownership and path substitution
  still require preservation review before the wider gate closes.
- Invitation/default-folder validation, recovery import bounds and the full
  signature/authority law and negative-trace inventory remain in scope.

These are recorded review findings rather than claims of a reproduced
exploit, data loss or complete remediation. The focused exchange correction
can ship while the broader gate remains open.

## Integrated release validation

PR #93, head `1dceda17f7d922bca957825c7558408ce15069e8`, integrates main `4ed9591db9b26337a80461b90eb884fddd9163b3` before
validation. CI 39097819823, lab 39097819810 and bridge
39097819829 pass. v0.6.23 was published and independently
verified before this candidate. The complete joining/signing review and
M6/M7 resource and elapsed gates remain open. Fresh v0.6.24 tag gates and
independent publication/install checks are required before it is released.
