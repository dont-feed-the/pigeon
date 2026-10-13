# 0032. Backup machines run Linux only in v1

- **Status:** Accepted, 13 October 2026, by Mario (chat: "Linux seulement");
  Guillaume may object. Narrows ADR 0005 for the custodian role.

## Context

ADR 0005 targets Linux and macOS in v1, for every machine. Backup machines
(custodians, ADR 0002) are headless and always on: a NAS, a mini PC, a home
server. They carry the safety guarantee (P1), so their configuration is the
one most worth testing: locked, read-only objects, append-only serving
(ADR 0014), scheduled checks, purge after the waiting period (ADR 0018).
Each extra platform for this role multiplies those tests.

## Decision

- In v1, a backup machine runs Linux (x86-64 and arm64), with the standalone
  git-annex build (ADR 0020) and `pigeond` as a systemd service.
- People's computers run Linux or macOS, as in ADR 0005.
- `pigeon join --backup-machine` refuses to run on another platform, with a
  `Problem` that says why.

## Consequences

- The custodian test suite (fault injection, E9, E10, E13, E14) runs on
  Linux only.
- A Mac used as a backup machine waits for a later version; a member with
  only Macs can still use a Linux NAS or mini PC.
- Packaging for backup machines: a static tarball and a systemd unit; NAS
  vendor packages (Synology, QNAP) are later work.
