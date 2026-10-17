# Buffered data and join input diagnostics

A follow-up typed diagnostic audit found four more derived Debug values:
the pasted invitation in `invite::Ask`, partial local socket JSON/raw bytes
in `LineDecoder`, arbitrary Git object data, and explicitly configured child
environment values. Four actual formatting regressions fail before the
correction; no production logging call is needed to reproduce the boundary.
This does not establish that a prior log captured a live secret.

Explicit formatting now redacts the invitation and environment values,
keeps environment variable names, and summarizes socket/object bytes by
length. Object identity and kind and join metadata remain available.
Actual values and intended child delivery, stream rest and object data stay
unchanged. Existing codecs and native transfer behavior are independent
checks; this does not complete every error/dependency instrumentation audit.

Validation and current-main integration are recorded after execution.

All four regressions pass after correction, including ordinary/alternate
formatting and unchanged data delivery. All-target workspace Clippy with
lab/trace/console features, formatting and repository structural/ADR/spec
checks pass. Broader affected selections are repeated after current-main
integration with the preceding private-state and wording corrections.

Integrated current main `a9f8447d09af85a9c5938554038e65346c6e62c6` before repeating all core/protocol, Git-library, daemon-library, codec/format, native annex and backup-rejoin checks. All pass, followed by all-target Clippy and repository checks. Exact-head PR CI remains separate.
