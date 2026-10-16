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

`pigeon-telemetry` uses the OpenTelemetry 0.33 SDK and generated OTLP wire
encoders, with a tracing-opentelemetry 0.34 bridge. Each round is one trace,
with group, device and Lamport IDs inherited by its step spans. W3C context
crosses CLI socket envelopes, git child environments, joins and annex stream
headers; async scopes attach it only while their future is being polled.
Files rotate and retain at most 256 KiB, and debug bundles flush the SDK.

An admin selects `telemetry_device` through `pigeon settings telemetry-device
NAME` (`off` clears it). The receiver admits only that group's roster
members when it is the chosen collector; OTLP bodies are bounded and records
of another group are refused. Collection runs outside custody loops, with a
1 MiB device queue and 256 KiB batches. A local `[telemetry] endpoint` opts
that collector into OTLP HTTP forwarding. Environment variables, remote
rosters and redirects cannot choose an external backend.

The user dashboard is Home. `Health` reads current-file counts and retained
older-key counts from the same accepted ref snapshot as its headline;
pending saves replace their path's older current state and count as local.
An unreadable inventory is unknown and requires attention. A signed
`checking` report excludes that custodian from protection for both
inventories. Paper keys never count as storage.

Home also shows pending suggestions, endangered retained keys, each
machine's last contact and timestamped signed capacity/queue report, and
effective accepted device removals that began repair. Contact does not
refresh old capacity figures. Five-second events replace text values;
membership or repair-history changes reload the view. This uses no OTLP
backend. Its evidence is in [health-5](../reports/health-5.md).

The optional operator dashboard and setup instructions live in
`telemetry/`. It includes version-state counts, pending bytes, deferred
keys, annex call latency/outcome, transport bytes, acceptance/protection
lag and constructed work batches for capture, history, protection and
downloads. Scrub/rebalance measurements follow those implementations;
CPU/memory budgets remain M6. Batches clear on every exit and include the
current work until its stage advances; short batches may fall between SDK
samples. A missing inventory produces no count sample.

Acceptance records at most 256 new accepted capture commits per update,
with a truncation counter. Protection records a sampled
unprotected→protected transition for a current path/key; already-safe
startup inventory is excluded. A bounded immutable snapshot preserves its
observation time while a busy round finishes the protected history lookup.
Read-only counts continue during long offline-peer retries. Each group
has at most 4096 timing identities and 4096 unprotected identities per
process; excess diagnostics can be omitted. Wall-clock skew is explicit,
never a zero lag. These diagnostic clocks never decide safety or retention.

Admitted annex streams sample the selected iroh direct/relay/unknown path
in both directions. A reused connection counts again; later migrations
are not attributed. Transport bytes retain `unknown` path and include
protocol bytes. See [operator evidence](../reports/operations-5.md).

- Spans, events and metrics are exported as OTLP by `pigeon-telemetry`, the
  only crate depending on `opentelemetry*`, pinned to one minor version.
- Every span carries natural IDs (device, repository, commit, key,
  proposal, decision) and the Lamport clock (§12.3). W3C trace context
  crosses CLI requests (`traceparent`), child processes (`TRACEPARENT`), the
  join and each stream header ([09 §9.8](09-network-and-joining.md)).
- By default only to `<state>/telemetry/` as OTLP-JSON, at most 256 KiB,
  oldest dropped first **[no ADR: bound]**. `pigeon debug bundle [--from NAME…]` collects logs, telemetry
  and trace events from this device and chosen peers (`pigeon/debug/1`),
  after asking each peer's person **[no ADR: consent on the peer]**.
  `pigeon debug allow NAME`, or the explicit consent form in Settings,
  grants one part for that group and authenticated peer for ten monotonic
  minutes. Restart cancels it. Recovery keys cannot request parts. Peers
  receive only that group's records; ungrouped records stay local. Requests
  have a 45-second deadline and at most 2 MiB of JSON per peer.
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

The optional `pigeond/tokio-console` feature enables local task inspection
only in a debug build compiled with `--cfg tokio_unstable` and run with
`PIGEON_TOKIO_CONSOLE=1`. It fixes the listener to 127.0.0.1:6669; release
builds and ordinary starts install no console layer. Runtime spans are
filtered per layer, independently of the daily log level, and are excluded
from OTLP. See [telemetry setup](../../telemetry/README.md) and
[debug-console evidence](../reports/console-5.md).

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
- Traced: every action of `Roster` and `CaptureInstall`, the coded enums.
  Not yet coded: `Authority`, `Evict`, `Purge`, `Repair` and `CustodyTokens`,
  every action likewise, except that the attacker's actions (`HCommit`,
  `HMerge`, `HAdd`, `HDecide`) and the faults (`LoseDisk`, `Rot`,
  `Partition`, `Heal`) log nothing and stay free in the check; `Crash` is logged at restart, and
  `Tick`, `Replace` and `WriteHandle` are inferred from the scan,
  `GroupChange` from accept and `Observe` from status reports.
- In production the feature is off; `PIGEON_TRACE=1`, with a build that has
  it, attaches a trace to a bug report **[no ADR: switch name]**.

The small local spool keeps diagnostics within the steady disk budget even
for a small shared folder. Older diagnostic records are expendable; file
history and safety evidence are unaffected. A chosen collector can retain
longer diagnostic history through its explicitly configured backend.
