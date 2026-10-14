# 12. Observability

> Mission: the three channels through which pigeon explains what it did:
> logs for people (ADR 0022), telemetry for a distributed group (ADR 0025),
> and trace events checked against TLA+ (ADR 0008). Logs and telemetry may
> change freely; trace events are a checked interface, and the `trace` laws
> cited in §12.3 are normative.

## 12.1 Logs (ADR 0022)

- Spans cover each loop ([08 §8.1](08-custody-loops.md)), repository, peer
  and git or git-annex command, with fields such as `repo`, `device`, `key`,
  `path`, `commit`.
- **Levels:** `error`, a person must act; `warn`, degraded but handled (a
  peer unreachable, a refused `REMOVE`, a retry); `info`, a state change a
  user could see; `debug`, protocol detail; `trace`, wire level.
- Binaries log by `PIGEON_LOG` (default `info`): human lines on stderr (the
  systemd or launchd journal), and JSON lines in `<state>/logs/`, one file
  per day, seven kept ([11 §11.2](11-local-state.md)).
- **Redaction:** keys, tokens and invitation secrets print as `[redacted]`,
  which a test checks for every such type. File names are logged; `pigeon
  debug bundle` asks before including them. Logs are never parsed as traces.

## 12.2 Telemetry (ADR 0025)

- One pipeline: spans, events and OpenTelemetry metrics exported as OTLP by
  `pigeon-telemetry`, the only crate depending on `opentelemetry*`, pinned
  to one minor version.
- **Correlation:** every span carries natural IDs (device, repository,
  commit, key, proposal, decision) and the Lamport clock (§12.3). W3C trace
  context crosses every boundary pigeon owns: CLI requests (`traceparent`),
  child processes (`TRACEPARENT`), the join exchange and each stream header
  ([09 §9.8](09-network-and-joining.md)).
- **Where it goes:** by default only a local OTLP-JSON file
  (`<state>/telemetry/`, at most 50 MB, oldest dropped first **[no ADR:
  bound]**). `pigeon debug bundle [--from NAME…]` collects logs, telemetry
  and trace events from this device and chosen peers (`pigeon/debug/1`),
  after asking each peer's person **[no ADR: consent on the peer]**.
- **Telemetry device:** if the roster names `telemetry_device`
  ([04 §4.2](04-roster-and-authority.md)), the others ship OTLP to it over
  `pigeon/telemetry/1`. Nothing leaves the group unless a member sets
  `[telemetry] endpoint` on their own device.

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

**Dashboards.** People see group health in the web interface from
replicated state, without telemetry (P6; [06](06-status.md),
[10 §10.3](10-interfaces.md)). Developers use any OTLP back end; the
reference is `grafana/docker-otel-lgtm` with dashboards in
`ops/dashboards/`.

## 12.3 Trace events (ADR 0008)

Trace events let a run of the real system be checked against the TLA+
specifications ([13 §13.3](13-verification.md)).

- Each specification defines its events (action names, variables, value
  encodings). `pigeon-core::trace` has one enum per specification and one
  variant per action, behind the cargo feature `tla-trace`; each name is an
  operator of its `.tla` file (`roster_trace`:
  `action_names_are_operators_of_their_specs`).
- NDJSON, one line per action, in `<state>/traces/`:

```json
{"spec":"CaptureInstall","device":"3f9a…","clock":1042,"action":"InstallStep","vars":{"path":"p1","step":"RenameIn","key":"k7"}}
```

- **Lamport clock** (`clock.json`, [11 §11.3](11-local-state.md)): incremented
  at every event, carried by every message to a peer (stream header, join),
  and set to max(local, received) + 1 on receipt. Logs merge in clock order,
  ties by device ID. Laws (`trace`): `a_line_parses_back_to_its_event`,
  `the_lamport_clock_orders_receives_after_sends`.
- git-annex is not instrumented: pigeon emits an event per git-annex command
  and its JSON result, and the harness snapshots `whereis --json` between
  steps.
- Specifications with events (ADR 0008 asks at least the first five):

| Specification | Actions traced |
|---|---|
| `CaptureInstall` | the model's actions (13 §13.13): the person's `WriteAtP`, `WriteHandle`, `Replace`, `Delete`, `Open`, `Close`, `Tick` (inferred from the scan); pigeon's `Capture`, `Refresh`, `Start`, `TakeGroup`, `MoveAside`, `Scan`, `Check`, `RenameIn`, `Index`, `Done`, `Release`, `Retry`, `Crash`, `Recover`, `RecoverEnd`; the group's `NewVersion`, `Decide`, `ResolveMine` |
| `Roster` | `Commit`, `Merge` ([13 §13.3](13-verification.md): runs of pigeon-core's fold) |
| `Authority` | `Capture`, `Revise`, `Adopt`, `Decide`, `Transfer`, `Remove`, `Merge`, `Fetch`, `Served`; the attacker's actions (`HCommit`, `HMerge`, `HAdd`, `HDecide`) log nothing: their commits appear in `Fetch` and `Served` |
| `Evict` | `ChooseFree`, `ChooseKeep` (the `wanted` target), `Hide`, `HideFinish`, `Release`, `DropFree`, `Prune`, `Get`, `Stage`, `MoveIn`, `Unhide`, `Restage`, `Unstage`, `Capture`, `Crash`, `Restart`; `Replace` and `WriteHandle` inferred from the scan, `GroupChange` from accept, `Observe` from status reports |
| `Purge` | `Request`, `Cancel`, `Restore`, `Seal`, `Release`, `DropReleased`, `Refetch` |
| `Repair` | `Join`, `RemoveLost`, `StartGet`, `FinishGet`, `AbortGet`, `Sync`, `Drop`, `LaptopDrop`, `Scrub`, `FsckDone`, `Crash` (logged at restart), `Remove` (a roster change); the faults `LoseDisk` and `Rot`, and `Partition` and `Heal`, are not logged and stay free |
| `CustodyTokens` (existing) | the hand-off events of the existing model |

- In production the feature is off; `PIGEON_TRACE=1`, with a build that has
  it, attaches a trace to a bug report **[no ADR: switch name]**.
