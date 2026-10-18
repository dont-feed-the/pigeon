# M7: default joining destinations

> Mission: received names do not choose filesystem paths outside the default base.

Three real-Git regressions fail together in 0.251 seconds against the
previous implementation. An absolute invitation name chooses a laptop
folder outside the home default; a parent component in the invitation's
group ID chooses a bare repository outside the configuration folder; an
absolute sheet name chooses a recovery folder. Each fixture owns the
entire target tree and verifies that an unwanted directory was created.
The original log, test source and declaration patch remain private evidence.
These observations establish unwanted destination creation, not a remote
code-execution or signing bypass.

Default laptop/recovery names must now be one portable component under
the existing naming rules (05 §5.4). A received path or nonportable name
returns `join-folder-needed` before creating its destination. The English
and French Problem tells the person how to supply an explicit folder.
Names are neither rewritten nor silently sanitized. An explicit folder
remains the person's choice, even when the display name cannot be used
as a default. Invitation and recovery group IDs must be the supported
40-character lowercase genesis IDs before destination creation; ordinary
invitation handling checks this before applying a received relay.

Seven focused tests pass, including the three original cases, both
explicit-folder controls, malformed group IDs with an explicit folder and
two 1000-case laws. The laws preserve the parent and exact spelling of
portable names and refuse paths, reserved/nonportable names and oversized
components. All 19 localization checks pass. An intermediate fixture used
an unavailable `toml` crate and failed to compile; the retained correction
uses the existing `toml_edit` dependency. That failure is not runtime or CI
evidence. The complete daemon library suite passes all 234 checks in
24.035 seconds; all-target workspace Clippy and repository rules pass.

After integrating PR #117's readiness correction at
`c80b1008c9d513f2eff1518eb909b00776ba4c85`, a stable native rebuild
passes all seven invitation, join-retry and complete-sheet recovery flows
in 113.230 seconds, with no skipped tests. The complete recovery takes
110.987 seconds. Program and runtime/test source SHA-256 hashes stay
unchanged throughout that run. This is compatibility evidence, not a
performance comparison with another recovery run.

This is lexical destination validation. Existing symlink aliases, concurrent
path replacement, folder ownership, every recovery-field semantic constraint
and the complete joining/signing review remain separate work. Current main
`8f8992c8e6a1a612ce3f86e9a8f993693c0bffe5`, the green PR #117 merge,
is integrated after that run. Its tree is identical to the earlier native
base, and the final runtime/test source hashes still match. Fresh PR
validation and publication are required; M6/M7 and v1.1 remain open.
