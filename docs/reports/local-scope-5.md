# Private work and local folder choices

M4 follow-up, 0.4.16, 16 October 2026. The final command and usability audit
found three ways that accepted state and a person's local state needed to
remain distinct.

## Changes

Every roster change now builds from the accepted tree, changes only the
roster blob, verifies the signed history and authority, then uses a CAS to
advance main. Only that metadata is staged afterwards. Folder-rule changes
use the same writer as invitations, recovery keys, collector choices,
computer removal and leaving. An unrelated staged file or open editor save
cannot become shared merely because one of these actions commits metadata.

Files and folder pages include uncaptured local paths and label them Local,
overriding the previous accepted version's safety for an unfinished edit.
A failure to read the listing or pending changes produces a problem instead
of an empty page. Pending changes are not promised as incoming downloads.

Local Free and Keep choices use the nearest explicit folder choice. Keeping
a child of a freed parent brings that subtree back without bringing back
its siblings, and covers future children. A later choice for the parent
replaces older child choices. These choices are saved atomically in format
1; legacy Free lists remain readable. A failed write returns a problem
instead of acknowledging an unsaved choice. The pure `keep_targets` algebra
has three property tests with 1000 cases each.

The help synopsis also includes `view --suggestion ID`.

## Verification

The real three-laptop folder-rule test keeps an editor open and an unrelated
raw blob staged while changing the collector and admitting a fresh person.
The accepted trees on both computers exclude the private file; its original
staged object and working bytes remain on the first computer. It passes in
10.82 seconds. Paper-key recovery with a declined ordinary-member suggestion
passes in 95.40 seconds; the two-refounding/in-place/fresh-join regression
passes in 32.02 seconds. Both real telemetry tests pass in 247.52 seconds,
including two actual SDK exports of acceptance and protection lags, the
selected direct path, all four implemented queues, consent and disabling.

The final local-file checks add a real parent-Free/child-Keep regression,
legacy format reload, new root and nested unfinished files in the actual
web pages, and the existing verified and forced drop and crash-recovery
targets. The first extended fixture incorrectly looked for a protected
file in unscoped status, which intentionally lists only files needing
attention; it now asks about that exact path. Both slice scenarios pass
in 52.88 seconds, the real pending-file pages in 0.94 seconds, and the
interrupted Free target in 15.97 seconds. The forced-drop target passed in
1.77 seconds. Legacy reload, all three 1000-case folder-choice laws, CLI
tests, all-target clippy with lab/trace features and repository checks pass.
