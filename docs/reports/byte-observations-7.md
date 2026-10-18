# M7: working-byte observations across a native rename

> Mission: distinguish a bounded journalled move-aside from missing or wrong bytes.

## Original macOS observation

Background [run 39128408817](https://github.com/dont-feed-the/pigeon/actions/runs/39128408817),
source `24dcd39b382b7e3a961b8554c5002c6a15d04a4d`, completes its workspace,
installation, daemon shards, lab groups, S5 and both random S9 shards.
Its remaining slice job [117689043078](https://github.com/dont-feed-the/pigeon/actions/runs/39128408817/job/117689043078)
fails S14 step 17: the hold of Ana's group version and Bob's suggestion
observes `sha256 None`. The whole background run is failed.

The retained trace records Ana's second laptop moving the unchanged
protected plan aside for thinning and then renaming the same bytes back.
The old byte observer retries for one second without consulting that
path's install journal. These facts identify a candidate observation race;
they do not establish the original missing-path duration. The original job
is neither retried unchanged nor reclassified as passed.

The original slice archive is 546388 bytes, SHA-256
`c57bae6334be7788a2603cb615e17a118c864aa52c6024413e9a54d20ddb606d`.
Its reports and all daemon/trace logs remain preserved before changes.

## Controlled reproduction and observer

A native-file regression writes protected bytes, records an open journal
transaction for that path, moves the file aside and restores it after
1.4 seconds. The unchanged one-second observer reports missing bytes and
fails the assertion in 1.408 seconds. The retained RED log SHA-256 is
`1a984537501569241ba4f9e4b62d671d9483dbc5761e70f419ac6728ae446e44`.

The byte condition still hashes the actual regular working file. A valid
open transaction for the exact path may extend its one-second observation
to at most five seconds. Readable wrong bytes return immediately for the
normal hash comparison to fail. Persistent absence fails at the bound.
Malformed, torn, completed and other-path journals cannot extend it.
A journal never supplies substitute bytes or authorizes recovery. Failure
messages now name the device and fixture path as well as the observed hash.

All ten focused native observer checks pass, including the causal
regression, a hard-bound clock fixture and immediate wrong-byte observation.
The observation helper has its own module so the executor stays within the
repository's file-length rule. This is functional byte checking; it does
not measure file availability latency or qualify any resource budget.
Scenario assertions and model verdicts remain required.

## Targeted platform validation

The manual `macos-background` input `people_diagnostics_only=true` runs
only the native people scenarios and their trace checks after the pinned
annex proof. It does not repeat unrelated workspace, lab or random-crash
jobs. The default periodic run and all release-tag Linux coverage remain
full. Both paths use the existing scenario partitioner and live catalogue.

The changed-source macOS result is recorded below. The finite local
observer checks do not establish that platform's end-to-end S14 result,
the complete M7 audit, a 72-hour soak or final-RC qualification.

## Local native scenario

The finalized observer also runs S14 against frozen v0.6.41 lab programs
(source `c08b5274cc99a5e89fee4c7bf2c952ce69f159a5`). Five real Linux
peers in directory isolation pass the functional scenario in 85 seconds:
23 checks, zero failures, with the action-trace check skipped because these
frozen programs have the lab feature without trace emission. Running the
trace collector on that output also fails for lack of traces; no local TLC
pass is claimed. The full tag slice still requires emitted traces and TLC.
This is neither APFS evidence nor a resource measurement.
The following macOS checkpoint provides separate platform validation.

## Changed-source macOS checkpoint

Targeted [run 39141412086](https://github.com/dont-feed-the/pigeon/actions/runs/39141412086),
source `fecb04ca66d1cefc3d81facfac39486f5815fcaf`, passes on native APFS
images. S14 and S15 each pass 23 checks in 334 seconds; S16 passes 23 in
260 seconds. There are no native failures or skips. All required filesystem
and roster trace projections are accepted by TLC. Dependency and installed
annex proofs pass first; unrelated workspace, lab and random S9 jobs are
correctly excluded from this explicitly selected diagnostic profile.

Artifact 12142573346 is 252780 bytes, SHA-256
`132764772fe800030fa0acf544f40e6ddd5564ae15c5f62129f28f6ce6528981`.
Its original reports/traces and complete finished-job log are retained.
This is one changed-source first-attempt result; the older full background
run remains failed with its original S14 observation. The lab's simulated
Mac custodians do not supply production macOS backup support. This profile
is neither a whole-platform sweep nor resource, elapsed or final-RC evidence.
