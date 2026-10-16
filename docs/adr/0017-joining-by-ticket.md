# 0017. Joining by a one-time ticket; people manage their own devices

- **Status:** Superseded by 0075 (one invitation per group, shared as a
  line that installs and joins). Accepted on 13 October 2026 by Guillaume
  (tooling interview, Q11). Refines the authority rule of ADR 0009 for
  device entries.

## Context

A device takes part only once its ID is in the roster (ADRs 0006, 0007,
0009), through a signed commit. ADR 0009 lets only admins change `.pigeon/`,
so every new laptop would wait for an admin. Short codes need a PAKE
(SPAKE2, CPace), as magic-wormhole does; a long random secret does not.
git-annex's `p2p --pair` pairs repositories through magic-wormhole, but it
knows nothing of the roster.

## Decision

- `pigeon invite` creates a ticket: the group ID (hash of the genesis
  commit), the inviter's node ID and addresses, 256 random bits, its purpose
  (a new person, or a new device of an existing person) and an expiry
  (default 24 hours). The inviter stores only a hash of the secret, and the
  ticket works once. It is shown as text and as a QR code.
- `pigeon join <ticket>` generates the device key, connects to the inviter
  over iroh, and proves the secret inside that connection, so the secret is
  bound to the joining node ID. The inviter's device then signs the roster
  commit that adds the device, and the new device fetches.
- Authority over device entries: adding a new person needs an admin; adding
  or removing a device of person P is valid if signed by an admin or by one
  of P's devices, under the roster at the parents.

## Consequences

- A ticket is a bearer secret for 24 hours: whoever holds it can join once.
  It must travel over a channel the group trusts. Every join is a signed,
  visible roster commit, and an admin can revoke the device.
- Revoking a device stops new connections; it cannot unshare what that
  device already fetched.
- Short codes with a PAKE can be added later behind the same `join` step.
