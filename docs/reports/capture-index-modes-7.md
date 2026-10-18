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
