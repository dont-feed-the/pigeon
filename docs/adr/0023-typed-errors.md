# 0023. Errors: typed enums everywhere; one serialisable problem for people

- **Status:** Accepted, 13 October 2026. Chosen by the agent after a survey
  that Guillaume asked for.

## Context

Survey of crates.io on 13 October 2026 (downloads over the last 90 days):
`thiserror` 422 M, `anyhow` 241 M, `eyre` 23 M, `snafu` 22 M, `miette` 21 M,
`error-stack` 1 M. Common practice: typed error enums (`thiserror`) where
callers must tell errors apart, `anyhow` where errors are only reported.
pigeon's errors cross a process boundary (daemon to CLI and web, ADR 0011),
must say why and how to fix (P6), and must map to exit codes.

## Decision

- Each crate defines `thiserror` enums at its module boundaries and keeps the
  cause as `#[source]`, adding the operation and the object ("capturing
  `photos/a.jpg`: `git annex add` failed: …"). `anyhow` is allowed only in
  tests and developer tools.
- A failed `git` or `git annex` call keeps the command, the exit status and
  the end of its stderr.
- An error that reaches a person becomes a `Problem` with a stable code
  (`custodian-unreachable`), a message, a hint and details. The CLI prints
  it to stderr (or as JSON with `--json`), the web interface shows it; both
  get the same `Problem` from `pigeond`.
- No `unwrap` or `expect` outside tests, except for an invariant stated next
  to it; clippy's `unwrap_used` and `expect_used` lints enforce this.
- Release builds use `panic = "abort"`. A panic is a bug: it is logged and
  `pigeond` is restarted by its supervisor. Recovery from a crash at any
  instant is already required (P2), so restarting is safe.

## Consequences

- More code for error types, in exchange for errors that callers can match
  and identical messages on every interface.
- Problem codes are part of the public interface and are documented.
