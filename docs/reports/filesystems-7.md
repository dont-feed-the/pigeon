# M6 filesystem admission checkpoint

This change implements the supported-volume policy of [14 §14.2](../spec/14-platforms-and-packaging.md).
It does not complete M6 or establish its reference-tree or HDD budgets.

## Policy and native observation

`pigeon-core::filesystem` admits exact Linux ext4, btrfs and xfs drivers for
laptops and bare backup repositories; ZFS is backup-only. macOS laptops
require APFS and the native local-volume flag. Unknown drivers, lookalike
names, explicit remote flags and other platform/role combinations refuse.
Three hegel laws each run 1000 generated cases.

The platform adapter opens the target or its closest existing ancestor
without creating anything. Linux binds the descriptor's native `statx`
mount ID to its exact `/proc/self/mountinfo` record, including bind mounts;
path prefixes never select a driver. macOS uses native `fstatfs`. Missing,
malformed or ambiguous identities never produce a guessed supported driver.
Opening a FIFO is nonblocking, so an accidental non-folder target cannot
hold the daemon waiting for a writer.

Create and join check before creating target directories or a repository
and recheck the canonical target. Doctor checks existing group volumes.
Public Problems distinguish an unsupported volume from an unreadable
identity, with English and French next steps. The lab feature alone permits
Linux overlayfs container fixtures and existing simulated Mac custodians
on local APFS. These are not extensions to release filesystem support.

## Validation and limits

Local native checks cover observation of a missing target without creation,
a FIFO without a writer, an actual proc descriptor distinct from the
fixture volume, malformed/ambiguous mount records, exclusive renames,
stable hard-linked identities and bytes, real create/join refusal and
doctor on a legacy tmpfs repository. A refused create or either join role
must preserve an existing binary file and leave no repository behind.

The first CLI fixture failed because this workspace's `/tmp` is tmpfs,
which the policy correctly refuses. Moving local fixture allocation to the
workspace's overlayfs made the accepted source fixture valid for the lab.
The tmpfs policy was not relaxed. Both native CLI checks then passed. The
final focused run passed all 14 checks, including existing-file preservation
and the FIFO without a writer. All-target Linux Clippy passed again after
those additions. Apple Silicon system-crate cross-compilation also passed
at the preceding checkpoint; cross-compilation is not macOS runtime evidence.

CI adds ext4, btrfs and xfs loop-mounted fixtures to one existing daemon
shard, reusing its binaries and Cargo cache. Each fixture asserts the
observed kernel driver and checks public preflight plus native install
primitives. These are small sparse filesystem fixtures, not large-file or
HDD performance measurements. Their first actual CI results must be recorded
before claiming these native driver checks passed. This workspace lacks the
privileged mount tools needed to run that script locally. ZFS runtime and
current-source macOS filesystem evidence remain separate open checks.

The initial 748-check parallel local run on `40b6ae6` passed 743 checks and
failed five bridge/shared-storage fixtures that forced `/tmp`, despite the
configured temporary volume. `/tmp` is tmpfs on this host, so create correctly
refused it. Those five fixtures now honor the configured temporary directory;
all five passed together in the focused retake, including the native paged
catalogue. This does not relax the release or lab tmpfs policy, and the
original failed run is not described as a successful 748-check run. Fixed and
random S9 fixture defaults likewise follow the selected temporary volume.

The named keep-index S9 regression passed against the filesystem branch's
current-version binaries in 14.2 seconds. TLC accepted its two filesystem
streams and roster stream. This scenario is not a resource-budget measurement.

## Usability

Independent English and French agent sessions, accepted by the user for
usability evidence, found the refusal and next action clear but lacked
explicit reassurance about existing files. The messages now state that
the check does not change files. The guide explains `df -T` and Disk
Utility's Format field so a person can identify a suitable folder.
The original observations and focused runtime retakes are retained in
[English](m6-filesystem-usability-english.md),
[French](m6-filesystem-usability-french.md),
[English retake](m6-filesystem-usability-english-retake.md) and
[French retake](m6-filesystem-usability-french-retake.md).
These sessions assess wording; they do not establish filesystem support
or existing-file preservation. Native assertions provide the latter check.
