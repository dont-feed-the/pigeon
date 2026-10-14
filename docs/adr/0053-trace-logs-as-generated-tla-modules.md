# 0053. Trace logs reach TLC as generated TLA+ modules; TLC stays at 1.7.4

- **Status:** Proposed, 14 October 2026, by the agent, for issue #10
  (Guillaume's review: "an ADR re-pinning TLC with the Json module").

## Context

ADR 0008 has each `*Trace.tla` read the merged NDJSON log with
`ndJsonDeserialize`, which it calls part of our `tla2tools.jar`. It is not:
`Json` is one of the CommunityModules, a separate jar. ADR 0026 pins TLC
v1.7.4 by SHA-256 and foresees a new ADR pinning a build that reads NDJSON,
either a TLC 1.8 build or the CommunityModules jar beside 1.7.4.

Measured on 14 October 2026 (Java 21, the pinned 1.7.4):

- **CommunityModules beside 1.7.4 fails.** Every `CommunityModules-deps`
  release tried, from 202107061436 to the newest, 202610131629, stops a
  spec that extends `Json` with `NoClassDefFoundError:
  tlc2/value/impl/KSubsetValue`. The modules are built against TLC 1.8,
  and the 1.7 line, kept for its stable releases, lacks that class.
- **TLC 1.8 has no fixed build.** The GitHub `v1.8.0` release is a rolling
  prerelease whose jar is replaced every night (on 13 October: `TLC2 Version
  2026.10.13.164326`, rev 210df3a, SHA-256 0f56d6a8…). The Maven snapshots
  (`1.8.0-20261013.165330-105`) are purged after a time. A pin by SHA-256
  breaks with the next nightly, unless pigeon mirrors the jar itself.
  With CommunityModules 202610131629, that nightly reads NDJSON correctly.
- **A generated module is enough.** An NDJSON log of 20 000 events,
  written as one TLA+ module, is parsed and checked by 1.7.4 in 1.7 s
  (`ndJsonDeserialize` under the nightly: 0.7 s). A fault-injection run
  logs far fewer events, and each run is validated on its own.

## Decision

- The NDJSON format of 12 §12.3 is unchanged: pigeon logs as ADR 0008 says.
- `scripts/trace-to-tla` turns a merged log into a module that defines the
  log as a tuple of records, and the `*Trace.tla` module extends it. TLC stays
  pinned at v1.7.4 for every model and trace (ADR 0026).
- The encoding is total or refused, never approximate (as ADR 0048 asks of
  unknown values). Integers, booleans, tuples, records with identifier keys
  and printable ASCII strings are written; `null`, fractions, control and
  non-ASCII characters, reserved words as keys, empty and repeated-key
  objects are refused with their line. The trace vocabulary of each spec
  (12 §12.3) writes paths and keys as such identifiers (`p1`, `k7`), so a
  refusal is a bug in the logger. Lines out of the merge's Lamport order
  are refused as well.
- A trace configuration has the invariant `TraceNotConsumed`
  (`l <= Len(log)`), violated exactly when a behaviour of the spec explains
  every line. `scripts/run-tlc` names the verdict `trace:accepted`, or
  `trace:rejected:N`, where N, the depth of TLC's search, is the first line
  no behaviour explains (13 §13.3).
- CI checks the pipeline now, before the first real trace:
  `scripts/trace-to-tla --check` regenerates the fixtures and expects 15
  refusals; `TraceSample` (a toy save-then-capture spec) accepts a good log
  whose values carry quotes, backslashes and nested values, checked by
  `ASSUME`, and rejects a bad log at line 2.

## Consequences

- One TLC for models and traces, pinned by SHA-256, with no nightly in CI.
- The converter is pigeon's code to keep correct. Its encoding is a law:
  for every value it accepts, TLC reads back what the NDJSON held (13 §13.4,
  encodings). The fixtures and the `ASSUME`s check it.
- Very long logs cost parse time. If a run's log grows past about 10⁵
  events, the harness splits it at points where the state is logged whole.
- TLC 1.8 features stay out of reach (for example `POSTCONDITION` and the
  CommunityModules' `TraceSpec` helpers). The inverted invariant does the
  job of the postcondition.
- **Revisit** when TLC 1.8 has a stable release: pin it and the
  CommunityModules jar by SHA-256, and change only the definition of the
  log in each `*Trace.tla`.
- ADR 0008's sentence on `ndJsonDeserialize` is superseded on this point;
  16 §16.2 records the departure until this ADR is decided.

## Alternatives considered

- *TLC 1.8 nightly and CommunityModules, mirrored as assets of a pigeon
  release and pinned by SHA-256.* Works today and follows the paper's tools
  exactly. Rejected for now: CI would run a build nobody released, for every
  model, and pigeon would maintain a mirror of third-party jars.
- *The CommunityModules jar beside 1.7.4.* Fails, as measured above.
- *A Java override of our own for `ndJsonDeserialize` on 1.7.4.* Ties pigeon
  to TLC's internal value classes, the very thing that breaks above.
