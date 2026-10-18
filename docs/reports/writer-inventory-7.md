# Linux capture writer inventory

This is a native process-enumeration optimization, not a passing M6 budget.
The earlier full reference preparation timeout remains separately retained;
its exact capture bottleneck has not been established from that artifact.

## Repeated work and correction

Capture previously scans every process's descriptors and memory mappings
separately for every candidate file. The work multiplies process-table
size by candidate count before the batch can stage any version. A local
comparison exercises the real kernel scanner on 256 private files with an
open writer, checking identical writer decisions in both implementations.

Linux now takes one fresh writer inventory for the queried inode set per
capture candidate pass. Writable descriptors, unknown descriptor flags and
shared writable mappings count as busy. Read-only descriptors and private
mappings do not. An inode absent from the queried set is never clear, so a
replacement observed afterward waits for another pass. Failure to enumerate
the process root keeps every candidate busy. The inventory is not cached
across rounds and is not a lock or authorization.

The existing settling and complete-stat/exact-byte checks remain. Capture
continues its previous observed-writer semantics; install's credential-aware
single-file `clear_for` scan stays separate. Non-Linux capture still uses
its original per-file scanner; no native Mac performance claim is made.

## Local observations

Three alternating-order comparisons on this kernel have median per-pass
costs of 4.242103 seconds for 256 single-file scans and
5.241 milliseconds for the shared inventory. Each comparison
requires exactly equal decisions for every queried inode. The earlier
serial-first measurements are retained separately. These are local
primitive timings, not full-tree, CPU-window, physical-HDD or publication
measurements; they cannot explain the earlier full timeout by themselves.

All 37 system-library/native cases pass in 0.210 seconds, including open
writers, read-only descriptors, hardlinks, shared/private mappings,
replacement identities, unavailable process enumeration and 1,100 decoded
mapping combinations checked against the single-file rules. The manual
timing case is explicitly ignored in routine runs. All 178 daemon-library
cases pass in 17.617 seconds. All 64 native install/capture cases pass in
27.398 seconds, including writer deferral, settling, hot saves, exact bytes
and crash replay. All-target workspace Clippy has zero warnings.

This initial validation is on the v0.6.28 base. Current main is integrated
before hosted PR validation. Both full reference/HDD budgets and the elapsed
M7 gates remain open.

## Full-size diagnostic before this correction

Run 39104345692/job 117606899294 on ordinary-build source
`8a7ed457be15a0d3af38160a1edb509b69a6e64b` reaches its shorter 600-second
preparation limit, with no measurement window. The source's scan markers
report 100,000 pending paths in 92–141 milliseconds. At failure its index
still has two metadata entries and its diagnostic object count is zero;
the backup also has zero entries/objects. Source cgroup CPU totals about
602 seconds, including native descendants, while backup CPU totals about
3.4 seconds. The source daemon is about 112 MB resident. These are active
preparation observations, not an idle-CPU budget failure or pass.

Original artifact 12127863489 is 1,201 bytes, SHA-256
`ea2a68397bc031055781f194e8f1898af97de749ead9589711b95fe13a3558fb`.
It is retained without retry. The observation locates a CPU-heavy interval
before user files are staged; it does not identify a sampled stack. The
process-enumeration comparison above independently reproduces one repeated
cost in that interval. A new-source full-size run is needed before claiming
its measured end-to-end effect. New diagnostics also retain live-process CPU
ticks and identity fences; completed writer inventories log only counts and
duration.

The repository app cannot manage Actions variables. The follow-up therefore
also supplies a tracked nightly-campaign expiry, 26 October exclusive at UTC
midnight, which the existing policy uses when no variable override is set.
It expires automatically and does not manufacture a scheduled run or a
green night. Expired overrides preserve the routine policy. Complete actual
Linux evidence is still required. The campaign's source and expiry are
reviewable through the normal PR without additional administration access.


The combined diagnostic/policy follow-up integrates the corrected S2 and
join-preservation source before its next hosted validation. At this local
checkpoint, 179 daemon-library cases pass in 17.650 seconds, 120 script
contracts in 3.429 seconds and all 16 native capture-scheduling cases in
18.508 seconds. All-target Clippy and repository checks pass. The subsequent
same-repository native cancellation correction and latest main must still
be integrated before the writer optimization's PR is validated.


