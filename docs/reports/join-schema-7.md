# Joining compatibility and proof comparison

The joining review found that typed admission did not inspect `Join.v`:
a correctly authenticated request with an unknown declared version could
reach ordinary or recovery admission. The native regression records the
original behavior before correction. Invitation, nested admitter/refounding,
join and answer serde types also accepted unknown fields, contrary to the
closed-schema requirement of ADR 0048. Five codec regressions record this.

The central welcome boundary now refuses an unsupported version before
recovery dispatch, roster lookup or an edit. Known current requests still
join. The localized refusal asks both computers to update and reuse the
existing invitation. Closed serde types reject unknown fields while keeping
all supported optional fields and wire serialization unchanged.

Invitation hash/proof comparisons now use the existing workspace's subtle
constant-time primitive rather than a custom byte fold. Native wrong-proof
checks exercise every hex position and shorter/longer/empty proofs, require
unchanged accepted history, then admit the valid proof. Functional tests do
not measure a timing side channel or establish a complete security audit.
The unchanged HMAC remains checked against RFC 4231 by existing codecs.

This does not change ADR 0075's bearer invitation, existing-person consent,
removed-device identity refusal or refounding policy. The remaining full
joining/signing and instrumentation reviews and elapsed M7 gates stay open.
Validation and current-main integration follow below after execution.

Before correction, all five schema fixtures fail and the native unknown-
version fixture receives Welcome for version zero. After correction all
18 protocol library checks, all 20 invitation/codec/format checks and all
18 Fluent checks pass. The native version test refuses four unsupported
versions on both ordinary and recovery paths without changing main, then
admits the current version. The proof test refuses all 64 changed hex
positions and empty/shorter/longer proofs without changing main, then
admits the unmodified proof. All-target workspace Clippy with lab/trace/
console features, formatting and structural/ADR/spec checks pass.
Current-main integration and exact-head CI remain separate before release.

The correction is folded into the scheduling rollup after integrating
current v0.6.14 main `5fc77893e687d30efe6fcb855535b2b0701dcbc9`. All affected protocol, native invitation/
codec/format, health, web/Fluent and native annex checks pass again, with
S2/S3 filesystem/roster TLC, all-target Clippy and repository checks.
Exact-head PR validation remains separate; the bearer invitation policy
and wider review gates remain as documented above.
