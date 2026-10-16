# Group diagnostics

People use `pigeon open` for group health: the headline, folder and file
states, people, computers, backup machines, last contact and suggestions.
It reads the signed group state and needs no telemetry server.

Operators can additionally collect diagnostics. Spans and metrics stay in
each device's bounded `state/telemetry/` directory by default. Nothing is
sent merely because an OTEL environment variable or local endpoint exists.
Diagnostics include file names and member activity; enable collection only
on a computer trusted with that information.

An admin selects one existing computer by its group name:

```sh
pigeon settings telemetry-device 'Backup at home'
pigeon settings telemetry-device
# Stop collection for the group:
pigeon settings telemetry-device off
```

This writes a signed roster change, which replicates normally. Every device
must be upgraded before enabling it: older releases wait on an unknown
roster field (ADR 0048). Removing the selected computer disables collection.
Other groups on the same device retain their own independent choices.

On the collector only, add to its `config.toml`:

```toml
[telemetry]
endpoint = "http://127.0.0.1:4318"
```

This is the OTLP **HTTP** base URL, not the gRPC port. A local reference
backend is `grafana/docker-otel-lgtm`; bind its OTLP and Grafana ports to
loopback. Import [the operator dashboard](dashboards/pigeon-operations.json)
and select its Prometheus source. Any OTLP HTTP backend also works. An
external endpoint is used only when explicitly configured on that device.
Redirects and implicit proxy configuration are disabled.

Telemetry goes over authenticated `pigeon/telemetry/1` iroh streams. The
receiver checks the roster membership and collector choice before accepting
a body; every record must name that stream's group. It stamps the source
identity from QUIC, then writes local files and forwards to the configured
backend. Unknown devices and mixed-group batches are refused.

Local files rotate at 5 MiB and retain at most 50 MiB. The SDK queues at most
2,048 spans; the outbound group queue holds at most 1 MiB and batches are at
most 256 KiB. Offline telemetry can be discarded. Collection runs outside
capture and custody; an unavailable collector or dashboard does not change
file protection. `pigeon debug bundle` flushes and includes recent local
OTLP records.

The operator dashboard graphs current protection counts, pending bytes,
deferred capacity, annex call durations/failures, transported bytes, capture
acceptance lag, observed protection lag and actual work batches. Batch
depths cover capture, history, protection and downloads; they clear on
success or error. Short batches can fall between the SDK's samples. Scrub
and rebalance classes appear when those queues are implemented; absence of
a measurement is not a proven zero. Process resource budgets remain M6.

Acceptance lag uses the signed capture commit's wall-clock time and records
its arrival on each peer. Protection lag records a current path/key only
after this process has observed it unprotected, then protected. Sampling
is every 30 seconds and may be delayed by a busy round. Already-safe
startup inventory is excluded; clock skew is counted separately, never
converted to zero-second success. Duplicate suppression is bounded to
4096 identities per group/process; excess timings are expendable. A
restart begins a new diagnostic series. At most 256 new metadata records
are inspected per acceptance, with truncation counted explicitly.

Connection observations sample the selected iroh path when an annex stream
is admitted, on incoming and outgoing sides. Reused connections count
again. They do not attribute later migration or bytes: throughput includes
protocol traffic and retains its `unknown` path label. None of these
operator measurements replaces Home's signed-state safety judgement.

## Optional Tokio task console

For a local development session, build the matching binaries with Tokio's
unstable task tracing and pigeon's optional feature:

```sh
RUSTFLAGS="--cfg tokio_unstable" cargo build -p pigeon -p pigeond -p git-annex-p2p-iroh --features pigeond/tokio-console
PIGEON_TOKIO_CONSOLE=1 target/debug/pigeond
# In another terminal, with tokio-console installed:
tokio-console http://127.0.0.1:6669
```

Use the appropriate build directory if `CARGO_TARGET_DIR` is set. This
opens a local debug endpoint containing runtime/task activity. Stop the
debug daemon before starting the installed daemon again. The feature alone
does not start a listener; the switch also requires a debug build with
`tokio_unstable`. Release builds return no console layer. The address is
fixed to 127.0.0.1:6669 and console environment settings cannot expose it
on another interface. Events and per-client buffers are bounded; task
retention is 60 seconds. The normal JSON log level does not filter away
console events, and runtime instrumentation is excluded from the OTLP
bridge. This is independent of the group collector and changes no file
protection decisions.

Cumulative metric snapshots take priority over traces in the bounded outbound
queue. A newer snapshot supersedes an older queued snapshot for the same
group; verbose traces cannot indefinitely starve the operator measurements.

Acceptance and protection percentiles use cumulative observations since each
process started. They can display the first observed save in a quiet group;
they are not five-minute rates. Other rate panels need successive exports.
