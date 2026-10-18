# Initial peer admission fairness

> Mission: keep one authenticated device's incomplete requests from occupying every application slot.

## Problem and change

The existing global limits bound handlers but allow one transport identity
to consume them all before repository and roster admission. A controlled
native QUIC case opens sixteen incomplete headers across two connections
and then a seventeenth. Without the change it fails after 5.224 seconds:
the excess request is not refused and occupies another device's slot.

Each authenticated transport identity now has eight initial connection
slots and sixteen pending annex header slots shared across its connections.
Excess work is refused without spawning a waiting task. Repository and roster
admission releases the header slot and the connection's initial slot. The
existing global 64-connection and 128-stream limits remain for the whole
handler, and accepted long copies and their metadata exchanges retain
their existing behavior. Control protocols retain initial connection slots
through their bounded exchange. Cancellation and errors release permits.
Weak peer entries are pruned on lookup, while live handlers and headers keep
their shared budget alive. [ADR 0084](../adr/0084-bound-incoming-peer-application-work.md)
remains Proposed.

## Evidence and limits

All eight focused checks pass: seven native QUIC cases plus identity-map
lifecycle checks through 1000 retired identities. The new header case passes
in 0.237 seconds. The initial-connection case checks refusal, another device's
admission and capacity recovery after closure. A known roster member reaches
admission over nine simultaneous connections, above the initial cap; this
fixture deliberately has no listener and does not claim nine live transfers.
Existing global limits, shared streams and idle-versus-active cases pass.

After integrating published v0.6.39, all 198 daemon library cases and eight
native join, join-resume, recovery-sheet and refounding cases pass. All-target
workspace Clippy reports zero warnings; repository checks precede submission.
The causal red log and all green logs are retained separately. No CI retry
is used to obtain this evidence.

This is initial admission isolation by transport identity. Already-admitted
members, multiple identities and transport-owned resources can still exhaust
global capacity. It does not complete the broader M7 security review, resource
qualification, real elapsed nights or the actual 72-hour soak.
