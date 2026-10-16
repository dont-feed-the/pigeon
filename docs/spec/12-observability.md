# 12. Observability

> Mission: logs for people (ADR 0022), telemetry for the group (ADR 0025),
> and trace events checked against TLA+ (ADR 0008). Only trace events are a
> checked interface; the `trace` laws cited in §12.3 are normative.

## 12.1 Logs (ADR 0022)

- Spans cover each loop ([08 §8.1](08-custody-loops.md)), repository, peer
  and git or git-annex command, with fields `repo`, `device`, `key`, `path`,
  `commit`. **Levels:** `error`, a person must act; `warn`, degraded but handled (a
  peer unreachable, a refused `REMOVE`, a retry); `info`, a state change a
  user could see; `debug`, protocol detail; `trace`, wire level.
- `PIGEON_LOG` (default `info`) sets the level: human lines on stderr, JSON
  lines in `<state>/logs/`, one file per day, seven kept ([11 §11.2](11-local-state.md)).
- **Redaction:** keys, tokens and invitation secrets print as `[redacted]`,
  which a test checks for every such type. File names are logged; `pigeon
  debug bundle` asks before including them. Logs are never parsed as traces.

## 12.2 Telemetry (ADR 0025)

Coded so far (`pigeon-telemetry`, laws in its `tests.rs`): each round of
the loops is one trace, `round` with `group`, `device` and `lamport`, and a
child span per step (`capture`, `accept`, `install`, `place`, `repair`,
`publish`), failed steps with status 2. Spans and gauges are OTLP-JSON
lines in `<state>/telemetry/pigeond-<µs>.jsonl`, written by hand rather
than by `opentelemetry-otlp` **[no ADR: hand-written local export]**; files
rotate at 5 MB and the oldest go above 50 MB. `pigeon debug bundle` carries
the last 500 lines. Not yet: the metrics below, trace context across
processes and streams, `telemetry_device`, and the developer dashboards.

- Spans, events and metrics are exported as OTLP by `pigeon-telemetry`, the
  only crate depending on `opentelemetry*`, pinned to one minor version.
- Every span carries natural IDs (device, repository, commit, key,
  proposal, decision) and the Lamport clock (§12.3). W3C trace context
  crosses CLI requests (`traceparent`), child processes (`TRACEPARENT`), the
  join and each stream header ([09 §9.8](09-network-and-joining.md)).
- By default only to `<state>/telemetry/` as OTLP-JSON, at most 50 MB,
  oldest dropped first **[no ADR: bound]**. `pigeon debug bundle [--from NAME…]` collects logs, telemetry
  and trace events from this device and chosen peers (`pigeon/debug/1`),
  after asking each peer's person **[no ADR: consent on the peer]**.
- If the roster names a `telemetry_device` ([04 §4.2](04-roster-and-authority.md)),
  the others ship OTLP to it over `pigeon/telemetry/1`. Nothing leaves the
  group unless a member sets `[telemetry] endpoint` on their own device.

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

People see group health in the web interface, without telemetry
([10 §10.3](10-interfaces.md)); developers use any OTLP back end
(reference: `grafana/docker-otel-lgtm`).

## 12.3 Trace events (ADR 0008)

A run of the real system is checked against the TLA+ specifications
([13 §13.3](13-verification.md)).

- `pigeon-core::trace` (cargo feature `tla-trace`) has one enum per
  specification and one variant per action; each name is an operator of its
  `.tla` file (`roster_trace`: `action_names_are_operators_of_their_specs`).
- NDJSON in `<state>/traces/`, one line per action:
  `{"spec","device","clock","action","vars"}`.
- **Lamport clock** (`clock.json`, [11 §11.3](11-local-state.md)): incremented
  at every event, carried by every stream header and join message, set to
  max(local, received) + 1 on receipt; logs merge in clock order, ties by
  device ID. Laws (`trace`): `a_line_parses_back_to_its_event`,
  `the_lamport_clock_orders_receives_after_sends`. The slice keeps the clock
  in memory and sends it to no peer yet.
- git-annex is not instrumented: pigeon logs an event per git-annex command
  with its JSON result; the harness snapshots `whereis --json` between steps.
- The current daemon emits observed `Roster`, `CaptureInstall`, `Evict`,
  `Thin`, `Restore` and `Resolve` operations. Filesystem operations have
  before/after states; roster moves include accepted signed history and the
  daemon-loaded roster. The adapters check actual values against the model
  actions. They do not claim coverage of every action in the design models.
  `Authority`, `Purge`, `Repair` and `CustodyTokens` have no daemon adapter.
  Environment changes, including crashes, are constrained by observations.
- In production the feature is off; `PIGEON_TRACE=1`, with a build that has
  it, attaches a trace to a bug report **[no ADR: switch name]**.

See [Checking daemon traces](../daemon-traces.md) for commands, required CI
coverage, the observation contract and the limits of these projections.
