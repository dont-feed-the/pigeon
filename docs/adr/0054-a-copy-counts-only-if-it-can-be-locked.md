# 0054. A copy counts only if it can be locked: custodians answer `CHECKPRESENT` with a lock

- **Status:** Proposed, 14 October 2026, by the agent, for issue #10
  (Guillaume's review: "Model the actual Pigeon boundary around annex
  locking/drop, including failures and durable state"). Refines ADR 0001
  (E1's result holds over local paths only) and ADR 0043 (the filter).

## Context

ADR 0001 rests custody on git-annex's verified drop, and E1 found it safe: 40
concurrent drops of the last two copies left one each time. E1 ran over local
paths. pigeon's devices reach each other over git-annex's P2P protocol
(ADR 0043), and there the drop is not safe. In git-annex 10.20261006:

- a local drop first takes the content's removal lock (`performLocal` in
  `Command/Drop.hs`, `lockContentForRemoval` in `Annex/Content.hs`), an
  exclusive lock that fails while a shared one is held;
- `verifyEnoughCopiesToDrop` (`Annex/NumCopies.hs`) asks each remote to
  `LOCKCONTENT`, a shared lock (a `LockedCopy`). If that fails, it falls back
  to `CHECKPRESENT` (a `RecentlyVerifiedCopy`);
- with a removal lock, `safeDropAnalysis` (`Types/NumCopies.hs`) needs
  max(numcopies, mincopies) copies "of any kind", since "that lock will
  prevent other git repos that are concurrently dropping from using the local
  copy as a VerifiedCopy". `--mincopies` therefore does not help;
- over a local path, presence is checked with `inAnnexSafe`
  (`checklocal` in `Remote/Git.hs`, `Annex/Content/Presence.hs`), which
  reports a copy locked for removal as absent. Over P2P the server answers
  `CHECKPRESENT` with `inAnnex` (`CheckContentPresent` in `P2P/Annex.hs`),
  which ignores the lock.

So two custodians that drop the same key at once each fail to lock the
other's copy, each see it present, and both drop. E23 reproduces it with real
git-annex over P2P (a lab network, `git annex p2p --link`): with numcopies 1 no
copy is left; with numcopies = mincopies = 2 on three custodians, one is. Over a
local path the same drop refuses. Custodians drop their own copies when they
rebalance ([08 §8.10](../spec/08-custody-loops.md)), so the race is
reachable whenever shares move. The `AnnexDrop` model finds it in 8 steps
([13 §13.2](../spec/13-verification.md)).

## Decision

**On custodians, pigeon's filter answers `CHECKPRESENT` with a lock**
([09 §9.4](../spec/09-network-and-joining.md)). For `CHECKPRESENT k` from a
peer, the filter sends `LOCKCONTENT k` to git-annex instead. On `SUCCESS` it
sends `UNLOCKCONTENT` and answers `SUCCESS`; otherwise it answers `FAILURE`.

- A copy is said present only if a shared lock on it could be taken, so a copy
  that its custodian holds for its own drop is never counted. That is the
  meaning `inAnnexSafe` gives a local path, and with it E1's result holds over
  P2P.
- The rules of ADR 0046 apply to the answer: `SUCCESS` waits until the object
  is synced to disk, and while the repository is `checking` the answer is
  `FAILURE`.
- The filter's laws gain one (ADR 0012, [13 §13.4](../spec/13-verification.md)):
  a positive `CHECKPRESENT` only for a copy it could lock. It forwards every
  other allowed message unchanged.
- Laptops keep git-annex's answer: their copies never count (ADR 0002).
- pigeon reports the bug upstream, with E23 as its reproduction. The probe
  stays until the minimum supported git-annex
  ([14 §14.3](../spec/14-platforms-and-packaging.md)) answers `CHECKPRESENT`
  with `inAnnexSafe` over P2P, which E23 then shows.

## Consequences

- The `AnnexDrop` model checks the drop as pigeon runs it: `AnnexDrop_design`
  passes, and `AnnexDrop_p2p`, which is git-annex over P2P without the probe,
  violates INV_K. E23 shows the same races keep every copy behind the probe,
  and a drop with no concurrent drop still succeeds.
- A probe that arrives while the custodian starts its own drop of that key
  holds a shared lock, so that drop fails to take its removal lock and is
  retried later. Two custodians dropping at once may both refuse; one of them
  succeeds on retry. Liveness is delayed, never safety.
- Like every `LOCKCONTENT` over P2P, the probe's lock writes a retention
  timestamp 10 minutes ahead (`lockContentShared` in `Annex/Content.hs`,
  called from `P2P/Annex.hs` with `p2pDefaultLockContentRetentionDuration`).
  It is removed with the lock, unless another shared lock is held at that
  moment or the server dies; then the custodian's own drops of that key wait
  up to 10 minutes.
- `git annex copy --to` and `get` still learn whether a peer has a key: a
  present copy can be locked unless it is being dropped, and then it is better
  copied again.
- A drop through the probe costs one more round trip per copy it cannot lock
  directly, which is rare: `LOCKCONTENT` is tried first.
- E1's row in [13 §13.7](../spec/13-verification.md) states that it ran over
  local paths. On acceptance ADR 0001's status gains "drop safety over P2P
  refined by ADR 0054".

## Alternatives considered

- **Rank the custodians and count only copies ranked above one's own.** The
  model passes with it even without the probe (`AnnexDrop_ranked`), but the
  top-ranked custodian could never drop: rebalancing would stop at it.
- **Answer every `CHECKPRESENT` with `FAILURE`.** Drops would stay safe, since
  they lock first, but every peer would believe a custodian lacks what it
  holds: `copy --to` would offer the key again and `fsck --from` would report
  it missing.
- **Wait for a fix in git-annex.** pigeon cannot ship a drop it knows is
  unsafe; the probe is small and goes once a fixed version is the minimum.
- **Raise `--mincopies`.** A local drop with a removal lock counts copies of
  any kind, so mincopies changes nothing (E23 runs the race with
  numcopies = mincopies = 2).
- **Serialise drops through one coordinator.** It needs one custodian that
  every other reaches whenever it drops, and a protocol of pigeon's own to
  verify; the probe keeps git-annex's.
