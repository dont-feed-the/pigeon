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
