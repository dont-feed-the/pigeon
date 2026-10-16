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

Final real-device regression, actual SDK payload ingestion/query results,
normal-build usability and final pre-commit checks remain pending.
