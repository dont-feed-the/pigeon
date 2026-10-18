# Complete content identity at the copy barrier

A native regression at source `b63cb38ef2db66cf9d967cd86a6106c6fd04a000`
creates an actual annex object whose SHA-256 matches its canonical SHA256E
key while its byte length is one smaller than the key declares. Native
git-annex 10.20261006-pigeon2 reports its `contentlocation`. The ordinary
custodian has no optional shared-provider registration. Its hash-checked
network barrier incorrectly returns true; the original failing test and
separate native setup receipt remain retained.

This is a validation gap, not evidence of observed data loss. For ordinary
non-thin content, upstream `Annex/Content/Presence.hs`'s `inAnnex` checks
existence. A location reply does not independently prove the complete
content identity. The shared pool verifies size and digest when registered;
its optional verifier cannot supply this check for ordinary repositories.

The barrier now uses the existing pure `pigeon_core::export::matches`
predicate, checking size and SHA-256 together. Size counts the same streamed
bytes used by the digest, rather than a separate file-metadata observation.
The existing digest-only helper retains its interface. Native lookup reuse,
fresh object lookup, durability syncs, mismatch repair and un-hashed
durability answers retain their behavior. No positive result is cached.

The real native fixture independently checks the installed bytes and native
location, then requires the wrong-size key to fail and the correct key to
pass for both laptop and ordinary custodian roles. All 240 daemon-library
checks pass, including later corruption/removal, native batch boundaries,
helper lifecycle and the actual pinned native capability. Workspace
all-target Clippy, formatting and repository rules pass. Current main `1cdf1cc8ae12463c530c365e6ae92dca355c960c` is integrated before validation. The exact-head native regression and existing 1000-case matching law pass. All seven native capacity, slice, health and status checks pass in 57.636 seconds with pigeon2, with unchanged source and program hashes. PR #121 passes CI 39174375525, lab 39174375537 and bridge 39174375559. All 164 script contracts also pass, including source-bound companion versions. Fresh v0.6.52 tag gates and independent publication verification remain required.

This strengthens the physical-copy checks beside
[stream-owned lookups](sync-lookups-7.md). It supplies no missing reference
budget, HDD, elapsed-night or release-candidate qualification.

## Historical archive identity

The archive preparer's hard-coded `-pigeon1` suffix rejects the legitimate
pigeon2 companion while accepting a different version ending in pigeon1.
An end-to-end executable archive fixture retains all three original misses.
The preparer now reads VERSION at the already resolved immutable source,
checks its bounded Git blob and single version assignment, then compares
the executed companion's complete version. It never executes the version
file. Correct pigeon1 and pigeon2 fixtures pass; stale companions, mismatched
upstream versions, malformed pins and changed blob metadata are refused.
This is source-bound archive preparation, not historical-data upgrade proof.
