# Network diagnostic privacy checkpoint

Serving-side debug events retained and formatted the last 96 bytes in each
relay direction. Even a legitimate authenticated stream can leave connection
credentials or private file fragments in those tails. Refusal diagnostics
also echoed the peer's complete refused line. Both violate ADR 0022's rule
that diagnostics must not include secrets. The default info level does not
justify exposing them when debug logging is enabled.

Two live tracing-subscriber fixtures reproduce both leaks using synthetic
credentials/private fragments, without printing their contents. The first
feeds both directions across transport chunk boundaries and still requires
exact byte counts, peer and a real relay-completion event. The second
requires the refused rule and peer while denying private input echoes.
Both original fixtures fail the privacy assertions.

The relay now retains counts only. Refusals record peer, rule and line
length. Protocol framing, refusal decisions, data forwarding, durability
barriers and the separately typed conformance traces are unchanged. This
corrects these two diagnostic boundaries; it is not the complete secret-type
Debug, joining/signing security or instrumentation audit required by M7.
Validation and current-main integration are recorded after running.

The corrected privacy and existing logging contracts pass together: seven
checks, including retained numeric counts, rule metadata, logging filters,
nonblocking overflow, shutdown draining, bounded JSON and file retention.
The real git-annex copy/get/refused-remove integration also passes. Final
all-target workspace Clippy with lab, trace and console features and the
repository structural/ADR/spec checks pass. These targeted checks do not
claim the broader M7 audit is complete.
