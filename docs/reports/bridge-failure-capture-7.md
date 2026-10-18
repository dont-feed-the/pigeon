# Capturing failed bridge fixtures

> Mission: retain the native metadata needed to diagnose a failed convergence
> assertion without changing its deadline or accepting missing content.

PR #87's original custodian bridge job 117549755010 in run 39084909218
fails the regular-peer offline-save assertion. Artifact 12119963498 is
12,906,833 bytes, SHA-256
`52012fcf23419fc0aad84e15a66211d6e888bb96a03e22912be4095d0b5ac36a`.
Its extracted backup repository's accepted main lacks native-offline.txt;
the other peer's retained working file has the exact expected 38 bytes.
All 206 retained DAV responses for that path are 404. Control/source/other
logs show repeated approximately 30-second metadata-fetch timeouts, not an
index.lock failure. The saved observations do not establish the exact cause.

The new failure artifact capture retained the bare source.git, logs and
traces, but the upload action's default hidden-file exclusion omitted every
ordinary repository's .git directory. Those omitted histories cannot prove
whether the regular peer captured its working file. The upload now explicitly
includes hidden fixture files. The artifact remains failure-only, contains the synthetic test group,
and is retained for 14 days. It is not collected from
a person's running installation. No assertion, timeout or protection rule is
relaxed. There is no new user-facing behavior.

Matching original binary artifact 12120019675 is 50,003,892 bytes, SHA-256
`f9a1edc85d2ded7b9e71a8d54534a064e9dfb6e30259df6769be345476f863f0`.
Its synthetic merge 10bc8863a8a7ec50cf6e68bcee673f27e76fca72 has exactly
PR head 4788c78c5ce0373539e51b8912615b00389613f2's tree. A local execution
of those binaries passes all four behavioral checkpoints, including both
offline/online histories and provider loss/restart. That first local execution
used the wrong trace environment name. A second local execution set the
trace directory but omitted the explicit enable flag. Their subsequent
no-lifecycle-traces failures are not model validation. The traced retake and any bounded CI retake
remain distinct evidence; the original CI miss is retained.
