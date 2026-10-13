# 0007. One ed25519 key per device: iroh identity and git signing key

- **Status:** Accepted, 13 October 2026, by Guillaume (tooling interview, Q3).

## Context

The transport helper (ADR 0006) admits only peers listed in the roster, and
the authority rule (P4) needs every catalogue commit to be signed. iroh
identifies a node by an ed25519 public key. git signs commits with SSH keys
(`gpg.format=ssh`, which runs `ssh-keygen -Y sign -n git`). Each protocol
signs messages with its own fixed prefix (TLS 1.3 `CertificateVerify`,
SSHSIG with namespace `git`), so no message valid in one is valid in the
other.

## Options

1. One ed25519 key per device, used as the iroh node key and as the SSH
   signing key.
2. Two keys per device, one for iroh and one for signing.
3. A key per person that certifies the keys of their devices.
4. OpenPGP.

## Decision

Option 1. Each device generates one ed25519 key at pairing. Its public key is
the device ID everywhere: iroh node ID, commit signer, roster entry. The
roster maps each person to the set of their device IDs; roles (admin, owner,
steward) belong to persons. pigeon writes the key once as an OpenSSH private
key (mode 0600) so that git can sign with it.

## Consequences

- One identifier per device, one revocation per lost device; no person key
  to lose.
- pigeon verifies signatures itself against the roster in force at the
  commit's parents (Rust crate `ssh-key`, SSHSIG). It does not use git's
  `allowedSignersFile` validity dates, which rely on forgeable commit times.
- A stolen unlocked laptop can sign until an admin revokes its device ID;
  this is outside the guarantees (keys are not stolen, §3 of the proposal).
- Signing needs `ssh-keygen`, present on Linux, macOS and Windows 10+.
