# 0016. Relays and discovery: n0 by default, mDNS on the LAN, own relay optional

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q10).

## Context

iroh dials a device by its node ID (ADR 0007). To find a device behind NAT
it needs an address lookup service and relays; n0 runs public ones, and
iroh connects directly when hole punching succeeds. Traffic is end-to-end
encrypted (QUIC, TLS 1.3), but the relay and lookup operators see metadata:
which node IDs are online, from which IP, and who talks to whom. On a LAN,
`iroh-mdns-address-lookup` finds peers without any server. `iroh-relay` can
be self-hosted.

## Decision

- By default `pigeond` uses n0's public relays and address lookup, plus mDNS
  on the local network.
- The roster may name a group relay (URL); when it does, every device uses it
  instead of n0's relays. Self-hosting is optional and documented.

## Consequences

- Zero operations for a new group; a group that minds the metadata can host
  a relay without changing the design.
- n0's service is a dependency for groups that do not host a relay: if it is
  down, devices on different networks cannot find each other, while LAN
  peers and direct connections keep working. `pigeon status` reports this
  (P6).
- The relay URL is a roster field, so changing it is an admin decision,
  recorded and signed like any other.
