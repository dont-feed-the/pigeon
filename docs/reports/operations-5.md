# Report: operator dashboard measurements

M4 progress, version 0.4.13, 16 October 2026. Home remains the user safety
dashboard. This patch extends the optional Grafana dashboard with measured
acceptance/protection delays, actual batch depths and connection-path
observations. It does not claim M4 or a release gate.

## Measurement boundaries

Capture acceptance is recorded after successful local CAS or peer
acceptance, excluding skipped commits. New peer metadata is bounded to 256
records per update, with truncation counted. Protection requires a current
path/key first observed unprotected, then protected in this process.
Already-safe startup content is excluded. The sample keeps its immutable
ref and observation time while the group completes its round and resolves
the checked catalogue origin. Read-only inventories can still export
during offline-peer retries. Missing inventories do not become zero.

Clock skew is counted, not rendered as a zero-second successful lag.
Four core laws run 1000 cases each over the full clock range, independent
milestones, duplicate suppression, distinct versions and fixed capacity.
Each group/process tracks at most 4096 timing identities, 4096 waiting
identities and one bounded queued snapshot. Excess diagnostic timings can
be omitted. Restart begins new diagnostics, without rewriting safety state.

Queue gauges describe actual constructed work for capture, peer history,
protection and downloads, with separate stage labels and existing deferred
capacity. Guards clear on every exit. They are not a claim that the future
adaptive scheduler or scrub/rebalance queues exist. Short batches can be
missed between exports. Path observations use iroh 1.3's selected path at
admitted stream opening. Reuse counts again; later migration is not
attributed and throughput remains path-unknown. File names/keys are not
metric labels.

## Verification and findings

The initial new test used a fixture home name as the machine name; selecting
the actual device identity corrected that fixture. The offline experiment
then found that the old collector sampler skipped every busy group,
starving counts during long offline-peer retries. Counts now use the same
read-only immutable inventory as status. Protected origin resolution is
queued for the end of the group round, preserving its actual observation
time without an additional content scan. The first combined measurement
replay remained incomplete before that correction; it is not a claimed pass.

The reference `grafana/otel-lgtm` stack starts locally from verified digest
`sha256:57c04cbd8e7022f32bb84c4ac005d79baf8b08e2c5be44facf11f5f1f9944ba2`
(image reports v0.36.0). Its ports bind to loopback. Grafana accepts the
dashboard and Prometheus parses all ten queries. Empty query results at
this setup stage are not data acceptance evidence.

The acceptance metadata reader initially used `git log`, which the command
allow-list correctly refused. It now uses permitted `rev-list` metadata.
Verbose trace traffic also crowded cumulative metrics out of the bounded
outbound queue. New metric snapshots now supersede older queued snapshots
for their group, are sent before traces, and survive trace-only eviction.
A regression floods the queue before and after a metric and proves that
the latest snapshot stays first, within 256 KiB, without crossing groups.

Both real-device telemetry tests passed in 121.64 seconds: collector consent
and disable, offline unprotected sampling, exact Free/Keep bytes, peer
acceptance, sampled protection, selected direct paths and all four actual
batch classes. The backend received actual SDK payloads, not constructed
metric fixtures. LGTM accepted 5 metric and 118 trace payloads. Lag count
and bucket series exist. A strengthened regression passed both tests in
152.19 seconds and required two distinct genuine SDK export timestamps for
both lag measurements. LGTM accepted its 7 metric and 183 trace payloads.
All ten dashboard queries parse; nine return real series, including four
throughput series and all implemented batch classes. Clock skew is absent
because this run had no skew; no artificial zero was injected.

Lag panels use cumulative p95 since process start so the first observed
save in a quiet group remains visible. The imported SDK buckets yield
48.75 seconds for peer acceptance and 98.75 seconds for observed protection
in this run; these are bucket estimates, not exact event durations. Rate
panels use successive exports. Diagnostic tracing respects the selected log
level while the separate optional console can still enable runtime traces.

The five independent normal-build usability sessions used a frozen matching
0.4.13 interface candidate before the operator-only metadata/queue follow-up.
Their exact bytes and safety results, and the UI/join failures they found,
are recorded in the separate M4 usability report. They do not validate this
final diagnostic exporter binary. Full all-target lab/trace clippy and
repository checks pass. All workspace targets and doctests pass across
the full run and a targeted continuation. The initial run stopped in the
slice when accumulated generated test fixtures pushed the host below its
backup reserve. After removing stale generated roots, the slice and all
remaining integration targets passed, as did the open-pending-save safety
regression and workspace doctests. The affected failure is recorded rather
than presented as a single uninterrupted green run.

A local real-binary S11 reproduced the hosted 0.4.1 disk-budget failure:
about 11 MiB of diagnostic files beside 16 MiB of kept content. The default
local spool now rotates within 256 KiB, carried from 0.4.1 through the stack.
This keeps diagnostics within the steady single-copy budget; a selected
collector can retain longer history. Repaired real S11 passed in 74 seconds:
16 checks, no failures, one platform skip with local directory isolation.
Linux namespace and macOS image runs remain the hosted release gates.

Chromium rendered the imported Grafana dashboard and both real lag charts
with no browser errors. The reference stack was stopped and removed after
the actual query and screenshot evidence was saved. No backend remains
running as an undeclared side effect of this acceptance test.
