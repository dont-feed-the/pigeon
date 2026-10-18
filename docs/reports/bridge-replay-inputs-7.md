# Replaying repeated DAV observations

> Mission: shorten repeated model replay while checking every raw observation
> and preserving the solver's acceptance boundary.

The saved original PR #87 bridge run has 389 DAV transactions, many of them
repeated unsuccessful reads while a native save is pending. Launching TLC
for every identical normalized input spends CI time without adding a new
model behavior. This change keeps a successful verdict only within one
replay invocation, keyed by the exact normalized event encoding, method,
resource, target and both TLA model source files. Every raw stream still
passes JSON parsing, duplicate-field checks and complete normalization before
lookup. A different input or changed model invokes TLC independently. Failed
verdicts are never cached. Negative mutations still invoke TLC individually.

The original implementation launches 1000 solver calls for a fixture with
1000 equivalent streams; the focused regression requires one and fails
before this change. All five regression checks now pass, including later
malformed clocks, unknown fields, truncation, different constants, changed
responses and changed model source. The full saved PR #87 observation accepts
all 389 transactions using 18 distinct TLC inputs. This is an invocation
count improvement, not a measured publication-time or failure-rate claim.

Six deterministic Rust bridge tests pass and generate 13 distinct positive
transactions; TLC accepts all 13. All 16 deliberately corrupted observations
are rejected independently, and truncated and empty streams are refused.
No model, expected verdict, production behavior, deadline or publication gate
changes. CI repository-rule discovery includes the new Python checks.

The local scripts suite passes 80 checks. Packaging discovery initially
reports two systemd verifier failures because this shell has no user runtime
directory; using a fresh mode-0700 XDG_RUNTIME_DIR passes all 32 applicable
checks, with the native Mac observation explicitly skipped. The native Mac
result above remains separate. Workspace all-target Clippy, formatting,
file/header, ADR-index and spec-reference checks pass before this commit.
