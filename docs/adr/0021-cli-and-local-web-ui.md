# 0021. Interfaces: a CLI and a local web interface served by `pigeond`

- **Status:** Superseded by 0076 (the web interface ships a bundled
  TypeScript client, built with Node). Accepted on 13 October 2026 by
  Guillaume (tooling interview, Q14).

## Context

The feasibility study requires test users to explain *local*, *published*
and *protected*, and to decide proposals. A command line alone will not
reach them. A tray or native GUI adds a toolkit per platform; a web page
served locally works on Linux, macOS and later Windows with one code base.

## Decision

- Two interfaces with the same operations: the `pigeon` CLI (ADR 0024) and a
  web interface served by `pigeond`. Both send the same request types over
  the same internal API, so no operation exists in only one.
- The web interface is rendered on the server (`axum`, `maud`), without a
  JavaScript framework; small scripts only refresh status.
- It listens on 127.0.0.1 only. `pigeon open` opens a URL holding a random
  token, exchanged for a session cookie; requests without the cookie, or with
  a `Host` header other than the loopback address, are refused (CSRF and DNS
  rebinding).

## Consequences

- The web interface needs its own usability tests, part of the study.
- Every new operation is designed once as a request, then exposed twice.
