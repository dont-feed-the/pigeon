# 0074. Mount the peer UI behind bridge authorization

- **Status:** Proposed, 16 October 2026, for the Nextcloud bridge interface.
- Extends ADR 0021's local interface with an authenticated hosted mount;
  the local listener and its session/origin rules remain applicable locally.

## Context

Each Nextcloud authorization has its own Pigeon peer and signing identity.
A second implementation of file history, suggestions, health and settings
would diverge from the normal interface. An external bridge mount cannot
reuse the daemon's loopback-only listener or forward a local web cookie.
The accepted ADR 0021 records that original local-only deployment.

## Decision

Reuse `pigeon-web` pages and handlers at a distinct URL prefix for each
managed peer. The hosting bridge authenticates the browser session, checks
origin and peer ownership, and supplies that session's form token. Before
every RPC, including live polls, the backend repeats session, ownership and
revocation checks and verifies the saved device and annex identities.
Expired or revoked live event streams stop. No daemon web cookie is passed.

Construct prefixed links, forms and event URLs at their source. Do not
rewrite rendered HTML, where a file name could look like a URL. Managed
clients leave local pinning to Nextcloud and setup/removal to bridge
management. The account's personal control peer uses the same review pages.
Local native pages retain their current authentication and controls.

## Consequences

Native and hosted interfaces share behavior and translations while each
write remains authorized by its selected daemon's identity. The mounting
API trusts its host to enforce authentication and origin before dispatch;
its documentation and boundary tests must make that contract explicit.
The native RPC list limit still applies to these pages. DAV and the bridge
management catalogue continue to use pagination.
