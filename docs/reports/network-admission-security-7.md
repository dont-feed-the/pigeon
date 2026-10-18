# M7 joining/signing review: bounded application admission

> Mission: bound connection/stream handlers while retaining authorization fences.

This follows [authentication before waiting](join-authentication-security-7.md)
and implements proposed [ADR 0084](../adr/0084-bound-incoming-peer-application-work.md).
The original accept loop spawns one application task per incoming connection
and one per annex stream, with no application concurrency cap. This is source
inspection of a resource-exhaustion risk, not a reproduced network exploit.

The endpoint now admits at most 64 connection/handshake handlers and 128
annex stream handlers across those connections. Owned permits are acquired
without waiting before spawning; excess work is refused or reset. Handshakes
have the existing ten-second header deadline. Finished stream records are
drained before each accept, and an annex connection with no live streams
closes after ten idle seconds. Active streams keep their ordinary lifetimes.
Capacity admission never supplies roster membership or authorizes a command;
all existing group, proof, role and content checks remain authoritative.

Four real QUIC cases initially pass in 10.024 seconds. Test limits of one
connection/stream expose refusal and capacity recovery without requiring a
large flood. A cross-connection stream case proves the shared budget. The
production ten-second annex idle case proves closure and slot release; a
shorter idle fixture proves repeated accept deadlines retain a live stream.
The completed-task drain is included before broader regression validation.

The wider security review remains open. These bounds do not establish
per-identity fairness, packet-rate resistance or bounded transport-owned
pre-admission memory/CPU. A valid invitation holder waiting behind a busy
repository can retain a bounded slot; its proof still gets rechecked under
the group lock. Recovery imports, complete signing/authority law coverage
and the remaining review inventory retain their separate gates. No M6 budget,
nightly streak, elapsed soak or release-candidate dogfood pass is claimed.

After the completed-task drain, all 187 daemon library cases pass in 21.042
seconds. Seven native invitation, failed-join retry and recovery-sheet cases
pass in 68.977 seconds after rebuilding matching binaries. This includes
refounded invitations, every altered proof position, preserved failed-join
files/UUIDs and recovery of an admin from another person's backup. All-target
workspace Clippy and the structural repository checks pass.

## Integration base

The follow-up integrates current main `c749d300798f047dfe9139fe14219edfe0dffa30` (the v0.6.32 candidate)
before its own validation. Its preceding publication keeps separate gates.

## Integrated release validation

PR #102, head `03af2f07ae6bb056c7887c106e4ba02eaa744949`, integrates main `c749d300798f047dfe9139fe14219edfe0dffa30` before
validation. CI 39116735611, lab 39116735614 and bridge
39116735576 pass. v0.6.32 is published and independently
verified before this candidate. Local validation on the integrated source
passes 187 daemon library cases, including four real QUIC bounds/lifetime
cases, and seven native invitation/retry/recovery cases, with matching rebuilt
programs, cargo-deny, zero-warning all-target Clippy and repository checks.
Fresh v0.6.33 tag gates and independent archive/install checks remain required.
The wider security review and actual resource/elapsed gates stay separate.
