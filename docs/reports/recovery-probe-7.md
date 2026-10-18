# Recovery sheet reachability probes

This checkpoint addresses a reproducible local ref-lock conflict in the
recovery-sheet prerequisite (spec 09 §9.9). It does not complete M6 or M7.

## Original hosted failure

Immutable v0.6.26, source
`3ae371976726199d72f31d9e122eeb6d961b66a8`, passed CI 39100534838 and
lab 39100534655. Slice 39100534657 failed S10 step 4 with
`recovery-backup-offline`; step 3 had passed both files' Protected checks.
The original backup logs record acceptance of the source's captured commit
and placement. The API refused before signing a recovery addition. The other recovery scenarios
passed; the original artifact is retained, without a retry:

- job 117595709259, artifact 12125969801, 49,687 bytes;
- SHA-256 `b76ce1e4262bb417b3a44e8b1cd790e755a787df2516051ab0935f4ba9051bcd`.

The old probe discarded Git's error. These logs do not establish the exact
reason both probes failed; they cannot prove a ref-lock conflict caused
this particular S10 failure. No protection assertion or deadline is relaxed.
The failed tag remains immutable and unpublished.

## Reproduced mechanism and correction

The old reachability probe fetched every head into the same cached peer
refs used by background fetches, outside the repository lock. A real Git
fixture keeps a peer online while holding its destination ref lock. The
old probe fails with Git's local ref-lock error. A second fixture shows
that, without that lock, merely checking reachability moves cached refs.
Both new regressions fail against the old probe (0.194 seconds total),
while the absent-peer control passes.

The probe now fetches only the peer's main, without a destination and with
an empty refmap. The empty refmap also disables opportunistic updates from
remote configuration. No tags or FETCH_HEAD are written. Background
fetching, authority verification, the 20-second deadline and the requirement
for a backup to answer all remain. A successful probe does not itself sign
a recovery addition or establish safe content. Errors now retain the
underlying Git failure in the daemon log for future classification.

The tests check the actual Git process, unchanged cached ref and lock bytes,
absence of FETCH_HEAD, and refusal of a missing peer.

## Local validation

The three real-Git probe contracts pass in 0.057 seconds after correction.
All 178 daemon-library tests pass in 17.794 seconds. The native recovery
fixture passes in 67.563 seconds: it recovers the latest bytes and retained
suggestions from another person's backup, with revocation and sheet-secret
checks still required. Neither a missing backup nor an offline backup is
made eligible by retrying or by weakening a status assertion.

The first additional local S10 run passes all eight functional checks in
119 seconds. Its trace check fails because the native nextest build replaced
the instrumented daemon binary with a lab-only one. The retained report
explicitly skips its ninth, no-trace check. This is functional evidence only;
a correctly ordered tracing build and fresh trace validation are required.

## Integrated release validation

PR #96, head `3c637b3cd7ffb219826fad8381b79ca84a93fe5b`, integrates main `3ae371976726199d72f31d9e122eeb6d961b66a8` before
validation. CI 39101792504, lab 39101792517 and bridge
39101792560 pass. v0.6.25 was published and independently
verified before this candidate. The complete joining/signing review and
M6/M7 resource and elapsed gates remain open. Fresh v0.6.27 tag gates and
independent publication/install checks are required before it is released.

The corrected instrumented local S10 run passes all nine checks in 63
seconds, without a skipped trace. Its four filesystem sequences and roster
sequence pass model validation in 5 seconds. This follows the retained
lab-only binary run and does not erase its missing-trace observation.
