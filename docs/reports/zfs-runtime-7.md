# Native ZFS backup validation

> Mission: exercise actual Linux ZFS backup admission and persisted bytes,
> separately from pure driver recognition and the reference/HDD budgets.

The normal Linux filesystem profile covers ext4, btrfs and xfs. ZFS is
backup-only, so applying the laptop-oriented profile to it would test the
wrong role. This explicit native profile creates one private file-backed
ZFS pool, verifies the actual kernel driver, refuses laptop creation before
writes, joins a backup on that volume and checks two named protected files.
Their physical objects must be regular files on ZFS, contain the exact
expected bytes and have matching SHA-256 keys. Restarting the daemon must
preserve its annex UUID, accepted main and exact objects without rejoining.
The existing native filesystem primitive suite also runs on the volume.

The eight-GiB sparse vdev is a filesystem-conformance fixture with sufficient
space for the unchanged five-GB reserve. It is not reference-tree/HDD data,
a throughput measurement or a pool-export/crash test. Default checks do not
silently pretend ZFS ran: the hardware test is explicitly ignored unless
scripts/check-zfs runs it with the required actual ZFS mount. Local discovery
and compilation are not a native runtime verdict.

Cleanup refuses pre-existing pool names, records and rechecks the pool GUID,
and retains an unidentified pool or image instead of guessing ownership.
On a native failure it copies synthetic Git metadata, including hidden
files, before destroying its known fixture pool. Special socket nodes are
omitted; daemon logs and failed native directories are also captured for
14 days. These are disposable test identities, not a person's installation.
Four shim-driven cleanup checks cover successful ownership, an existing
pool, changed/missing identity and failure metadata retention. They are
shell-control-flow tests, not native ZFS evidence.

The lab workflow has an explicit native_zfs_only dispatch. It runs one Linux
fixture job and skips the ordinary lab matrix and its selection aggregate.
Ordinary push/PR/reusable lab behavior is unchanged. Publication accepts only
push events for the exact tag/source and requires all four ordinary Linux
lab job families; a manual native profile cannot substitute for that gate.
The profile restores main's Rust cache without writing it and requires the
checksum-pinned patched git-annex. Kernel/module availability is checked
before a pool is created; unavailable support fails rather than passing.

## Local preparation

All 84 script checks pass, including the four cleanup checks. Native test
discovery finds the explicitly ignored ZFS case. The first Clippy pass after
adding the independent SHA check rejects direct LowerHex formatting of this
sha2 version's byte array; formatting individual bytes corrects it without
a lint exception. The later all-target workspace Clippy and repository rules
pass. Current v0.6.21 main is integrated before the first hosted dispatch;
its intervening changes are version/lockfile and release evidence, with Rust
sources equal to the preceding runtime parent. The hosted verdict remains
pending and no ZFS runtime success is claimed here.

GitHub rejects the first dispatch before allocating a runner: runner.temp
is not available in job-level env. Temporary-directory setup moves into
the execution step using RUNNER_TEMP; no native execution or filesystem
verdict is attributed to that rejected definition.
