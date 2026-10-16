# M4 recovery candidate: sheet authority

> Mission: record the disaster-recovery correction and its evidence.

Candidate 0.4.2, 16 October 2026. This is part of M4; it does not declare
that milestone or its remaining interface work complete.

## Authority

The previous recovery exchange authenticated the sheet through QUIC but
asked the backup to sign the new roster entry. An admin-owned backup could
sign that entry; another member's backup could not. The new join-history
check correctly rejects that unauthorized history.

Recovery now uses the sheet as specified by ADR 0047. A fresh computer
fetches metadata through a separate authenticated, read-only exchange,
checks the expected genesis and the complete signed history, and signs
its own device addition with the sheet key. It serves that signed commit
until the custodian has pulled it. The custodian checks the signer, exact
requested roster change, current parent and normal authority before moving
main. Ordinary annex streams still exclude recovery devices.

The transfer streams through private temporary files, with a 64 MiB limit
and a 120-second deadline. Files arrive separately through annex. The
sheet's secret is never saved by the recovered daemon. Both endpoints need
the new exchange; the old custodian-signed protocol is refused.

## Verification

A real-daemon disaster test created Alice's admin laptop, Bob's member
laptop and Bob's backup as the sole custodian. After two file versions and
the sheet reached that backup, both original laptops were stopped. The
fresh laptop recovered exact current bytes and both historical versions.
The recovery commit verified to the sheet's public key. The test checked
that its private key was absent from the recovered home and repository,
revoked the used sheet, waited for the backup to learn that revocation,
and confirmed a second recovery failed. The passing run took 108 seconds,
including network waits for the stopped laptops.

The first implementation run exposed a reply lock held too long and was
stopped. The next runs exposed a missing bundle command in the checked git
driver and a refusal that closed before delivery. Those defects were
corrected before the passing run. The driver permits only bundle creation
and head listing; importing uses the existing checked fetch into peer refs.

## Independent normal-binary disaster drill

An independent participant followed the guide with normal 0.4.2 binaries.
Only Bob's non-admin-owned backup survived. Recovery completed in 0.810 s;
exact current, older and deleted file bytes matched. The used sheet was
revoked, its replacement was printed, and the old sheet was rejected in
0.267 s. Every fixture daemon was stopped and secret files remained private.

Restore and removal operations waited about 20 s without intermediate text.
The following 0.4.3 interface patch now prints the ongoing action, displays
paper keys without last-heard status, and uses revocation wording. It also
includes verified archived versions and deleted files in ordinary history,
restore and view after refounding, without changing the live roster fold.
Tests cover nested refounding, archive substitution, an unauthorized archive
tip and a file deleted before refounding. Independent focused retests of
these changes are in progress.

Earlier disaster and refounding usability results are in
[usability-5.md](usability-5.md). No timed soak or sustained nightly evidence
is claimed.

## Diagnostic consent retest

The normal 0.4.4 build passed an independent French browser session with
Alice and Bob as actual daemons. Unchecked consent was refused in the
browser; a bypassed checkbox returned 400 and a forged form returned 403.
Neither granted permission. A deliberate local grant allowed one remote
part in a private (0600) local bundle; a second request was refused. All 51
exported remote log records belonged to the shared group; an unrelated
group's filename canary was absent. No bundle file appeared on the peer.

The session also found a real size failure: after a few minutes, the last
500 OTLP batches exceeded the 2 MiB transport cap. Fresh consent was then
consumed before the server refused the oversized snapshot. Peer parts now
retain complete recent records within section byte budgets and report
omitted records explicitly. The real two-daemon regression populates over
3 MiB of group telemetry, grants permission again, verifies a successful
bounded part with its positive control, and verifies one-use enforcement.
It passed in 5.15 seconds. A focused repaired normal-build retest follows.

A separate real-terminal test answered no to `debug allow`: `Cancelled.`,
exit 0, and no daemon connection. Non-terminal and `--no-input` uses kept
the confirmation error and also made no connection. The French permission
result now explicitly says “une seule fois” and that it expires in ten
minutes.
