# Own-read latency feedback

> Mission: connect bounded, comparable native capture reads to ADR 0037's
> disk back-off without changing capture verification or claiming HDD budgets.

Capture's final independent observation already streams through 64 KiB
cache-advised reads. It now times each successful full-size read on the
opened inode's device. Policy waits, hashing, EOF and short final tails do
not become latency samples. Comparing full reads avoids learning a tiny
file's cheap tail as the baseline for a larger operation. The original
post-read inode/stat and digest checks remain required.

Each device keeps the first sample in each ten-millisecond interval, at
most 1000 samples from the last ten seconds, and assesses their upper median
at most once per second. This is the median of bounded sampled reads, not
an unbounded ledger of every syscall. The first ten seconds train the
baseline only from an entirely quiet sample window, excluding independently
observed Linux I/O pressure; subsequent
quiet medians may lower it. A median strictly above three times that
baseline signals contention. Zero-resolution measurements become one
nanosecond; comparison uses wide arithmetic. Devices never train each
other. At most 32 active device histories are retained; entries idle for
ten minutes expire, and additional devices fall back to native pressure.

Both own-read feedback and the existing PSI signal feed the existing disk
controller. Stale read observations expire after ten seconds; the existing
five-second quiet period and thirty-second ramp still apply. Capture keeps
its existing 5% floor (50% under Fast). This conservatively backs off the
whole daemon when one observed device is busy. Scheduling other devices
independently, mapping physical devices beneath filesystems, and instrumenting
other own-read boundaries remain open. macOS can now obtain this capture
signal without Linux PSI; a hosted runtime check is separate from Linux
verification. No HDD throughput, foreground degradation, reference-tree
memory/CPU, elapsed M7 gate or complete global queue is established.

## Validation

Four pure laws run 1000 cases each: strict tripling and independent windows,
bounded retention under arbitrary/backwards clocks, exclusion of independent
pressure from baseline training, and overflow-safe extreme measurements.
A dense stream fills all 1000 slots without exceeding the bound. Native
controller fixtures check separate device baselines, capture floors, stale
samples, the unchanged quiet ramp and the device cap. An actual opened-file
fixture checks that policy waits report no reads, two full chunks report
the inode's device, and the final short tail and EOF report none; its exact
byte length, hash and post-read stat still pass. The first five core checks (four 1000-case laws plus the dense bound) and
eleven selected native/controller checks pass. A stronger quiet-window law
then exposes that one new quiet sample could train a baseline from older
independently busy readings. Its original failure is retained; every sample
in a baseline-training window must now be quiet. The strengthened retake
and broader validation are recorded separately below.

## Previous candidate retained

The immutable v0.6.19 candidate does not publish: its original native shards
0 and 3 fail initial-copy waits; one bounded CI retake passes shard 0 but
fails the same initial mass-deletion protection wait in shard 3. No deletion
has occurred. The second artifact 12120414778 is 5,466,831 bytes, SHA-256
`e25b78fab04a51d19cd9dc90f1106669a598c985d13b4464602d41e265f89b39`.
Both repositories accept all 102 names; the source has 102 verified SHA-256
objects and the backup has 74. Early listener refusals, transfer stalls,
metadata-fetch timeouts and closed-handle errors are retained. Unlike the
original pressure observation, this second fixture logs no high-PSI event.
These observations do not establish one exact cause. No third tag retry,
longer assertion deadline or missing-copy waiver is applied.

Unchanged source passes the isolated local mass-deletion case in 20.016
seconds and the concurrent shard's same case in 59.490 seconds. The entire
concurrent source-19 shard passes all 13 cases in 223.623 seconds; it
reproduces the CI selection and concurrent load, without relaxing timeouts.
That observation remains distinct from the two retained CI failures. This own-read checkpoint is not
claimed to fix the previous candidate's unproved transfer interleaving.

## Current-main validation

The strengthened five core checks pass, including all four 1000-case laws.
The 176 selected package-library checks pass, followed by all 275 core and
Git-driver checks (including the catalogue oracle and transfer contracts).
Seventy capture/install, held-deletion, folder-authority, pending-save and
resource-progress checks pass in 65.443 seconds, with one existing explicit
skip. The actual bounded-read fixture passes with two full-chunk observations
and no short-tail or EOF sample. These groups overlap; their counts are not
added into a claimed unique total.

The first native build finds an older dependency artifact lacking the new
core module in the shared local target. No failed runtime verdict is hidden:
cleaning the core/driver packages removes 752.2 MiB of stale artifacts and
the retake succeeds under the whole-phase validation lock. The earlier
quiet-window law failure remains separate.

Current main `8434c4f8549db0f88410a4c30dd7140c3274cc3b` (PR #88) is
integrated before PR validation. That integration adds CI/runtime evidence
and replay checks; it changes no Rust source from the preceding runtime
parent. Final formatting, all-target workspace Clippy and file/header,
ADR-index and spec-reference checks run again before committing.

## Release validation

PR #89, head `dab49b0a1fd73c257f2455586886ee1bc7bfb78b`, integrates current main
`8434c4f8549db0f88410a4c30dd7140c3274cc3b` before validation and passes CI 39088860699,
lab 39088860805 and bridge 39088860705.
The local proof, earlier quiet-window regression and cache-artifact rebuild
remain distinct above. M6 is still incomplete; this checkpoint does not
claim the reference/HDD budgets or the full global queue. The actual
Mac supervisor/CPU result is documented in the native preset runtime report.

This candidate also carries the invitation-paste and idle-supervisor fixes
from unpublished v0.6.19 and PR #88's verified CI/runtime improvements.
The failed v0.6.19 tag is immutable, with both original and bounded-retake
fixtures retained; no third retry or gate waiver is applied to it.
The latest independently verified publication preceding this candidate is
v0.6.18. The new tag must pass fresh Linux CI/lab/slice gates, both archives
and installation/publication verification. Main cache warming is not an
extra pre-tag gate, and active main Rust cache producers remain protected.
