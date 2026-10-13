# 0014. Custodians serve peers append-only: no remote `REMOVE`

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview,
  Q8), for custodians. Extending it to every device was proposed by the
  agent and rejected by Guillaume (Q9): laptops keep git-annex's default.

## Context

git-annex lets any paired peer send `REMOVE key` over the P2P protocol
(`special remotes/p2p`: "can also drop the content of any annexed file").
git-annex checks numcopies on the client side, so a buggy or modified client,
or a mistaken command, could delete custody copies. That contradicts P1 and
P4: only a signed purge, after retention, or a recorded Drop may release a
protected version. The git-annex binary (10.20261006) contains the server
modes `ServeAppendOnly` and `ServeReadOnly`, used by `git-annex-shell`
(`GIT_ANNEX_SHELL_APPENDONLY`) and by `p2phttp` permissions. Whether the
P2P remotedaemon can use them is not known yet.

## Decision

- Custodians serve peers in append-only mode: read, check presence, lock,
  and store are allowed; `REMOVE` is refused.
- A custodian drops content only on its own decision, run by its own
  `pigeond`: a purge whose retention has ended, or a rebalancing drop
  verified against K other custody copies.
- Mechanism: experiment E13 checks whether git-annex can serve a P2P
  network in `ServeAppendOnly` mode. If it can, pigeon uses it. If not,
  `pigeond`, which relays every incoming stream (ADR 0011), filters the
  protocol: an allow-list of messages, `REMOVE` answered with `FAILURE`,
  unknown messages closing the connection, and the protocol version capped
  to the highest one the filter understands.
- Laptops serve peers with git-annex's default permissions. Their copies
  never count toward K (ADR 0002), so a remote `REMOVE` on a laptop can cost
  a working copy, never a protected version; git-annex's client-side
  numcopies check still applies.

## Consequences

- P1 no longer depends on every client behaving: a remote client cannot
  delete custody copies.
- git-annex operations that drop from a remote (`move --to`,
  `--rebalance` run from a client, `sync --content` dropping from remotes)
  fail on custodians. pigeon never issues them; a refused `REMOVE` is logged
  as a bug.
- If the filter is needed, it parses the documented P2P framing (`DATA`
  lengths) and gets its own property tests (ADR 0012).
