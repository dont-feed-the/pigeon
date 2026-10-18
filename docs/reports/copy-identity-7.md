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
all-target Clippy, formatting and repository rules pass. Current-main
integration, exact-head network checks and CI still precede merging.

This strengthens the physical-copy checks beside
[stream-owned lookups](sync-lookups-7.md). It supplies no missing reference
budget, HDD, elapsed-night or release-candidate qualification.
