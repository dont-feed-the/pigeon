# M4 usability: five independent sessions and repairs

16 October 2026. The user explicitly accepts agent participants as equivalent
to real people. Five independent agents used actual normal-build CLI and
Chromium sessions without reading the implementation. Two were newcomers;
three disclosed earlier pigeon usability familiarity. Fixture preparation
was separated from participant actions. This is task evidence, not a field
trial or a longitudinal warning-frequency study.

## Frozen candidate and protocol

All participants used matching normal 0.4.13 binaries. The daemon's SHA-256
was `fa4b440c20204578efd45bf7fb2a92f0c0b4641942f31f8703c8b2375d75b1b6`.
This interface candidate preceded the final operator-only metadata/queue
follow-up; it does not validate that final diagnostic exporter binary.
Each read the current README and guide, used genuine invitations and real
backup processes, and recorded unassisted/assisted completion, elapsed
interaction time, errors, ease on a 1–7 scale and a three-state teach-back.

The seven tasks were: judge IMG_2201.jpg safety; explain real local,
shared-without-backup and protected states; free Photos/2019; restore an
older budget.xlsx while retaining newer history; restore an accepted-deleted
nested Recettes folder; inspect and decide an ordinary person's suggestion;
and join a genuine invitation on another computer. All recipe deletions
were explicitly accepted and absent from the held queue before restoration.
Canceling a held deletion did not count as recovery.

| Participant | Earlier familiarity | Full tasks | Older budget / deleted folder | Proposal inspection | Join |
|---|---|---:|---|---|---|
| First use | Returning | 5/7 | Both pass | Blocked; cautious decline works | History verification refused |
| Recovery | Returning | 6/7 | Both pass | Blocked; cautious decline works | CLI pass; browser-driver label limitation |
| French web | Returning | 5/7 fully in browser | Both pass | Blocked; decision works | Browser pass |
| Newcomer A | None | 6/7 | Both pass | Blocked; cautious decline works | Browser pass |
| Newcomer B, French 390px | None | 5/7 | Both pass | Blocked; cautious decline works | History verification refused |

Scoped Free succeeded functionally for all five, with actual local file and
annex-object absence and exact protected copies on backup machines. The
nested browser page lacked the action, so documented CLI fallback does not
count as a browser-only success. Four independent participants also found
that the parent wrongly promised intentionally freed children would arrive.
The first-use session measured 4,509,696 allocated bytes actually released.

All five restored exact prior spreadsheet bytes and independently opened
the newer version from retained history. All five restored exact accepted-
deleted nested files. All five distinguished a new local edit from older
protected versions, an ordinary peer from a required backup, Sending from
Safe, and local restoration from completed group sharing/protection. No
incorrect safety belief, including a transient one, was recorded.

## Thresholds and limits

The protocol's 4/5 teach-back and 4/5 old-version/deleted-folder criteria
were met by 5/5. The zero incorrect-safety-belief criterion was also met.
The existing 10-minute work grace, 1-hour stale check and 24-hour excluded
backup thresholds remain provisional defaults, supported by this teach-back
and the earlier scenario interviews in [usability-5](usability-5.md).
No accelerated clock is described as an actual hour/day field observation.
Interaction times include agent decisions, orchestration and normal waits;
they are not estimates of human reaction time. Task 4 times ranged roughly
12–40 seconds; Task 5 roughly 7–58 seconds. Prior docs and facilitator setup
were excluded, with interleaved-state timing limitations disclosed.

The candidate did not pass the entire protocol: all five lacked proposed-
content review, and two fresh browser joins refused valid decision history.
Failures were preserved without retry, update, source inspection or an
assisted rescue. Transient Home/history disagreements converged naturally;
a separate unchanged paused-state recheck stayed consistent for 67 seconds.
They are not reported as persistent pause failures. Files also looked empty
while a new local-only file was pending; Home correctly named that file.

## Repairs in 0.4.14

- Nested folder pages now have checked Free/Keep forms carrying the exact
  folder path. Parent arrival counts exclude intentionally freed children.
- Suggestions offers read-only copies of the actual changed proposed paths;
  review never installs them over accepted files. Deleted proposed paths
  are explained explicitly. Unknown proposals and unchanged paths are refused.
- Join fetches the separately retained signed proposals required to verify
  decisions on main. Only valid proposals are kept. Provisional unknown
  judgements remain retryable when metadata arrives, rather than becoming
  permanent refused catalogue entries. Signature and permission checks stay.

Real regressions pass for an unknown decision becoming verifiable after
its proposal arrives, exact read-only proposed bytes with accepted bytes
unchanged, nested Free/Keep with no false arrival promise, and a fresh real
join after a decline. Router tests cover exact scope, CSRF and conflicting
review selectors; request framing laws cover the new request. EN/FR bundle
and browser-router checks pass.

Both independent focused 0.4.14 retests passed without assistance. The
first-use participant freed the nested folder in the browser (30.403 s,
4,509,696 allocated bytes released), reviewed two successive exact proposed
CRLF versions without changing accepted bytes (64.241 and 81.054 s), declined
both, then joined a fresh laptop on its first attempt (33.803 s). Ease was
7/7 for each task; 46/46 external checks passed. The French newcomer freed
the exact nested scope in 7.8 s (object absence verified at 16.8 s), reviewed
actual proposed 20h text while accepted 19h text and history stayed intact,
and joined a fresh computer after decline: membership in 1.6 s, exact bytes
and Safe in 18.2 s. The old failed fixtures were unchanged. Both retests
closed and independently verified all their owned processes; neither found
an incorrect safety belief. The French singular copy “1 fichiers” remains
a minor wording issue.

These focused retests validate the repairs and do not replace or convert
the original failures into first-pass successes. Paper-key and archive
recovery are also being checked for the same decision-metadata dependency.
This report does not yet declare the M4 release complete.

## Evidence handling

Detailed task tables, own-word teach-backs, screenshots, byte manifests,
private transcripts and the two failed join fixtures are retained locally.
Invitations, login tokens and keys are excluded from this published report.
All five original sessions stopped their owned daemons and Chromium and
verified cleanup under a child subreaper. Harness-only locator/cache/close
errors are disclosed separately from product failures; no unrelated fixture
was changed or stopped.