After integrating native cancellation head
`8881da11da4088ef8c2181e3e02c1173703cf847`, all 234 combined system,
Git-driver and daemon library cases pass in 17.886 seconds. The system/native
suite passes 37 cases in 0.193 seconds (the manual timing case stays ignored),
120 script contracts pass in 3.219 seconds and all 54 non-trace native
install/capture cases pass in 22.187 seconds. All-target workspace Clippy,
formatting and repository checks pass. A stale local shared-target system
metadata artifact initially caused missing-export compilation errors;
rebuilding that package resolves them without changing source.
A clean pushed source head is required for the next full-size measurement.


## Functional protection waits under native I/O pressure

The final PR #98 head's original held-deletion fixture expires its initial
Protected wait with 102 valid source objects, only 26 valid backup objects
and both peers on the same captured main. Source artifact 12129121185,
SHA-256 `0e4445f2ffdccac74471a81659966e385e5b7d1d841f43308d5f331b79287d6b`,
retains repeated native I/O-pressure pauses and continued placement. One
unchanged-source shard retry passes. No source/timeout change is made to
that release candidate, and no further retry of the original is allowed.

This follow-up gives functional Protected waits their existing 120 seconds
of active work rather than charging the declared disk-pressure waits that
spec 15 §15.4 deliberately permits. A five-minute wall bound remains.
Only the known disk-wait headline, while no capture is pending, stops this
test clock. Capture and other waits retain their original elapsed budget;
missing copies, wrong states and absent captures still fail. Native default
resource limits remain unchanged. Existing ActiveClock laws cover clock
accounting; the native functional suites validate the real protection
predicate. The separate reference/E17 performance requirements are unchanged.


Main v0.6.29 candidate `47dc25a07b21c83b43505e63ac8f99f51db91b9a` is
integrated before the next hosted PR validation, with no PR version change.
All 234 combined library cases pass again in 18.123 seconds and all 120
script contracts pass. Both held-name native cases and all 16 native capture
scheduling cases pass together in 22.188 seconds. All-target workspace
Clippy has zero warnings; formatting and repository checks pass. The
clean-source full-size diagnostic remains pinned to
`589fdbd19c8a4d6ccac62d433a05bcf0916a3e14`; subsequent changes here are
release-version integration, documentation and functional-test waiting.
No production Rust source differs from that measured feature head.


## Clean-source full-size observation after inventory batching

Run 39107288813/job 117615795367 uses ordinary-build source
`589fdbd19c8a4d6ccac62d433a05bcf0916a3e14` with a clean worktree and the
600-second preparation limit. It generates all 100,000 real 500,000-byte
files. Writer inventories for those candidates finish in 293–605 ms.
Before the limit, the source reaches 23,170 index entries and 23,257
unverified annex object files, rather than the previous diagnostic's two
metadata entries and zero object files. The accepted complete source and
backup trees and backup objects remain zero: no capture commit is complete
and no independently hash-verified measurement window starts.

The source cgroup uses about 435 seconds of CPU including exited children;
the live daemon has 24.63 CPU seconds and a 179,355,648-byte resident
high-water mark. These preparation observations locate most CPU outside
the daemon; they are not a passing CPU or memory budget or an attribution
to a specific reaped native command. The scanner comparison independently
establishes its own repeated-work cost, without a full-workload speedup claim.

Original artifact 12129780898 is 1,758 bytes, SHA-256
`15b68665f2ce2fe79d321a2c780aeede0b1fd7a1aec5851db66f801cfb1eb66d`.
The failed diagnostic is retained without a retry. The full preparation
option and every complete-tree/object/window requirement remain unchanged.
Native watcher/overflow behavior and ten-minute scan scheduling are also
still open in the conformance inventory; this optimization does not claim
complete ADR 0013 implementation or close M6.

## Integrated release validation

PR #99, head `0e345d81ea6c114e7405a29f8f2a24e935b97ca0`, integrates main `47dc25a07b21c83b43505e63ac8f99f51db91b9a` before
validation. CI 39108827235, lab 39108827262 and bridge
39108827236 pass. v0.6.29 is published and independently
verified before this candidate. Local validation passes 234 combined library
cases, 37 system/native cases, 120 script contracts and 54 native install
cases, followed by 18 combined held/capture cases on current main.
The full reference/HDD budgets, native watcher and real elapsed M7 gates
remain separate; no passing resource budget is inferred from the primitive
timing or the incomplete preparation diagnostic. Fresh v0.6.30 tag gates
and independent archive/install checks are required before publication.
