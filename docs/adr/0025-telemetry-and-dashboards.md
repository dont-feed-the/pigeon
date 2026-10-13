# 0025. Telemetry: OpenTelemetry over iroh to an optional group collector; two dashboards

- **Status:** Accepted, 13 October 2026. Chosen by the agent after a survey
  that Guillaume asked for ("debugging a distributed system is likely to be
  tricky"). Supersedes the line "No OpenTelemetry in v1" of ADR 0022.

## Context

A fault in pigeon may span a laptop, two custodians and git-annex
subprocesses, with devices behind NAT and no central server. Survey of
13 October 2026: OpenTelemetry (OTLP) is the common wire format; the Rust
crates `opentelemetry`, `opentelemetry_sdk`, `opentelemetry-otlp` are at
0.33 (pre-1.0, breaking changes between minors), `tracing-opentelemetry`
0.34 bridges our `tracing` spans (ADR 0022). Back ends that accept OTLP:
Jaeger v2.22 (traces, Apache-2.0), `grafana/docker-otel-lgtm` (collector,
Prometheus, Tempo, Loki and Grafana in one container, Apache-2.0), SigNoz,
OpenObserve (AGPL-3.0), HyperDX (MIT); `otel-desktop-viewer` for a single
developer machine; `tokio-console` for async tasks. Telemetry carries file
names and member activity, so it is private to the group.

## Decision

- **One pipeline.** `tracing` spans and events, plus OpenTelemetry metrics,
  exported as OTLP by a `pigeon-telemetry` crate, the only crate that
  depends on `opentelemetry*`, pinned to one minor version.
- **Correlation.** Every span carries natural IDs: device, repository,
  commit, annex key, proposal, decision, and the Lamport clock of ADR 0008.
  W3C trace context crosses every boundary pigeon owns: CLI to `pigeond`
  requests, environment of child processes, the join protocol, and a small
  header pigeon prepends to each relayed iroh stream (ADRs 0011, 0014), so
  one action is one trace across devices.
- **Where it goes.** By default nowhere but the device: a bounded local
  OTLP-JSON file, which `pigeon debug bundle` collects from chosen peers
  over iroh. Optionally the roster names a *telemetry device*, usually a
  custodian: the others ship OTLP to it over iroh (its own ALPN), and it
  forwards to a local back end. Nothing leaves the group unless a member
  configures an external endpoint on their own device.
- **Dashboards.** For users, the web interface (ADR 0021) shows group
  health from replicated state, without any telemetry stack: devices and
  last contact, protected / published / local counts, repair epochs,
  capacity, pending proposals (P6). For developers and operators, the
  reference back end is `grafana/docker-otel-lgtm`, with dashboards kept as
  JSON in the repository: sync lag, git-annex call latency and failures,
  transfer throughput, relay versus direct connections, protection
  progress, queue depths.
- **Tests.** Fault-injection runs export OTLP-JSON files kept as CI
  artifacts. Developers may run `otel-desktop-viewer` or Jaeger locally;
  `tokio-console` is available behind a cargo feature in debug builds.

## Consequences

- Any OTLP back end works; the reference one is a convenience, not a
  dependency.
- Cross-device timelines are subject to clock skew; the Lamport clock
  attribute orders causally related spans.
- OpenTelemetry's churn is contained in one crate.
- Telemetry and TLA+ traces stay separate (ADR 0008): telemetry is for
  people and may change; trace events are a checked interface.
