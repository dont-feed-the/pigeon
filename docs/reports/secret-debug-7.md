# Secret formatting checkpoint

ADR 0022 requires secret-bearing types to redact Debug output. The network
configuration derived Debug over the device's 32-byte signing key; a served
repository did the same over its annex authentication token; the invitation
did so over its joining secret. Three focused fixtures reproduce those
formatting disclosures without printing the synthetic secret values. Both
ordinary and alternate Debug formatting, including nested repositories,
are exercised. This demonstrates a formatting boundary, not evidence that
a production log previously recorded any particular credential.

Explicit Debug implementations retain useful endpoint/group/role metadata
and print `[redacted]` for those secret fields. The repository still
reveals whether a token is configured. The actual key/token fields and
invitation serialization are unchanged. The invitation fixture also checks
that the encoded invitation round-trips with its full authority intact.
The broader audit of API request/response objects, recovery/join proofs,
error paths and instrumentation remains open; this checkpoint does not
claim every secret-bearing type is covered. Validation follows below.

All three original privacy fixtures failed before correction and pass after
redaction. Their intended key/token values remain accessible, and the
invitation codec retains its exact value. Full-target lint and integrated
validation remain separate from this focused formatting evidence.

Final all-target workspace Clippy with lab, trace and console features and
repository header/length, ADR and spec checks pass for the combined checkpoint.

Formatting and existing logging/annex contracts pass after integrating
current v0.6.12 main, along with all-target Clippy and repository checks.
This does not close the wider secret-type/security review.
