# 0006. Transport: our own `git-annex-p2p-iroh` helper in Rust

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q2).

## Context

git-annex supports P2P networks through a helper program
`git-annex-p2p-<netname>`: called with `address` it prints the local address;
called with an address it relays stdin/stdout to that peer; called with an
address and a socket path it serves incoming peers on git-annex's unix
socket. The P2P protocol carries both git (`CONNECT git-upload-pack` and
`git-receive-pack`) and annexed content, so one link serves everything.
Joey Hess's helper is a shell script over `dumbpipe`, Unix only. git-annex
allows any paired peer to `REMOVE` content, so the transport is also where
access can be restricted.

## Options

1. Our helper in Rust on the `iroh` crate (QUIC, hole punching, relays,
   node identity = ed25519 public key).
2. Joey's script with `dumbpipe`, unchanged.
3. Tor, built into git-annex.
4. SSH over a VPN (Tailscale, WireGuard).

## Decision

Option 1. The helper accepts only node IDs listed in the signed roster, and
it can run on Windows later. Option 2 stays for the prototype.

## Consequences

- One more binary to ship; it must follow the generic P2P transport
  interface exactly.
- Whether the helper also filters protocol messages (for example refusing
  `REMOVE` on custodians) is a separate decision.
