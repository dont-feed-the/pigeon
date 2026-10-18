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

## Release validation

PR #87, head `4788c78c5ce0373539e51b8912615b00389613f2`, passed exact-head CI
39084909214, lab 39084909025 and bridge
39084909218. Current v0.6.18 main was integrated
before validation. The local 35 protocol and 24 affected daemon checks,
four 1000-case parser/mode laws, original controlled parser cutoff, idle
assertion and fixture-lint misses remain separate evidence above.
The original bridge custodian job 117549755010 timed out waiting for
the regular peer’s offline save. Fixture artifact 12119963498 is
12906833 bytes, SHA-256
`52012fcf23419fc0aad84e15a66211d6e888bb96a03e22912be4095d0b5ac36a`.
It retains the bare provider, working bytes and 206 native-path DAV 404s,
but ordinary .git directories were excluded by the upload default.
Repeated metadata-fetch timeouts and slow native Add/whereis are observed;
the exact CI interleaving is not established. Matching binary artifact
12120019675 is 50003892 bytes, SHA-256
`f9a1edc85d2ded7b9e71a8d54534a064e9dfb6e30259df6769be345476f863f0`;
its synthetic merge tree equals the PR head. The fully traced local retake
passes four behavioral checkpoints, TLC accepts one lifecycle and all37 DAV
transactions. Earlier local trace-configuration mistakes are excluded from
that validation. One bounded same-head CI retake, job 117552306313,
passes on workflow attempt2. No deadline, assertion or release gate changes.

Main cache preparation is not an extra pre-tag gate for the unchanged
dependency lockfile; active main Rust producers remain protected. All
required tag CI/lab/slice gates and both archives precede publication.
v0.6.18 was published and independently verified before this candidate.
The full security review, complete global queue, Mac dynamic presets,
own-read latency feedback, measured reference/HDD budgets and actual
elapsed M7 gates remain open.
