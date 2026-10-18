# Device-specific capture verification

> Mission: use the actual opened device for final capture-read permission
> and active time, preserving exact inode, hash and cancellation checks.

The prior adapter trains each device separately but combines every busy
reading into one native policy. Its independent capture verification also
used that combined permission, so a different device's read delay could
pause verification of an otherwise quiet device.

A real bounded-read regression fails against that behavior in 1.013 seconds:
the generic policy is closed while the opened inode's policy permits reads.
The supervisor ignores the actual device and the outer fixture deadline
cancels it. The corrected read path uses its expected native device only
after streaming_read_exact validates the opened inode against that complete
stat. Its post-read stat, digest and expected-file checks remain unchanged.
Read timing still excludes policy waits, hashing, short tails and EOF.

Each bounded device window now owns a Disk controller. System pressure has
a separate Disk controller; a device's read share is their minimum. Unknown
devices inherit system pressure without inheriting another device's read
median. Each source retains the original capture floor, five-second quiet
period and thirty-second recovery. Existing native children and content
streams still conservatively use the combined controller. This checkpoint
improves capture verification, not all native or network device scheduling.
The existing 32-device/sample and inactive-retention bounds remain.

Eleven focused checks pass in 1.608 seconds. They cover independent device
shares, global pressure overriding both devices, Gentle/Fast floors, stale
recovery, exact real bytes, opened-device selection and a pause longer than
the normal read budget. The pause uses an open generic policy and a closed
actual-device policy; it resumes exact bytes without spending the pause.
Default driver controls retain their old Capture policy; scoped download
views forward both device permission and active charging.

A further 1000-case adapter law varies device identities and quiet latencies,
including wide integer ranges: one slow median cannot reduce another or an
unknown device's capture-read share. The underlying latency and Disk algebras
retain their existing laws. Final broader verdicts follow when complete.
Neither synthetic latency samples nor these finite native reads establish
foreground HDD, reference-tree idle/scan/memory or elapsed soak budgets.

## Broader local validation

All 163 daemon-library cases pass in 15.655 seconds before adding the
separate 1000-case adapter law. All nine selected real-peer cases pass in
111.333 seconds, covering erasure/refounding, held changes, fresh join after
a decline, capacity, returning backups, free and keep. The CLI, daemon and
transport helper are rebuilt before native validation. The focused group
overlaps the library run, so those counts are not summed.

The separate adapter law passes all 1000 cases. All 50 Git-driver tests
pass in 6.463 seconds, including existing pause/deadline/cancellation
contracts; all-target Clippy and repository checks pass. The release-gate
and script suite separately validate the S9/KPI rollup without changing
this runtime's byte-verification or policy boundaries.

## Original PR #92 Files-page observation

Head `e3b49d3467bce1a8908c12139cf263791831293f`, CI 39095141973,
job 117579932940 fails the native S2 case in 20.550 seconds at its Files
page assertion just after freeing a folder. The page reports P-INTERNAL-ERROR,
not missing file bytes. The shard otherwise passes 13/14 in 109.401 seconds.
Artifact 12124600907 is 12,298,655 bytes, SHA-256
`a590f5b91f1b71e0585bef429989ea2ce3e17966b92ac7caf82dd2c26d5866b1`.
The retained source and both backups each have two independently hash-valid
objects. The large working file is freed; the small file remains exact.
The generic page omits its underlying error, and the original log does not
establish that exact cause. The original directories and job log are retained.

Inspection finds one concrete read conflict: the pending-save scan rejects
an index snapshot if Git replaces the index during its before/read/after
fence. The adapter formerly returns that transient observation immediately.
The unchanged single-read policy fails two new contract cases: recovery
after changed observations and the intended maximum observation count.
This is a separate mechanism reproduction, not proof of the hosted error's
exact cause. The corrected adapter takes at most three complete fenced
observations, retrying only that specifically typed change. Persistent
instability, unreadable data and parsing errors still fail closed. No lock
is removed, no path mutation is retried, and all file/index validity guards
remain. Files-page errors now log their underlying reason locally.

No assertion deadline or generic error is waived. The old head's lab,
bridge and complete five-shard S9/TLC profile pass. That profile establishes
all fifty schedules under the new layout; the runtime correction gets a
fresh exact-head validation before merge.

The bounded snapshot contract retake passes all three index checks,
including the existing ctime fence. Both unmodified real-peer S2 cases
pass in 28.096 seconds; the original Files-page case passes in 20.354
seconds. All-target Clippy and structural checks pass. The original
underlying page error remains unproved; future failures retain a precise
local error log rather than only the generic rendered page.

After the snapshot correction, all 166 daemon-library cases pass in
15.587 seconds. Formatting, all-target workspace Clippy with zero warnings,
header/length, ADR index and spec references pass before commit.
