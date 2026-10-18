# M6 reference resource measurement

The resource budgets remain unmeasured until the full native profile
completes. The reduced smoke fixture is never a passing budget result.
Physical E17 HDD, elapsed soak and milestone gates remain separate.

## Reproducible native profile

Dispatch `lab` with `native_reference_only=true`. The profile builds the
ordinary release programs without lab features. `scripts/check-reference-budget`
owns one new sparse 256 GiB Btrfs image mounted with forced Zstd compression.
There must be at least 6 GB of free physical host space. The image's recorded
filesystem UUID must still match before unmount/removal; busy or unidentified
mounts retain their private image. This is compressed logical storage on a
hosted runner, not a physical HDD or an incompressible-content benchmark.

The actual tree contains 100,000 distinct regular files of 500,000 bytes
each: 50,000,000,000 logical bytes. Each has an eight-byte unique header and
zero padding. These ordinary bytes are written, hashed, captured and sent
through the production local-network P2P path to a real second daemon.
They are not sparse working files, duplicate-key aliases or prebuilt annex
metadata. Generation checks compression after each thousand files to abort
before an uncompressed run fills the host.

Preparation waits at most 90 minutes for all paths in both accepted main
trees and every backup object. It independently checks each pointer's size
and expected digest, and hashes all 100,000 unique objects on both peers.
It does not infer completion from an empty Protected headline. Native
failure or incomplete preparation produces no passing budget verdict.

Default settings and the default round interval remain. The laptop runs
in a transient systemd service with the installed Linux service's Nice=10,
CPUWeight=20, idle I/O, IOWeight=20 and MemoryHigh=512M. The backup uses
normal priorities. Both units use cgroup v2 CPU accounting. The 600-second
window includes all descendant CPU, including exited and reaped Git/annex
children, without CLI or Git polling by the profiler. Every sample fences
the daemon's PID start time. RSS and its lifetime high-water mark are read
for each daemon; each must remain at most 300,000,000 bytes. Each computer's
idle CPU must average below 1% of one core over the full window.

Successful capture candidate scans log only counts and monotonic duration
at debug level. At least one scan of all reference index entries, with no
pending changes, must start and complete inside the measurement window in
at most 600 seconds. The CPU window includes the actual default recurring
scans. Missing/rotated evidence fails closed. Completed scans are diagnostics,
not custody evidence or authorization.

The JSON artifact records source, kernel, filesystem/options, preparation,
raw monotonic samples, CPU, RSS high-water marks, scans and separate verdicts
for both peers. No invitation or private identity material is printed.
Both services stop before the owned volume is unmounted. The profile is a
manual measurement rather than an additional ordinary patch release gate.

## Initial local validation

The real 32-file/131,072-byte smoke profile completes with both accepted
trees and every object independently verified. It observes eight completed
scan markers before its five-second observation. Its verdict is
`not-measured` and its CPU field is null: the smoke run uses ordinary
processes without cgroup accounting or service presets. Neither its RSS
nor its duration establishes the full reference budget.

Eleven measurement and ownership contracts pass, including exact pointer
size/digest checks, corrupt/symlink object refusal, native-child cgroup
accounting, CPU/window/memory/scan failure conditions, process-name parsing,
and busy/changed-mount image preservation. Command doubles establish only
control flow. Native hosted Btrfs/systemd behavior still needs the full run.

## First native preparation observation

Source `4342e9b56f459a9831d3b8b899462487fe0a6566`, manual lab run
39097165434, job 117585905429 builds the production programs and mounts
native Btrfs, then stops during the first compression check. No peer or
measurement window starts. Artifact 12124948513 is 507 bytes, SHA-256
`e4317cf828d1d3658601164e7095fb2a2b8f48f2d525c1d61346cc3dea9ed3ca`;
its verdict is `not-measured` with zero samples. It remains retained.

The original guard uses `st_blocks`, which can describe uncompressed
extents on Btrfs and cannot establish physical compression. The corrected
guard reads `compsize -b`'s physical Disk Usage column and records all three
byte counts, together with the actual generated file count. Its byte-only
parser rejects unreadable output. `compsize.c`'s `human_bytes` and
`print_table` confirm that `-b` emits raw decimal bytes. The original
observation does not itself prove compressed allocation; the next native
run must record that evidence. The whole profile is revalidated on current
main before another dispatch; no failed immutable tag is retried or moved.
