# Report: group health without a telemetry service

M4 progress, version 0.4.8, 16 October 2026. This patch supplies the user
dashboard of ADR 0025; it does not declare M4 complete.

## Behavior

Home keeps its single headline and shows current-file counts separately
from other retained content keys, including earlier, deleted and suggested
versions. Counts preserve missing,
erased, local, published, uncertain and protected states. Pending saves
replace the older state of their current path. Retained bytes remain in
the headline even after their file is overwritten or deleted.

The dashboard shows pending suggestions, endangered keys, current
machines, their contact times, and the capacity and queues in their latest
admitted signed reports. Each report has its own age and freshness;
contact never refreshes a disk measurement. Paper keys are identified
without inventing storage or an offline computer. Effective accepted
computer removals supply repair epochs with their names and causal
commits. No OTLP service or telemetry permission is needed.

Home updates text every five seconds. Group, membership and repair-history
changes reload the page. Names are escaped, live data never becomes HTML,
and action results do not subscribe to Home's stream. Current-file
explanations stop at 200 with an explicit omitted count; aggregate counts
still cover the inventory. Missing inventories or suggestion counts stay
unknown rather than becoming zero.

## Verification

- Eight core laws have 1000 cases each: saturating count combination,
  exact state partition, pending precedence, and the canonical effective
  removal projection, including skipped edits and paper keys.
- A 1000-case wire law preserves Unicode, arbitrary counts, missing
  inventories and report metadata across split socket reads. Existing
  request framing laws include `Health`.
- Router tests check both inventories, stale capacity despite fresh
  contact, a one-byte send queue, HTML-escaped names, text events, omitted
  explanations, French unknown values and membership changes.
- A real two-daemon test protects two successive versions, then injects
  a correctly signed checking report from the stopped backup. Both
  inventories stop counting its copies as protected. Breaking the local
  accepted ref produces Needs you and two unknown inventories; the
  working file retains its exact bytes. An initial fixture mistakenly
  created an existing ref; its corrected compare-and-swap test passes.
- The open-writer pending-save regression also passes. The English and
  French message catalogues pass their coverage checks.
- A second real three-daemon regression removes the backup and checks
  that another laptop's retained-history evidence means shared, not
  local-only. Per-key retaining heads determine publication; unrelated
  proposals cannot withhold evidence for an already shared older key.
- A backup counts its own locally observed stored copies only with its
  admitted custodian role and the expected annex UUID. Both current and
  retained inventories pass this real two-daemon check. Checking reports
  also exclude its own copies until checking finishes.

## Independent normal-build sessions

Two independent participants used real Chromium, ordinary daemons and
isolated groups, including 375/390-pixel screens, without inspecting the
implementation. The user accepts these agent sessions as usability
participants. The initial pass found stale file explanations after a safe
live headline, ambiguous permanent-erasure wording, a misleading empty
group headline, file counts that included several versions of one path,
and a local-only claim for history held by another laptop. Each was fixed
and retested in the actual browser. Current, deleted and suggested
versions now remain distinct in the labels and counts; changing counts
and their explanations update together. Exact current, older, proposed
and read-only view bytes survived all sessions. Neither narrow screen
overflowed. Refounded-group confirmation uses the actual Join button in
French and has no terminal prompt.

The first-use participant also joined a replacement backup after a
removal. An initial 30-second observation was inconclusive; a later
149-second replay proved a transport problem despite a clean doctor.
The old short socket alias exceeded the Unix socket path limit in this
long home. Per-registration 0700 temporary directories now keep aliases
short and prevent older helper cleanup from removing newer aliases. A
real Unix connection regression checks long paths, permissions and
overlapping registrations. Restarting the same preserved three-machine
fixture once with the repair made both laptop inventories safe within
6.16 seconds, cleared replacement work, and retained exact bytes/history.
All 25 targeted socket-session checks passed. The backup's own conservative
status discrepancy from that replay has the separate automated fix above.

The final general first-use replay passed 58 of 59 recorded checks; its
remaining replacement failure was resolved by that socket-session retest.
These are repeated task/byte/viewport checks, not separate participants.
The French participant passed all targeted final findings, including
live pending suggestions and deletion. Each complete harness ran under a
subreaper and verified process cleanup; private evidence remained private.

Minor remaining finding: a membership change reloads Home and closes
expanded details. Counts and accepted removal times are correct after
reopening them. No field-trial or long-duration claim is made.

## Remaining M4 work

The optional operator dashboard still needs lag, connection-path and
queue evidence and its debug console feature. Folder ownership needs a
supported setup flow, and folder restoration and free/keep completion
wording have their own follow-up. The prior recovery, export and threshold reports
remain separate evidence; this health patch does not replace them or any
release gate.
