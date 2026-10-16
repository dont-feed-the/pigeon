# Report: folder restoration and local availability

M4 progress, version 0.4.9, 16 October 2026. A normal-build independent
recovery session found that an accepted deleted nested folder could not be
restored as a folder: CLI lookup expected a single file pointer, and the
browser offered only individual files. The participant distinguished this
failure from Bring them back, which correctly cancels a held deletion.
The same session proved that Keep said files were kept while they were
unreadable for 43.5 seconds with the only source offline. Exact bytes
returned after the source restarted. Free also claimed completion before
its background work ran. These findings are failures in the 0.4.8
baseline, not claimed passes.

## Changes

Restore can rebuild missing nested folders from one accepted snapshot,
using its latest nonempty saved state or an inclusive `--at` cutoff.
Unmentioned newer files stay in place. A file's default restoration is its
latest saved version, including its state before deletion. File version
numbers remain available; folders explain that a date selects their state.
The browser offers whole-folder actions for wholly deleted subtrees and
uses the latest saved version for individual deleted files.

Downloads and total staging space are checked before folder writes.
Existing install journals protect each file, including open writers and
late saves. Incomplete restoration returns exactly one final Problem,
including restored and unchanged counts, so the CLI exits unsuccessfully
and cannot stop before reading the failure. Symlinks and nested
repositories cannot redirect a deleted-path restore.

Saved executable modes are retained. The first integration pass exposed
that capture had already lost the original mode: git-annex 10.20261006
`Annex/Ingest.hs` `ingestAdd'` reads the pointer's mode from
`contentLocation`; `Annex/Link.hs` `stagePointerFile` uses that supplied
mode. In an isolated patched-annex reproduction, a 0755 working script
staged as 100644. Capture now explicitly stages the working executable
bit. Mode-only saves remain distinct in file history, even with one annex
key, and restoring uses the saved mode without prematurely staging the
restore as already captured.

Free/Keep confirm requested background work. Files reports requested local
copies not yet downloaded or installed separately from their backup safety.

## Verification

- Four core snapshot-selection laws run 1000 cases each, covering one
  coherent moment, inclusive cutoff, exact scope and preservation of
  unmentioned newer paths. Existing framing laws include local-arrival
  counts.
- A real integration test exercises accepted bulk deletion, three-level
  restoration, binary/UTF-8/CRLF bytes, executable modes, an earlier
  snapshot, retained newer bytes, an open writer, partial failure and an
  outside symlink sentinel. The test exposed and drove the mode and
  multiple-final-response corrections above.
- Web tests check outermost wholly deleted subtrees without offering a
  partly current ancestor; both language catalogues cover the messages.

The complete workspace suite passes with the lab and TLA trace features,
including real multi-device collection, both backup recovery roles, all
eight refound SIGKILL boundaries, exact-byte exports and folder restoration.
The repository rules and warning-free full-workspace Clippy checks pass.

An independent normal-build CLI/Chromium participant repeated the failed
0.4.8 tasks in a new laptop/backup group. Both accepted-deletion folder
restores recovered all five exact files and executable mode 0755; the CLI
completed in 0.581 seconds. An earlier snapshot retained the newer-only
file, and a separate read-only history view retained the exact newer bytes.
A real open editor and an active writer both caused explicit incomplete
3/1 results, preserving the editor's final bytes. Close/retry succeeded.

Free correctly explained background work. With the only source stopped,
Keep and Files continued to explain two pending local copies after 45.0
seconds; both remained unreadable. After restarting the backup, both exact
hashes were verified at the 7.2-second sample and the waiting text cleared.
The participant verified accepted deletion rather than cancellation of a
held deletion before counting either restoration pass. All fixture daemons
and Chromium were stopped and verified absent. This is Linux evidence;
other platforms' open-file behavior remains covered by their CI, not this
session. Folder ownership setup and the optional operator dashboard remain
separate M4 work.
