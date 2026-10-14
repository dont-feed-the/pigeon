# Report: the safety slice, v0.1

The first build of pigeon v2 and what its slice showed, against the plan of
[13 §13.12](../spec/13-verification.md) (issue #17). This report is not
normative: the specification and the ADRs say what pigeon does.

## What was built

One person, one laptop and two custodians on a LAN, with K = 2. `pigeond`,
`pigeon` and the transport helper `git-annex-p2p-iroh` run over git-annex's
P2P protocol. The commands of 13 §13.12's first-build list work; every
other command answers `not-in-this-build`. The choices the slice made where
the specification left room are in row 13 of
[16 §16.3](../spec/16-open-questions.md); none needed an ADR.

The scenarios are written in pigeon-lab's EDSL (`crates/pigeon-lab`). Each
one runs in the model first, then against the lab build of the real
binaries, on sized disks: loop-mounted ext4 with mount namespaces on
Linux, APFS disk images on macOS. The merged trace logs are checked by TLC
against the TLA+ modules.

## Result

At 8f9342b, slice run 38636908664 passes S1–S11, with S9 at every named
crash point, on both `ubuntu-24.04` and `macos-latest`. The earlier
models-full runs 38610156407 and 38620593663 agree.

| Scenario | What it shows | ubuntu | macOS |
|---|---|---|---|
| S1 | *local* → *published* → *protected*, each step from a recorded observation | pass | pass |
| S2 | `free` keeps what is not protected, then frees it | pass | pass |
| S3 | `keep` and `restore` give back the same bytes | pass | pass |
| S4 | an offline custodian; *protected* once it returns | pass | pass |
| S5 | a killed 1 GiB transfer resumes; no partial object counts | pass | pass |
| S6 | a save racing `keep`'s install wins | pass | pass |
| S7 | a destroyed custodian, its removal and a repair in queue order | pass | pass |
| S8 | a full custodian refuses admission; a full laptop refuses a restore first | pass | pass |
| S9 | a crash at each named point loses and duplicates nothing | pass | pass |
| S9, random | 50 random crash schedules per push, 1000 in models-full | blocked | blocked |
| S10 | a fresh laptop recovers with the recovery sheet | pass | pass |
| S11 | steady and peak disk use per device | pass | pass |

**Still blocked: the random S9 schedules.** They did not fit the slice job
beside S1–S11, which take about 35 minutes, so `slice.yml` runs them in a
job of their own, two shards of 25 per runner: a schedule takes about 10 s
on Linux and 60 s on macOS. In run 38648627496 (at 13188f8) both Linux
shards and one macOS shard passed; in the other macOS shard one case failed
once and passed in hegel's 10 replays, which overwrote its logs, so the
cause is not known yet. Each case now keeps its logs and prints why it
fails. They leave `slice::BLOCKED`, and v0.1.0 is tagged, once both runners
pass.

## What the slice found

In git-annex 10.20261006, the server side of the P2P protocol has two bugs
(ADR 0069):

- `CHECKPRESENT` ignores a removal lock (E23), so two custodians that drop
  the same key at once can leave fewer than K copies.
- The relay can send `CONNECTDONE` before the last of git's output (E25),
  and the fetch then hangs. Over 500 fetches in CI, the stock build hung
  18 times on Linux and 9 on macOS; pigeon's build never hung (git-annex
  run 38630611813).

pigeon now runs a pinned git-annex with both fixes
(`third_party/git-annex/`). Its workarounds stay as defence in depth: the
serving filter probes with `LOCKCONTENT`, every P2P fetch has a deadline,
and a stuck `remotedaemon` is restarted.

In pigeon itself:

- Killing git with KILL alone orphaned its git-annex, which kept a transfer
  lock for 15 minutes (S5). pigeon-git now sends TERM first, then KILL
  after 10 s (ac236be).
- The daemon's socket did not answer while the journal replayed, which
  macOS's slower start showed (S9). It now answers, and busy locks keep the
  worktree untouched (911c423).
- A restore that did not fit below `annex.diskreserve` failed halfway; it
  is now refused before it starts (S8, 911c423).
- Dialling a stopped peer held up the others, and a restarted endpoint
  changed port (5d9e740, c163400).
- Re-thinning raced saves; it now waits 60 s without change and hashes
  before the move-aside, and a mismatch goes to `recovered/` (f68a822).

In the lab: a macOS runner once failed `hdiutil create` with no message, so
it is retried (9e32f95). A request that met a daemon as its crash plan
killed it is made again (8f9342b).

## Seen, not fixed

- A restarted iroh endpoint still receives packets of old connections,
  which then wait out the 30 s idle timeout.
- With the stock git-annex, lab E12–E13 is flaky on CI runners (a push
  that fails, a change notice that does not arrive), which fits the relay
  race. It passed 6 of 6 times locally. The lab still runs the stock build.
- Patch 0001 can answer "absent" for a key that the serving process itself
  holds a shared lock on. The error is on the safe side.

## Next

Milestone M1 of the [roadmap](../roadmap.md): signed commits,
`clock.json`, place in queue order with admission (issue #13), the problem
catalogue and status reports, and versioned formats with fixtures
(issue #15).
