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

The operator dashboard currently graphs protection counts, pending bytes,
deferred keys, annex call durations and failures, and transported bytes.
Transport bytes include protocol traffic. The path label is `unknown`
until connection-path instrumentation is added; it never claims direct or
relay evidence that was not measured. Sync/protection lag, process resource
metrics and per-class ready queue depths remain to be instrumented for M6.
