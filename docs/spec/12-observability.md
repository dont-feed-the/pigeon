# 12. Observability

> Mission: specify the three channels through which pigeon explains what it
> did: logs for people (ADR 0022), telemetry for diagnosing a distributed
> group (ADR 0025), and trace events checked against the TLA+ specifications
> (ADR 0008). They are kept separate: logs and telemetry may change freely;
> trace events are a checked interface.

## 12.1 Logs (ADR 0022)

- All instrumentation uses `tracing`. Spans cover each loop of `pigeond`
  ([08 §8.1](08-custody-loops.md)), each repository, peer and git or git-annex
  command, with fields such as `repo`, `device`, `key`, `path`, `commit`
  (`#[instrument]`).
- **Levels.** `error`: a person must act. `warn`: degraded but handled (a peer
  unreachable, a refused `REMOVE`, a retry). `info`: a state change a user
  could see (captured, accepted, protected, installed, freed). `debug`:
  protocol detail. `trace`: wire level.
- Only binaries install a subscriber: `tracing-subscriber` with an `EnvFilter`
  from `PIGEON_LOG` (default `info`); human-readable lines on stderr (the
  journal of the systemd unit, or launchd's log); JSON lines in `<state>/logs/`
  through `tracing-appender`, non-blocking, one file per day, seven kept
  ([11 §11.2](11-local-state.md)).
- **Redaction.** Types holding keys, tokens or invitation secrets have a
  `Debug` and `Display` that print `[redacted]`; a test serialises every such
  type and checks it. File names are logged (they are the person's own data on
  their own machine); `pigeon debug bundle` asks before including them.
- Logs are never parsed as traces (ADR 0022).

## 12.2 Telemetry (ADR 0025)

- **One pipeline.** `tracing` spans and events, plus OpenTelemetry metrics,
  exported as OTLP by the `pigeon-telemetry` crate, the only crate that
  depends on `opentelemetry*` (0.33, with `tracing-opentelemetry` 0.34),
  pinned to one minor version. This supersedes ADR 0022's "no OpenTelemetry
  in v1".
- **Correlation.** Every span carries natural IDs (device, repository, commit,
  annex key, proposal, decision) and the Lamport clock of §12.3. W3C trace
  context crosses every boundary pigeon owns: CLI to `pigeond` requests (a
  `traceparent` field of the request), the environment of child processes
  (`TRACEPARENT`), the join exchange, and the header of each relayed iroh stream
  ([09 §9.8](09-network-and-joining.md)).
- **Where it goes.** By default only to a bounded local OTLP-JSON file
  (`<state>/telemetry/`, at most 50 MB, oldest dropped first **[no ADR: bound]**).
  `pigeon debug bundle [--from NAME…]` collects logs, telemetry and, when on,
  trace events from this and chosen peers over `pigeon/debug/1`, after asking
  each peer's person **[no ADR: consent on the peer]**, into one archive.
- **Telemetry device.** If the roster names `telemetry_device`
  ([04 §4.2](04-roster-and-authority.md)), usually a custodian, the other
  devices ship OTLP to it over `pigeon/telemetry/1`, and it forwards to a local
  back end. Nothing leaves the group unless a member configures an external
  OTLP endpoint on their own device (`[telemetry] endpoint` in `config.toml`).

### Metrics

| Metric | Kind | Labels |
|---|---|---|
| `pigeon.queue.depth` | gauge | `class` (the queue classes of [15 §15.2](15-resource-use.md)) |
| `pigeon.transfer.bytes` | counter | `direction`, `peer`, `path` (direct, relay) |
| `pigeon.annex.call.duration` | histogram | `command`, `outcome` |
| `pigeon.versions` | gauge | `state` (local, published, protected), `uncertain` |
| `pigeon.sync.lag` | histogram | time from a capture to its acceptance on each peer |
| `pigeon.protect.lag` | histogram | time from a capture to *protected* |
| `pigeon.backoff.seconds` | counter | `cause` (disk, network, battery, metered, paused) |
| `pigeon.process.cpu`, `pigeon.process.memory` | gauge | — (the budgets of ADR 0037) |

### Dashboards

- **For people**, the web interface shows group health from replicated state,
  without any telemetry stack (P6): devices and last contact, counts per
  state, repair epochs, capacity, pending suggestions (ADR 0025;
  [06](06-status.md), [10 §10.3](10-interfaces.md)).
- **For developers and operators**, the reference back end is
  `grafana/docker-otel-lgtm`, with dashboards kept as JSON in
  `ops/dashboards/`: sync lag, git-annex call latency and failures, transfer
  throughput, relay versus direct connections, protection progress, queue
  depths, back-off time. Any OTLP back end works; `otel-desktop-viewer` or
  Jaeger serve locally; `tokio-console` is behind a cargo feature in debug
  builds.

## 12.3 Trace events (ADR 0008)

Trace events let a run of the real system be checked against the TLA+
specifications ([13 §13.3](13-verification.md)).

- **Vocabulary.** Each protocol's TLA+ specification defines its events:
  action names, variable names, value encodings. The Rust side has one typed
  enum per specification, one variant per action, in `pigeon-core::trace`, behind
  the cargo feature `tla-trace`. A test parses the `.tla` file and checks that
  the names match.
- **Format.** NDJSON, one line per spec action, written in `<state>/traces/`:

```json
{"spec":"CaptureInstall","device":"3f9a…","clock":1042,"action":"InstallStep","vars":{"path":"p1","step":"RenameIn","key":"k7"}}
```

- **Lamport clock.** Each device keeps a counter (`clock.json`,
  [11 §11.3](11-local-state.md)), incremented at every event; every message to
  a peer carries it (the stream header and the join exchange), and receiving
  one sets the counter to max(local, received) + 1. Logs from several devices
  are merged in clock order, ties broken by device ID, which gives an
  interleaving consistent with causality.
- **git-annex at the boundary.** git-annex is not instrumented. pigeon emits
  an event for each git-annex command it issues and its JSON result, and the
  test harness snapshots which devices hold which keys (`whereis --json`)
  between steps.
- **Specifications with events** (ADR 0008 asks at least the first five):

| Specification | Actions traced |
|---|---|
| `CaptureInstall` | `Save` (inferred from the scan), `Capture`, `InstallStep`, `Crash`, `Recover`, `Decide`, `Resolve` |
| `Authority` | `Receive`, `Validate`, `Merge`, `Propose`, `Adopt`, `Reject` |
| `Evict` | `FreeFolder`, `KeepFolder`, `DropVerified` (lock, check, drop) |
| `Purge` | `Request`, `Cancel`, `Release`, `DropReleased` |
| `Repair` | `MarkDead`, `Want`, `Get`, `ScrubFail` |
| `CustodyTokens` (existing) | the hand-off events of the existing model |

- In production the feature is off; a person may turn it on (`PIGEON_TRACE=1`,
  with a build that has the feature) to attach a trace to a bug report
  (ADR 0008) **[no ADR: switch name]**.
