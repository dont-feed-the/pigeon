# M7: complete objects before joining or recovery writes

> Mission: refuse missing file objects instead of materializing invented empty bytes.

## Reproduced boundary

The shared `recovery::hide_all` function prepares laptop files after an
ordinary welcome and after sheet recovery. It previously reads a file blob
using an optional result and substitutes an empty byte vector on failure.
A missing committed object is therefore confused with a real empty file.

A native fixture writes a real Git tree containing binary, empty and annex
pointer blobs. It removes only its own loose binary object after opening
the group, confirms that Git cannot read it, and places newly saved working
bytes at the target. The original function reports success instead of
refusal; the missing-object regression fails. Its companion positive case
passes. The original two-case log (one failure, one pass, 0.061 seconds)
is retained before correction. This demonstrates the materialization
boundary, not an exploit through the full verified remote-history path.

## Correction

Each file is read using its tree object's exact identity, through the
complete ordered Git reply parser. The reply must contain a blob. Missing,
failed and wrong-kind reads return `join-files-unreadable` before that file
is written. Real empty blobs remain valid, and annex pointers remain hidden
with their sparse/index state. The Error has English and French messages
and a documented stable Problem code. Signature and authority verification
still precede this setup; no schema, signing rule or invitation policy changes.

Both initial native cases pass after correction in 0.116 seconds. The
missing-object case now covers both ordinary and pointer objects and checks
that the existing working bytes survive. The positive case independently
checks binary bytes, a genuine empty file, an absent hidden name and its
sparse/index flags. A further native wrong-kind fixture uses an actual Git
tree/index entry naming a commit where a blob was declared; materialization
must check the object's actual kind rather than trusting the entry mode.

The first localization run catches an omitted row in `docs/problems.md`:
18 checks pass and one fails. The row is added and the original failure
retained. This is a documentation omission, separate from the native missing-
object regression and from any hosted CI result.

This change stops fabrication at an unreadable file. It does not make the
whole setup transactional: earlier valid entries can already have been
written. The broader ownership/path-substitution review, recovery-input
bounds, default-folder validation and full joining/signing security gate
remain open. No complete independent cryptographic audit is claimed.

## Signature-law coverage

The three existing signature properties used only 200, 300 and 100 cases,
below HANDOFF's 1000-case requirement. Each now runs 1000: signed round trip,
byte mutation and signature transfer to another commit. A fourth 1000-case
law signs an independently verified payload in another application namespace
and requires Git verification to refuse it specifically for that namespace.
The existing fixed duplicate-signature/armour check also passes. All five
signature checks pass together in 0.391 seconds. The signing implementation,
keys, algorithm and `git` namespace remain unchanged.

## Native validation preconditions

All 19 localization checks and the 217 initial daemon library checks pass.
The additional wrong-kind fixture passes with both initial object cases
(three checks, 0.127 seconds). Six native invitation/retry cases pass, but
the full recovery-sheet case fails its first Protected wait in 122.830
seconds, before reaching recovery: the backup reports `room=0`, one deferred
11-byte key and `headline-machine-full`. Workspace free space after the
failure is 4860833792 bytes, below the custodian's five-billion-byte reserve.
The complete original log and private fixture are retained. No product
assertion or reserve is relaxed.

Disposable owned build caches and already-verified public archive downloads
are reclaimed with journals before deletion. Obsolete completed public
verification installations are retired only with per-file SHA receipts and
live-process checks. Original fixtures, explicit frozen probes, current
installations and all verification receipts stay intact. A first retake setup is
interrupted because main was integrated while it was starting. It receives
zero validation credit. The stable checkout integrates v0.6.46 source
`95c30ff69ec7a815453e1358b52c75df76b45ac4` before rebuilding. Native recovery
will be measured separately with actual free-space headroom after building.

The stable v0.6.46 checkout passes all 218 daemon library checks, all five
signature checks and all 19 localization checks. After building, the native
run starts with more than six billion free bytes; all seven invitation,
retry and recovery-sheet flows pass in 108.935 seconds. The full recovery
case passes in 106.765 seconds, including recovery of an admin from another
person's backup. SHA-256 fences prove all three executed programs and all
changed runtime/test files stay identical throughout that run. The original
reserve failure and interrupted setup remain retained without a pass claim.
Final all-target Clippy and repository rules precede committing and CI.

All-target workspace Clippy completes with zero warnings. Formatting,
file/header limits, ADR index and spec references pass before the source
commit. Main is integrated and v0.6.46 independently published and verified
before this PR proceeds; hosted exact-head validation remains separate.
