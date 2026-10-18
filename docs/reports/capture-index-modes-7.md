# Index-only capture mode correction

> Mission: retain an ingested pointer while correcting executable bits without another content read.

## Native regression and change

After explicit annex ingestion, capture corrects executable bits because
git-annex 10.20261006 `Annex/Ingest.hs` (`ingestAdd'`) obtains the mode from
`contentLocation`; `Annex/Link.hs` (`stagePointerFile`) stages the pointer
with that mode. The previous correction uses `update-index --chmod`, which
can invoke annex's clean filter on the working file again.

Two real-annex regressions fail on the unchanged correction: one counts
two clean-filter children; the other writes a new save after explicit
ingestion and observes that correction replaces the staged pointer OID.
Their causal red run takes 0.405 seconds. An earlier compile-only pilot is
retained separately and supplies no causal evidence.

Correction now reads the current bounded chunk's exact stage-zero entries
with `ls-files --stage -z`, using literal pathspecs, then applies differing
executable modes with one `update-index -z --index-info` call. It retains
the original staged OIDs and never opens working contents. A complete,
unique matching response is required before any write: missing, duplicate,
unmerged, unexpected, nonregular, malformed or incomplete entries fail
closed. NUL framing retains literal tabs, newlines, leading dashes and Git
pathspec-like names. This adapter does not relax capture's name policy;
nonportable names still remain held by ordinary capture.

File modes still come from regular-file observations. Full independent
hash/stat/mode binding remains mandatory afterward, so new working bytes
cannot acquire an old pointer's positive proof. Quiet/hot deadlines, writer
checks, immutable annex copies, capture chunk limits and resource settings
remain in effect. An external writer to the same Git index is not granted
a new transaction protocol by this change.

## Controlled measurement

Three alternating observations use a private native-annex fixture of 128
synthetic files, each 500,000 bytes, with mixed executable modes. Each trial
resets staged modes outside its timed interval and verifies every staged OID
after correction. The previous method takes 0.246, 0.229 and 0.284 seconds;
index-only correction takes 0.00424, 0.00492 and 0.00534 seconds. Median
mode-only time changes from 0.246 to 0.00492 seconds. All old trials start
two clean filters; all new trials start zero.

These measure mode correction alone. The full 100,000-file diagnostic
[identifies ingestion](preparation-phases-7.md#full-size-bounded-observation)
as the immediate preparation bottleneck, but this local result neither
explains the whole duration nor establishes an M6 resource pass. A fresh
bounded diagnostic on integrated source is still required.

## Validation and restart provenance

All five new adapter checks pass, including native literal-name/mode/OID
checks, a later-save byte-proof refusal and whole-chunk refusal when one
staged pointer is missing. The trace requires exactly one bounded index
read and one index write, with zero annex clean filters. All 203 daemon
library cases and all 56 capture/install cases pass, including the
1,700-file byte/object/mode fixture in 16.619 seconds. All-target workspace
Clippy reports zero warnings.

A host restart then removes background processes and ephemeral `/tmp`
reports. The pre-restart counts and benchmark numbers above are recovered
from recorded tool output; the synthetic fixture and native trace files
survive. They are not presented as newly recovered original log files.
After integrating v0.6.40 source
`1dbcf729a7f33de98afbe215c309371963d676d8`, all five focused cases pass
again, along with three affected native capture cases. Rebuilt matching
lab programs complete a fresh two-peer, 32-file, 4096-byte Fast-to-Gentle
smoke and independently hash every unique object on both peers. Its budget
verdict is `not-measured`. Post-restart evidence is stored under the
persistent workspace. Repository checks precede submission; actual release
validation and the fresh full-size diagnostic remain separate.

## Integrated release validation

PR #110, head `893769af541c018b61473ec45b7f995e948d26e7`, integrates main `1dbcf729a7f33de98afbe215c309371963d676d8` before validation.
CI 39133597398, lab 39133597486 and bridge
39133597341 pass. v0.6.40 is independently published
and verified first. Local validation includes 203 daemon library and
56 capture/install cases, five focused and three native integrated checks,
and a rebuilt two-peer smoke with independent object hashes. All-target
Clippy and repository checks pass. Fresh v0.6.41 tag gates and independent
publication verification remain required. The changed-source full reference
diagnostic is separate and cannot certify an M6 budget. Actual elapsed
qualification remains open after the host interruption.

## Changed-source full-size diagnostic

Run [39133641982](https://github.com/dont-feed-the/pigeon/actions/runs/39133641982)
uses clean head `893769af541c018b61473ec45b7f995e948d26e7`, production
programs, all 100,000 files and 50 GB logical content, with diagnostic Fast
preparation and a 600-second bound. It expires during ingestion. The last
completed batch reports 20,480 paths at 579,242,976 microseconds. The final
source index has 20,482 entries including metadata, with 21,262 unverified
object files; an in-progress batch is not a completed or accepted capture.
Independent proof and backup planning have not started. Accepted paths and
backup object files remain zero. Twenty preparation snapshots are retained.

The earlier `b41ee1` diagnostic completed 24,576 paths around its deadline.
These separate hosted observations demonstrate no full-workload speedup;
the local mode-only timings above are not an end-to-end claim. The source's
new last cgroup CPU total is 300,228,739 microseconds and RSS high water is
156,524,544 bytes, versus 400,841,378 and 133,808,128 in the earlier last
sample. Different runs, policy waits and runner conditions prevent assigning
those changes to mode correction alone. Neither run reaches the resource
measurement window or qualifies M6.

The completed original job log and artifact 12139641787 are retained before
another experiment. The 3,555-byte archive has SHA-256
`811a84b9708fe17572b5eb204c93c25f887b367585966fa0a9157ec200809477`.
No unchanged-source retry is used. The next diagnostic adds fixed operation
labels to the existing end-state native process snapshot. It reads at most
4096 command-line bytes per already-sampled process, within the existing
64-process bound and PID identity fences, exporting only recognized labels
such as `git/update-index` or `annex/add`. Unknown or unreadable input remains
`other` and does not erase the resource row. No argument, path, key or token
is exported. This is one snapshot, not attribution of the whole workload;
it adds no resource polling and changes no dataset, bound or budget.
