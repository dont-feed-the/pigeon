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

An independent participant is retesting the walkthrough with normal
binaries. Earlier disaster and refounding usability results are in
[usability-5.md](usability-5.md); those older results do not validate this
new exchange. No timed soak or sustained nightly evidence is claimed.
