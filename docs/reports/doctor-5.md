# Report: checking the complete installation

M4 progress, version 0.4.11, 16 October 2026. Independent folder-rule
usability found a test-package omission: the frozen binaries lacked the
transport helper, so creating a group failed. The participant diagnosed it
through the JSON error, while `doctor` incorrectly said everything was in
place. Adding the matching helper corrected the fixture prerequisite.

The product now probes `git-annex-p2p-iroh --version` in git-annex's child
environment, with a five-second deadline and child termination on timeout.
The version command works without a running daemon or repository. Missing,
failed or mismatched helpers give a localized installation problem and a
reinstall remedy. Any doctor Problem suppresses its final reassurance.

An isolated real-process test checks absent, stale, matching and failed
executable fixtures without changing the process-wide PATH. An independent normal-build no-group replay confirmed standalone helper
version output, normal doctor success, and actionable missing/stale helper
failures in 17–31 milliseconds with a nonzero CLI exit. Restoring the full
package restored success. A French locale produced the translated remedy.
No failure included the everything-in-place reassurance. All fixture
daemons and Chromium were stopped before the final browser replay.

The replay also found that Settings had no browser entry for Doctor,
despite the interface specification. Settings now links to the same
read-only request; it uses normal login/host checks, renders localized
remedies and omits raw diagnostic details. The independent final Chromium replay passes missing and stale helpers
and the repaired complete package through Settings. French error results
appear in 0.281–0.325 seconds and remain identical after 2.5 seconds and a
reload. Repaired installation gives the success reassurance in 0.320
seconds; CLI agrees in every case. All owned daemons and Chromium were
stopped and verified absent.

The isolated process regression, all 23 web tests, all 12 i18n tests,
warning-free full-workspace Clippy and repository checks pass. Browser
routing tests check authentication, localized remedy and omission of raw
private details. This patch does not claim M4 or a release gate.
