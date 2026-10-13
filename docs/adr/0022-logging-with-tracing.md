# 0022. Logging: `tracing`, human logs to stderr, JSON logs to a file

- **Status:** Accepted, 13 October 2026. Chosen by the agent after a survey
  that Guillaume asked for ("look up the best practices today"). Its line
  "No OpenTelemetry in v1" is superseded by ADR 0025.

## Context

Survey of crates.io on 13 October 2026 (downloads over the last 90 days):
`log` 320 M, `tracing` 219 M, `tracing-subscriber` 160 M, `env_logger`
85 M, `opentelemetry` 69 M, `slog` 9 M. tokio, iroh and axum emit `tracing`
events, and `tracing` spans keep context across `.await`, which a plain
`log` line cannot. Under systemd and launchd, a daemon's stderr already goes
to the system log.

## Decision

- All instrumentation uses `tracing`. Spans cover each protocol loop,
  repository, peer and git-annex command, with fields such as `repo`,
  `device` and `key` (`#[instrument]`).
- Levels: `error` needs a person to act; `warn` is degraded but handled;
  `info` is a state change a user could see (captured, accepted,
  protected); `debug` is protocol detail; `trace` is wire level.
- Only binaries install a subscriber: `tracing-subscriber` with an
  `EnvFilter` read from `PIGEON_LOG` (default `info`), human-readable on
  stderr, plus JSON lines in the state directory through `tracing-appender`
  (non-blocking, daily files, seven kept) for bug reports.
- Secrets are never logged: types holding keys, tokens or invite secrets
  have a `Debug` that prints `[redacted]`.
- No OpenTelemetry in v1.

## Consequences

- One logging system for our code and our dependencies.
- Logs are for people and may change freely. The TLA+ trace events of ADR
  0008 are a separate, typed channel checked against the specs; logs are
  never parsed as traces.
