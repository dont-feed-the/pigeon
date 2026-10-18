# M6: reduce temporary catalogue grouping memory

> Mission: remove oversized per-file grouping allocations without changing catalogue values.

## Allocation cause and change

`catalogue::units` groups a Git tree into ordinary files and whole packages
(05 §5.1, 07 §7.9). Previously every ordinary file created an inner
`BTreeMap<String, Entry>` holding one empty relative path. Its first node
reserves eleven entries, even though an ordinary file needs just one leaf.
The real Git `Entry` occupies 72 bytes on this Linux build before its
three owned strings. Many temporary nodes coexist before the final units
map has been built.

The grouping now stores an optional boxed leaf alongside the children map.
Ordinary files allocate one leaf; packages allocate their relative-path
maps as before. The outer map still sorts literal unit names before
collecting folded slots, preserving the winner when arbitrary inputs have
colliding spellings. A package-root leaf with children is still discarded;
a root without children is still a file. No object identity, catalogue
format, authority rule or package merge behavior changes.

## Controlled allocation comparison

A private, single-thread Linux probe constructs 100000 distinct ordinary
files using real `authority::Entry` values, with 40-hex synthetic object
identifiers. A counting system allocator records requested Rust heap bytes,
not allocator rounding. It resets the peak after building the input tree,
then calls `units` once. It verifies every output path and complete entry
against the input after recording the allocation peak. These are synthetic
identities, not actual Git objects or a transfer fixture.

The original implementation is frozen from source
`474fa4b11ece1904f21615ce78a4ca1dfdc5a8e8`. Both binaries use the same probe
and development build settings. Six fresh processes run in the order old,
new, new, old, old, new on one shared Linux host:

| Requested allocation | Original, each of 3 trials | Changed, each of 3 trials |
| --- | --- | --- |
| Input tree live before grouping | 28294980 bytes | 28294980 bytes |
| Additional peak while grouping | 132028272 bytes | 34421112 bytes |
| Additional live result | 22421112 bytes | 22421112 bytes |

The peak reduction is 97607160 requested bytes, about 74%. The final live
result is identical. Process RSS high-water ranges are 186768–186868 KiB
and 89764–89832 KiB respectively. Median operation times are 1.649950 and
1.536024 seconds; these shared-host development timings do not establish a
production throughput improvement. The original and changed probe binaries,
source, SHA-256 receipts, all six JSON observations and initial baseline
log are retained separately.

This operation-level improvement does not establish the daemon's 300 MB
budget. The previous full-size reference preparation exceeded that budget
on both peers, and later planning also retains other catalogue and transfer
state. See [the original resource diagnostic](metadata-startup-7.md).
Full reference preparation, measured Gentle window and physical HDD evidence
remain required; no finite allocation probe closes them.

## Behavioral checks

Two new laws run 1000 generated cases each. The first compares the optimized
function with the original grouping over arbitrary trees, including empty
components, Unicode-equivalent names, case collisions, package roots and
nested packages. The second expands disjoint file/package units and requires
all original leaves and values, with the expected unit count. Fixed cases
assert root-leaf handling and literal-order collision winners directly.
The existing catalogue schedule laws, Python-oracle cases and trace producer
also run against the changed implementation.

The focused macOS capture/metadata result for the preceding change is
recorded separately in [its metadata report](metadata-startup-7.md). It
predates this allocation change and supplies no macOS evidence for it.

All eight selected catalogue checks pass, including the existing Python
oracle replay and trace producer. After integrating main
`95f2643a2fbc7aa6b9b8e15ddc6a6b5391055e97`, all 215 daemon library checks
pass. All-target workspace Clippy and repository checks pass. An initial
library filter used an invalid binary identifier and started no tests;
its original command log remains retained. The corrected `--lib` invocation
passes with the same source. There is no CI retry or source regression in
that command-selection mistake.
