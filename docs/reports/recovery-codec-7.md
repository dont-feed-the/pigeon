# M7: recovery-sheet input and property-case counts

> Mission: refuse unsupported recovery inputs without choosing an earlier authority.

## Original readers and regressions

Recovery-sheet types accepted unknown fields at the top level and inside
backup entries. The text reader searched backwards for a *valid* code,
skipping an unreadable or unsupported last candidate. Pasting an earlier
valid sheet before a later damaged or future sheet could therefore select
the earlier group's recovery authority. These are codec observations, not
a demonstrated remote signing bypass.

Five original regressions fail together in 0.199 seconds: unknown root and
nested fields, a later damaged code, a later unsupported version and input
larger than the existing IPC frame. The complete original is retained. An
intermediate digit-only version recognizer passes the first fixtures but
still skips a damaged prefix; a further regression fails in 0.007 seconds.
Both originals remain distinct from the final result.

## Supported input and refusal

Both version-one serde schemas are closed (ADR 0048). The optional legacy
`label` still defaults to empty. Serialization, checksum and key semantics
are unchanged. The last whitespace token whose prefix is `pgr`, ignoring
ASCII case, is the recovery-code candidate. That candidate alone is decoded:
unsupported versions, fields, corrupt bodies/checksums, case-damaged and
truncated prefixes refuse instead of selecting an earlier code. An earlier
unsupported example does not prevent a later supported code being used.
Normal paper labels and supported fields retain their values exactly.

The existing one-MiB IPC-frame limit is applied to standalone text decoding
before checksum work or base32 copies. Text exactly at the bound, including
padding, remains supported. This adds no smaller wire limit and does not
change the printed code format. Full field semantics, default-folder/path
ownership and the complete security review remain separate work.

Two laws run 1000 generated cases each: supported fields and legacy labels
round-trip through decorated text, and unsupported last candidates never
fall back to an earlier supported sheet. Fixed tests cover closed schemas,
future versions/families, malformed prefixes and both size boundaries.
All 227 daemon library checks pass against the combined source, followed
by 28 authority/capacity checks and eight lab-law checks. Current main
`16f0bfac5a2b84f020a0f26f782ec21d8e698c45` is integrated after those
checks and before rebuilding native programs; it changes version/lockfile
and release documentation relative to the tested PR #116 parent.

After that integration, the stable native rebuild passes the complete daemon
library suite again. All seven real-annex invitation, failed-join/retry and
recovery-sheet flows then pass together in 121.505 seconds, with no skipped
tests. The full paper-sheet recovery takes 119.238 seconds. Program and
runtime/test source SHA-256 hashes are unchanged throughout this final run.
No checksum, input, disk reserve or timing requirement is relaxed.

## Case-count audit

An attribute audit finds six remaining properties using 300 cases: one
file/pointer authority law, one per-custodian capacity law, and four pure
scenario/model-interpreter laws. All now use 1000. The previously corrected
signature laws remain at 1000. In this snapshot, all 277 explicit
`hegel::test` attributes specify at least 1000 cases, and none relies on an
implicit count. The before/after inventory is retained. This is a count
audit, not proof that every planned law exists or that every trace/refinement
obligation is complete; the broader M7 audit and elapsed gates remain open.
