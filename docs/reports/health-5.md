# Report: group health without a telemetry service

M4 progress, version 0.4.8, 16 October 2026. This patch supplies the user
dashboard of ADR 0025; it does not declare M4 complete.

## Behavior

Home keeps its single headline and shows current-file counts separately
from distinct retained older content keys. Counts preserve missing,
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

Independent normal-build browser sessions are in progress. Their findings
and retests will be recorded before this patch is marked ready.

## Remaining M4 work

The optional operator dashboard still needs lag, connection-path and
queue evidence and its debug console feature. Folder ownership needs a
supported setup flow. The prior recovery, export and threshold reports
remain separate evidence; this health patch does not replace them or any
release gate.
