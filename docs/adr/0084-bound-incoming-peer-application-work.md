# 0084. Bound incoming peer application work

- **Status:** Proposed, 18 October 2026.

## Context

The M7 joining/signing review finds an application task spawned for every
incoming connection, followed by another for every annex stream. Transport
handshake limits and header deadlines alone do not bound the number of
application handlers. An idle annex connection can also remain in its accept
loop after its streams end.

## Decision

Use nonwaiting owned permits before spawning peer application work: at most
64 connection handlers (including handshakes) and 128 annex stream handlers
across one endpoint. Refuse excess incoming connections or reset excess
streams rather than creating tasks waiting for a slot. Every exit and task
cancellation releases its permit. Authentication and authorization remain
separate and mandatory after capacity admission.

After the authenticated transport identifies a device, allow it at most
eight initial connections and sixteen annex headers awaiting repository
and roster admission, shared across its connections. Refuse excess work
without waiting. Successful roster admission releases that header's slot
and the connection's initial slot; the original global slots still cover
the entire handler. This avoids imposing a per-device transfer cap on
accepted copies and their metadata dependencies. Other application
protocols retain the initial connection slot until their bounded exchange
ends. Keep weak peer entries, pruning expired entries on lookup, so identity
churn cannot grow a permanent map or reset budgets held by live headers.

Bound each accepted handshake by the existing ten-second header budget.
Close an annex connection after ten seconds without a new stream only when
no live stream remains. Keep live transfers and header waits alive across
idle-accept deadlines. Drain completed stream task records before accepting
more work, so a fast completed-stream flood cannot grow that collection.

## Consequences

The limits bound application handler and relay-buffer growth; they are not
rate limits or a proof of resistance to network floods. Initial quotas
isolate incomplete work by authenticated transport identity, not by person.
Multiple identities and already-admitted members can still consume global
capacity; this does not establish general denial-of-service resistance.
Transport-owned pre-admission memory and CPU still require assessment. A
busy endpoint may refuse a legitimate peer, whose existing bounded retry
policy applies. The constants can be reconsidered with measured workloads;
they do not grant membership, relax proof checks or change a replicated
format. Native QUIC cases check refusal, capacity recovery, shared stream
limits and idle-versus-active lifetimes. Further native cases exercise
shared per-device headers, connection refusal/recovery and admission above
the initial connection cap; a lifecycle case exercises 1000 retired identities.
