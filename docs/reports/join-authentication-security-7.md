# Joining/signing review: authentication before waiting

This partial M7 checkpoint follows the [exchange lifetime correction](join-deadline-security-7.md).
It addresses the first recorded review finding; application connection
limits, failed-join ownership races and the broader audit remain open.
No elapsed or M6 hardware/resource gate is inferred from these tests.

## Defect and correction

The invitation welcome, recovery welcome and recovery bundle paths acquire
the group busy lock before checking the request's proof or recovery identity.
An unauthenticated network identity can therefore queue on a busy repository
even though the eventual answer must refuse it. The separate bounded header
read does not bound this application wait.

The regression holds a real group's busy lock. The old invitation and
recovery paths both fail the 500 ms refusal bound; the initial three-case
run has two failures and one passing revocation fence in 0.535 seconds.

Invitation preflight now checks the current invitation hash and HMAC bound
to the authenticated caller and local admitter before waiting. Unknown
recovery identities, wrong roles and invalid recovery additions are also
rejected before that wait; bundle requests require a current recovery key
and a local custodian. These early checks can only reject. All proofs,
membership, roles and the proposed addition are read and checked again
under the busy lock before signing, adopting or exporting anything. No
preflight snapshot grants authority after a wait.

## Validation

The three focused cases pass in 0.128 seconds after the correction. They
cover unknown invitation hashes, matching hashes with invalid proofs,
unknown recovery identities and bundle requests while busy, and a recovery
identity revoked while its valid request waits. A fourth case queues a
valid idempotent invitation request, removes its device from the current
roster, then releases the lock: the old roster cannot authorize the reply.
All 173 daemon-library cases pass in 17.626 seconds.

Current main, including the 0.6.24 release commit, is integrated before the
broader native validation and PR checks. All six native invitation, retry and recovery-sheet cases pass in
120.024 seconds after rebuilding the CLI, daemon and helper. This includes
recovery of an admin from a non-admin's backup, revocation of the used
sheet and refusal of an unknown key. Workspace all-target Clippy and
repository checks pass. These native flows are separate from the isolated
cached-roster lock fixtures.

## Publication checkpoint

v0.6.23, source `4ed9591db9b26337a80461b90eb884fddd9163b3`, was
published at 04:48:41 UTC on 18 October 2026 and independently verified:
six assets, four archive checksums, native Linux install, completions and
the pinned `10.20261006-pigeon1` annex. Its release lead is 803 seconds;
the interval from v0.6.22 is 3,803 seconds. Both meet the recorded KPI targets.

Its first tag CI health case fails after 85.250 seconds waiting for Bob's
second working version. Artifact 12124799438 is 12,356,798 bytes, SHA-256
`062e1603bfb2a0d133da0f070a4b60de350e1712d316ad445302a1aeeeebc9e8`.
The source and backup retain both independently hash-valid objects; Bob
has accepted the new main but retains only the first object and bytes.
No capture intent remains. I/O pressure is high at 04:37:03 (14.4%) and
04:37:28 (28.95%), with no quiet transition recorded before the failure at
04:38:28. The exact internal transfer state is not established. This
differs from the earlier PR #91 observation where Bob had both objects.

One unchanged-source retry of that failed shard passes. Publication alone
is then retried once using both already-built archives; all tag gates and
independent install/asset checks pass. No assertion, deadline, file/hash
guard or published tag is changed. The original fixture/logs are retained;
neither original can receive another retry.
