# Pasting an invitation with a long message

> Mission: bound repeated parsing work while preserving the existing invitation
> extraction and strict decoding rules.

The pasted-invitation adapter previously concatenated and decoded every shorter
word prefix until it found a complete invitation. A long message after a valid
invitation therefore repeated work over most of the same input. This applies
to user-pasted text; it is not a demonstrated unauthenticated network exploit.

The replacement scans the base32 word run once, decodes its bytes once and
parses one typed JSON value. It then examines word boundaries for the longest
complete value, allowing only JSON whitespace after it and the existing strict
base32 lengths and zero unused bits. Invalid alphabet words cannot belong to
any longer valid prefix. Wrapping, case, closed schemas, version checks and the
trimmed-input fallback remain unchanged. It introduces no invitation authority,
wire format or content-access rule. Its work and temporary storage grow linearly
with the pasted text; this does not change the transport's existing input limits.

## Original and retakes

The original fixture places 100,000 one-character words after a valid synthetic
invitation. After building, a bounded two-second nextest invocation is stopped
by timeout; its test ran 1.616 seconds before SIGTERM and produced no passing
assertion. This is a controlled cutoff, not an identified network attack or
hardware-budget failure. The first attempt to create its worktree named a
not-yet-existing working directory and did nothing; creation was then separated
from execution.

The replacement passes that fixture in 0.033 seconds in the first 22-check
library retake. The identical bounded two-second invocation then passes the
fixture in 0.030 seconds. These individual wall times establish the controlled
retake, not a sustained throughput or latency percentile. Two hegel laws run
1000 cases each against the bounded original parser: arbitrary valid/unsupported
invitations with wrapping, JSON escapes and trailing messages; and arbitrary
candidate text. Explicit examples retain canonical padding-bit refusal and the
longest accepted prefix with encoded trailing JSON whitespace.

The entire protocol crate passes 35 tests in 0.314 seconds, including existing
wire/health/policy/removal/time laws. Initial all-target Clippy rejects two new
fixture idioms (slicing before as_bytes and a manual multiple check); both are
corrected without lint exceptions, and the workspace check passes with zero
warnings. The final example also includes encoded leading JSON whitespace. After
integrating v0.6.18 main `40bdaf1c1f75258fd2181bd49b6b65e1483cf3f9`,
all 35 protocol checks pass again in 0.307 seconds. The same rollup passes
24 affected resource-controller/native checks and all-target workspace
Clippy with zero warnings. Its separate idle-reconciliation change is
recorded in [the native preset report](native-presets-7.md).

Current-main integration and exact-head CI still precede merging. This pure
input-adapter change does not close the wider joining/signing security review,
native resource queue, measured M6 budgets or actual elapsed M7 gates.
