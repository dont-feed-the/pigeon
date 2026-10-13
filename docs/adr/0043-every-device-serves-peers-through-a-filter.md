# 0043. Every device serves peers through pigeond's filter: pull only, append-only on custodians, drops with explicit copy counts

- **Status:** Proposed, 14 October 2026, by the agent, for issue #9 of
  Guillaume's review ("Prove git-annex integration cannot bypass catalogue
  validation or custody deletion policy"). The evidence is experiments E12
  and E13 (`ga-e12-e13-p2p-boundary.py`), checked against the git-annex
  10.20261006 source. Chooses the mechanism that ADR 0014 left to E13; ADR
  0014's decision stands.

## Context

ADR 0014 wants custodians to refuse remote `REMOVE`, natively if git-annex can
serve a P2P network append-only, else through a filter in `pigeond`. ADR 0011
leaves to E12 whether `remotedaemon` only fetches. Spec 03 §3.6 says devices
never push into each other, but nothing enforced it. The experiments and the
source show:

- `remotedaemon` serves every P2P network read-write: `P2PGeneric.hs` calls
  `serveAuthed ServeReadWrite`, and `GIT_ANNEX_SHELL_APPENDONLY` changes
  nothing. A peer removed C's copy with `drop --from c --force` (E13).
- `git-annex-shell p2pstdio` honours append-only, but it relays no git
  (`runRelayService`: "relaying to git not supported on this connection"), so
  it cannot serve a remote alone.
- Over P2P, a peer can push any ref into a custodian, its `main` included, and
  any ref but the checked-out branch into a laptop (E12). An unvalidated
  `main` on a custodian would feed its purge and status computations.
- `remotedaemon` fetches a changed remote by that remote's refspecs. It never
  moves `main`, `HEAD`, the index or the working tree (E12), and it exits when
  its stdin closes.
- The copy count of a drop comes first from `--numcopies` and `--mincopies`,
  then from `annex.numcopies` in `.gitattributes`, taking the highest over the
  key's files, even for `drop --key`, then from the git-annex branch, which
  any device can change (`Annex/NumCopies.hs`; E12/E13 phase D).
- In a laptop repository git-annex installs the hooks `pre-commit`,
  `post-checkout`, `post-merge` and `post-receive`; in a bare one, none.

## Decision

1. **A filter on every device.** `pigeond` already relays every incoming
   stream (ADR 0011). It passes each P2P session to the device's
   `remotedaemon` socket through a filter, a pure state machine over the
   framed protocol of `P2P/Protocol.hs`:
   - client messages on an allow-list for protocol versions 0 to 4: `AUTH`,
     `VERSION`, `BYPASS`, `CHECKPRESENT`, `LOCKCONTENT`, `UNLOCKCONTENT`,
     `GET`, `PUT`, `DATA`, `VALID`, `INVALID`, `DATA-PRESENT`, `SUCCESS`,
     `FAILURE`, `GETTIMESTAMP`, `NOTIFYCHANGE`, `CONNECT git-upload-pack`,
     `CONNECTDONE`, `ERROR`;
   - `VERSION n` becomes `VERSION min(n, 4)`, the highest version the filter
     knows (git-annex's `maxProtocolVersion` is 4 today);
   - on custodians, `REMOVE` and `REMOVE-BEFORE` are answered `FAILURE` and never
     reach git-annex. Laptops keep git-annex's default (ADR 0014);
   - on every device, a `CONNECT` to anything but `git-upload-pack` is answered
     with `ERROR` and the session closes: pull only;
   - an unknown message, a malformed `DATA` length or a line over 4096 bytes
     closes the session. A length is checked before anything is forwarded.
     Payloads pass by length, never parsed. Server output is parsed only for
     framing, so that the filter's own replies fall between frames.
2. **Pull only, twice.** Every repository also gets pigeon's `pre-receive`
   hook, which refuses every push, for any transport. The settings check
   (spec 03 §3.5) verifies that the hook is in place and that `core.hooksPath`
   is unset; otherwise it suspends serving and drops.
3. **Explicit copy counts.** Every verified drop pigeon runs passes
   `--numcopies K --mincopies K` from the roster. These outweigh
   `.gitattributes`, the git-annex branch and git config. The forced drop is
   reachable only from the purge and Drop code paths: the git driver exposes
   it to those two modules only.
4. **remotedaemon** runs per repository with its stdin held open by
   `pigeond`. A change notice only makes it fetch, by pigeon's refspecs;
   `pigeond` then validates, as after any fetch.
5. **Hooks** git-annex installs stay. pigeon runs nothing that fires
   `pre-commit` (it commits with `commit-tree`) or `post-merge` (it never runs
   `git merge`). `post-receive` cannot fire, since no push is accepted.
   `post-checkout` runs `git annex smudge --update`, which pigeon runs itself
   after sparse changes (E15).
6. The inventory of what may change a repository is spec 03 §3.8.

## Consequences

- A remote client cannot delete custody copies, and no device accepts a
  pushed ref. Every ref that matters is written by the device's own `pigeond`
  after validation. Lowered copy counts in `.gitattributes` or in the branch
  no longer weaken any drop.
- The filter must follow git-annex's protocol. A new protocol version is
  supported only once the filter's contract tests pass on it (ADR 0020).
  Until then a newer client is capped at version 4. The filter gets hegel
  properties (ADR 0012): any byte stream either passes as whole frames or
  closes, `REMOVE` never reaches a custodian, and framing survives every
  split of the input. E13 runs it against real git-annex with random read
  sizes.
- A git-annex client that tries to push over P2P waits after the `ERROR`
  (observed in E12). pigeon never pushes, so only a buggy or modified peer
  waits.
- Rejected alternatives:
  - `git-annex-shell p2pstdio` behind a shim: it gives no git, so it would
    need a second transport for history;
  - append-only on laptops, already rejected by Guillaume (ADR 0014);
  - relying on the hook alone: it does not stop `REMOVE`.
- If accepted, ADR 0014's status gains "mechanism chosen by ADR 0043 (E13)".
