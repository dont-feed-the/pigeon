# M6: reduce catalogue grouping and retained-view memory

> Mission: bound temporary comparisons and cached views without changing catalogue values.

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

## Retained historical views

The initial grouping fix does not address the engine's cache: up to 256
complete views were retained regardless of their size. A separate private
probe using 10000 files, six accepted contributions and one-file changes
observes additional requested live heap grow from 18897759 to 99542511
bytes after callers drop each view. Dropping the engine releases it. This
identifies retained views rather than live caller references as the cause
of that growth in this fixture.

The cache now limits both its 256 states and 32768 logical collection
entries. Its weight counts state membership, visible leaves, shown versions
and package children, writer/person sets, heads and rename bookkeeping.
Deleted-slot metadata counts even when the visible tree is empty. This is
a cache policy, not an allocator byte estimate or an overall memory limit.
One oversized current view stays reusable; the next cache miss releases the
cache's reference before computing its successor. Smaller entries are
cleared when their combined weight would exceed the limit. Callers' owned
immutable `Arc` snapshots remain valid after eviction. The engine's event
history, authority decisions and externally held views are separate live
state and are not erased to satisfy the cache limit.

Per-file shown/head vectors also start with room for one item, growing for
real conflicts instead of reserving four entries for every ordinary slot.
The temporary version vector reserves for the known heads count. Content,
representative order and conflict naming remain unchanged.

## Borrow comparisons; own changed events

Judging a contribution previously cloned each complete file entry into both
comparison maps. A warmed-cache regression with 256 unchanged files
observes 512 clones and fails its zero-clone target. Comparisons now borrow
the leaves using the same grouping function; only changed event writes
clone owned entries. Public `units` still returns the same owned values.
A separate 1000-case law requires borrowing and then owning every unit to
match the full owned grouping, including packages and Unicode/case collisions.
The clone-count original is retained separately from the corrected run.

## Combined validation and CI policy

The two cache laws run 1000 cases each: retention stays within the entry/state
policy (allowing one oversized current view), and eviction preserves outcomes
and all public view fields compared with uncached judgement. Owned snapshots
survive cache clearing. A fixed case checks deleted-slot metadata weight.
Together with the initial grouping laws, this rollup adds five 1000-case laws.
The existing schedule laws, full Python oracle and trace producer still run.

PR #115 originally validates grouping alone: CI 39148745250 passes, while
the bridge run is cancelled for the changed-source rollup. Its original
head `48249301aed052f4910aa9da8c0867b6324e5b41` and controller records are
retained. The waiting merge controller is stopped before combining the
remaining memory work in that PR. Final source receives fresh exact-head
validation once; no unchanged-source CI retry is used. The next release
still requires all four tag gates and independent asset/install verification.

## Full-count pure-engine comparison

A second probe uses 100000 real-shaped `authority::Entry` leaves. An empty
genesis is followed by six accepted contributions: the first adds the tree,
and each later contribution changes one entry. After each admission it
computes the current catalogue, compares every entry with the input and drops
the caller's view. All variants run as separate fresh single-thread processes
using identical synthetic inputs and probe source. The baseline already
includes the initial ordinary-file grouping fix. Each intermediate result
is retained; these are one run per variant, not production timing trials.

| Variant | Additional live heap after sixth view | Maximum requested Rust heap, including input | Maximum observed process RSS |
| --- | --- | --- | --- |
| Initial PR head, 256-state cache | 995308431 B | 1030498303 B | 1201256 KiB |
| Weighted cache | 188917079 B | 274052198 B | 353832 KiB |
| Plus compact per-file vectors | 143317079 B | 228452198 B | 310444 KiB |
| Plus borrowed comparison entries | 143317079 B | 206450358 B | 267304 KiB |

The input alone requests 28294980 live bytes. The final variant retains
about 86% less additional heap than the initial PR head. Its maximum observed
RSS is 273719296 bytes, but **this is not a daemon memory-budget pass**:
the probe has no native Git/annex children, signed histories, transfer plans,
50 GB contents, platform services or ten-minute measured window. Original
JSON observations, probe source, all four frozen binaries and SHA-256
receipts remain retained. The full reference and HDD gates remain open.

The combined source passes all 14 selected core checks, including the full
Python oracle replay, the trace producer, five new 1000-case laws and the
zero-clone regression. All 215 daemon library checks pass again after the
borrowed comparisons. Formatting, all-target workspace Clippy with zero
warnings, file/header limits, ADR index and spec references pass. The
original 512-clone failure and intermediate cache-only/vector-only RSS
observations remain retained; they are not replaced by the final result.

## Integrated release validation

PR #115, head `d201f1696b5cb7313bd12e23a46d9acde5b6ce42`, integrates main `95f2643a2fbc7aa6b9b8e15ddc6a6b5391055e97` before validation.
CI 39150066046, lab 39150066111 and bridge
39150066015 pass. v0.6.45 is independently published
and verified first. Local checks pass all fourteen selected core laws/oracle/trace
cases and 215 daemon library checks. Five new laws use 1000 generated cases
each. All-target workspace Clippy and repository rules pass with no warnings.
The retained command-selection error starts no tests; the corrected library
invocation passes without changing source. The initial native allocation
baseline and all six alternating comparison trials remain retained.
Fresh v0.6.46 tag gates and independent archive/install verification are
required. No daemon memory, HDD, security or elapsed release gate is closed
by this operation-level change.
