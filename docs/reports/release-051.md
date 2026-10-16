# Report: directory aliases during in-place rejoining

16 October 2026. Version 0.5.1 is the publication candidate for the completed
[M4 implementation](people-5.md). The v0.5.0 tag remains immutable at
`d8ede326c86c9140420a6b7a833fe63457346766`; it was not published.

## What stopped v0.5.0

PR #45 merged after all 22 current-head checks passed. The tag's actual
cross-runner relay passed (run 38878264291), and both pinned-annex platforms
and their installation checks passed. The macOS Rust workspace job
116898467894 in run 38878264360 found a configuration replacement failure.
The release was cancelled before publication, together with the remaining
superseded tag jobs, to preserve CI capacity. Partial model or scenario
results are not reported as complete passing workflows.

`replace_group` canonicalized an existing configured path but compared it
with an unnormalized replacement path. macOS's `/var` alias made the same
existing folder compare unequal, leaving its old entry and appending a new
one. The writer now canonicalizes both sides while preserving the requested
path spelling, group position, other repositories and unrelated settings.

## Verification

The existing regression now creates an actual directory symlink on every
runner. It reproduced the exact stale-entry/extra-entry failure on Linux
before the fix and passes afterwards. It still requires byte-for-byte
idempotent replay, the other repository with the old group ID retained,
original group order and preservation of comments and unknown settings.

Local candidate checks passed: `cargo check --workspace`, all 40 daemon unit
tests, both real refound crash/restart tests with the matching 0.5.1 helper
binaries, formatting and all-target Clippy with lab and core/daemon TLA
features and warnings denied. Hosted checks and a successful new tag's
platform/release gates remain required. Neither v0.4.0 nor v0.5.0 is moved.
