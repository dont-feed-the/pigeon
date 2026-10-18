# Joining review: preserve files after a failed backup join

The review reproduces a backup cleanup defect using generated files. This
checkpoint does not close the remaining joining/signing review, laptop
rollback races, connection limits or any M6/elapsed gate.

## Reproduction and correction

A backup join starts in a new or empty directory. On ordinary failure the
old rollback recursively removes either that whole directory or every entry
it contains. The initial emptiness check does not establish ownership of
files another application creates while joining waits on the network.

Two regressions create new personal bytes after the check, at the root and
inside a repository-shaped subdirectory, then invoke the actual bare cleanup
path. Both the new-directory and existing-directory cases lose those bytes
before the correction: two failures in 0.008 seconds.

An unsuccessful backup join now retains the complete unfinished repository.
It does not guess which later files belong to Git, annex or another writer.
The existing joining marker and annex UUID let the same group resume this
repository. It still is not a configured/served group until its welcome
and history pass verification. Logs distinguish this retained backup from
a rolled-back laptop. No deletion or new persistent format is introduced.
The joining specification records the role distinction; laptop rollback
behavior stays outside this correction.

## Validation

Both file-preservation regressions pass in 0.008 seconds after the change.
After rebuilding the programs, the real backup test gets a refusal with
an incorrect invitation secret, retains the unfinished marker and UUID,
adds a personal file, then joins with the correct invitation. The UUID and
exact personal bytes survive and only the joining marker is cleared.
That native case passes in 1.489 seconds. The existing laptop cleanup/retry
case passes in 2.338 seconds; the two-case run takes 2.339 seconds.

Current main, including the 0.6.25 release commit, is integrated before the
final library/workspace and hosted PR validation. The original native
binary validation predates only that version/evidence update; the runtime
code is identical. All-target Clippy and repository checks pass locally.

All 175 daemon-library cases pass in 17.782 seconds on the integrated base.
Formatting, all-target workspace Clippy with zero warnings, header/length,
ADR index and spec references pass. A separate native check restarts the backup daemon before retrying. It
passes in 1.686 seconds with the original UUID and exact personal bytes;
the accompanying laptop retry passes in 2.490 seconds (2.491 seconds total).

## Integrated release validation

PR #95, head `b502aa41254d6773c38d2ece9e65edb788a96646`, integrates main `c300bd119ae90027c1c9677bb58070998ec3ec3a` before
validation. CI 39099831774, lab 39099831777 and bridge
39099831844 pass. v0.6.25 was published and independently
verified before this candidate. The complete joining/signing review and
M6/M7 resource and elapsed gates remain open. Fresh v0.6.26 tag gates and
independent publication/install checks are required before it is released.
