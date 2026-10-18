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
