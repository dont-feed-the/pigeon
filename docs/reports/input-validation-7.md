# Input validation checkpoint

## Restore time selection

The shared RFC 3339 reader normalized impossible dates into another month
and accepted hours above 23, minutes above 59 or additional signs in UTC
offsets. A native restore of `2099-02-30T00:00:00Z` reproduced a successful
restore response instead of the required `restore-at` refusal.

The reader now round-trips the parsed Gregorian date and bounds decimal
offset components before returning an instant. Four hegel laws each run
1000 cases: UTC round trips across four-digit years, valid offset equivalence,
impossible-date refusal and malformed-offset refusal. Targeted century
boundaries retain valid leap days. Three checks failed before the fix;
all five passed afterwards. Existing second, fraction and separator policies
are unchanged.

The native restore regression uses two actual saved versions. Invalid future
dates and offsets must return `restore-at` and preserve the current bytes;
a valid time between saves must still restore the first version. Generated
date inputs are codec checks, not elapsed retention or soak evidence.

## Backup role refusal

Unsupported backup-machine roles now return the existing
`backup-machine-needs-linux` Problem before probing the selected filesystem.
The join adapter preserves that public code and message; its focused test
failed before the mapping was added. Linux backup and laptop admission are
unchanged, and macOS backup simulation remains explicitly lab-only.

English and French message reviews found the Linux next step clear and no
misleading APFS-folder advice. Both found that the guide's "Any computer
that stays on will do" contradicted the role restriction. The guide now
requires Linux with systemd and explains regular Mac joining without the
backup option. These are agent usability reviews of a simulated scenario,
not native macOS execution. Both guide/message retakes passed.

Three focused daemon checks passed: the public join adapter, the platform/role
table and the native unsupported-volume refusal. All five native restore
integration checks passed together, including time selection, folder snapshots,
missing contents, durable retry and member suggestions. These are Linux
observations and pure platform inputs, not a macOS runtime run. The full parallel
workspace run passed all 760 checks (two skipped) on the implementation before
the version-only v0.6.7 integration. After integrating that current main,
all 13 affected law/daemon/restore checks passed again. All-target workspace
Clippy, including trace, lab and optional console features, passed after
correcting a style diagnostic in the new test. Exact-head CI still precedes merging.

## Merge validation

Final combined source `b92d8deba60b8c52674a39b26b9eaafefa033a48` passed exact-head CI 39060412068, lab
39060412108 and bridge 39060412065. Main cache producers completed
their save steps before tagging. The full 760-check implementation run,
13 current-version input/role/restore checks, concrete-name law, 200
positive Authority histories and 200 final-verdict mutants, abstract TLAPS
checks, fold equivalence/mutant and nine-history end-to-end trace-driver
retake remain distinct evidence. The first combined CI head failed before
executing traces because nextest setup was missing; both calling workflows
now install the existing pinned action. No elapsed M6 or M7 gate is inferred.
