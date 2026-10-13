# 0011. One `pigeond` daemon per user; CLI and transport helper are thin clients

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q6).

## Context

git-annex starts the transport helper once per outgoing connection, and its
remotedaemon starts it once more to serve incoming peers (ADR 0006). If each
start opened its own iroh endpoint with the device key (ADR 0007), every
connection would pay for hole punching again, and several processes would
register the same node ID with the relay. The protocol loops (capture,
accept, place and repair, evict, purge, install) also need long-lived state:
watchers, batch git-annex processes, retry queues.

## Decision

- One daemon, `pigeond`, per user, on the tokio runtime (required by iroh).
  It owns the only iroh endpoint, the watchers, the protocol loops, and the
  long-lived `git annex … --batch --json` and `git cat-file --batch`
  processes of every repository it manages.
- `pigeon` (the CLI) and `git-annex-p2p-iroh` (the helper) are thin clients.
  They talk to `pigeond` over a local socket readable by the user only
  (mode 0600, under `$XDG_RUNTIME_DIR` on Linux and the per-user temporary
  directory on macOS), with newline-delimited JSON messages.
- The helper, called to connect, asks `pigeond` for a stream to the peer and
  then relays bytes. Called to listen, it registers git-annex's socket, and
  `pigeond` forwards incoming streams from roster peers to it.
- `pigeond` runs `git annex remotedaemon --foreground` per repository as a
  child, and stops it on exit.
- Supervision: a systemd user unit on Linux, a launchd agent on macOS.

## Consequences

- One node ID online per device; connections reuse the endpoint.
- The helper does nothing without `pigeond`; it exits non-zero, as the
  generic P2P interface allows.
- Whether remotedaemon only fetches, and never merges, must be checked (E12);
  pigeon alone merges (ADR 0009).
- On Windows (v1.1) the local socket becomes a named pipe behind the same
  interface.
