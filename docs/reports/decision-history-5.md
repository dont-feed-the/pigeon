# Historical decision proofs during recovery

M4 follow-up, 0.4.15, 16 October 2026. The five-participant usability round
found that two fresh joins refused histories with a declined suggestion.
The preceding repair fetched live proposal refs. This follow-up audits the
same dependency in paper-key recovery and archived histories.

## Changes

A historical decline/withdrawal now verifies the proposal's signed object
and its actual file change against its accepted base. Verification does not
require a live proposal ref, since an archived proposal belongs outside the
current group's live suggestions. An authorized ordinary capture or a commit
signed outside the base roster cannot be substituted as a proposal. File
proposals cannot change pigeon metadata, avoiding recursive decision claims.
Missing metadata remains provisional and can be retried after it arrives.

Paper-key recovery bundles include retained proposal refs and import them
only into the source peer's namespace. Normal proposal admission checks
signatures and permissions before making live refs. A refounded group keeps
at most 31 signed archive links and their valid proposal proofs. It verifies
each linked old history, binds its canonical archive ref to the signed tip,
and retains proposal objects for onward relay through another recovery or
refounding. In-place joins rebuild this chain from the private stage rather
than copying arbitrary incoming archive names into serving refs.

No historical author, signature, roster or accepted ADR is rewritten. These
changes do not expire proposals or release any content.

## Verification and findings

The metadata-retry regression now removes and restores the actual generated
proposal object. Merely removing a live ref is not missing proof when signed
objects remain available. Its original synthetic roster accidentally made
both people stewards; the strengthened fixture uses a real non-steward
proposal, and separate cases reject an authorized capture and an unknown
signer masquerading as a proposal. All eight merge tests and archive-history
regressions pass.

The real paper-key recovery with a declined ordinary-member suggestion
passed in 100.91 seconds. A strengthened refounding fixture exposed a Git
ref-prefix collision between a new shortened proof name and the existing
archive naming scheme. Proof retention now preserves the established
three-component proposal suffix, so a ref never also has to be a directory.
New real-device fixtures put a declined ordinary-member suggestion into a
paper-key recovery and into a refounded group's history. The refounding
fixture also retrieves an older exact version on fresh computers across two
signed refoundings. The repaired paper-key recovery passed again in 98.29
seconds; the expanded two-refounding/in-place/fresh-join regression passed
in 38.44 seconds. The actual suggestion/preview/join target passed in 32.03 seconds in
the same final integration sequence.

Two fixture/environment findings are recorded separately from product
passes. Old generated annex fixtures accumulated because read-only object
directories prevented TempDir cleanup; this pushed the test host below its
backup reserve, and placement correctly deferred. Test-owned temporary
roots now restore owner directory permissions without following links on
cleanup. Stale generated integration roots were removed, preserving named
usability failure fixtures. The full metrics run's affected slice passed
after capacity was restored.

A fixture edit made immediately after an incoming version became visible
correctly created a conflict copy under ADR 0071, rather than the suggestion
this scenario required. Setup now waits for journal/index completion and
the documented twelve-second lab interval. This is a fixture correction,
not a bypass or change of the ten-second safety behavior.

Final all-target clippy (including lab and trace features), formatting,
file/header checks, ADR-index checks and specification-reference checks
pass. The final real integration sequence passes all three targets; the
new fixture cleanup leaves their generated roots removed. This patch is
ready for hosted CI. No release or M4 completion is claimed here.
