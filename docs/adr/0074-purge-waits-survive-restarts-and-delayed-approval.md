# 0074. Purge waits survive restarts and delayed approval

- **Status:** Proposed, 16 October 2026, for the M5 implementation.

## Context

A member may request erasure long before an admin accepts the suggestion.
An unchanged requested deadline can then precede the acceptance time plus
retention_days, making that approval violate P4. Ordinary suggestion
adoption requires exactly the proposed bytes, so a deadline correction
needs a narrow, explicit exception.

A persisted wall-clock receipt timestamp cannot establish an elapsed
waiting period after a clock jump. A fresh monotonic timer on every restart
is safe but discards all progress. Status refs are replaced regularly;
forgetting a seal when a newer report omits it would also make completed
waits depend on report arrival order.

## Decision

- An approval may extend only a purge request's `retention_end`. Its exact
  requester, base, paths, keys and explicitly named proposals must match the
  signed, fully judged original request. The accepting commit must satisfy
  the original accepted parents' retention period, even if today's setting
  is shorter. No other metadata receives this adoption exception.
- Each decision's identity includes its accepted introduction and metadata
  path. Cancelling and introducing identical bytes again creates a new
  receipt and waiting period.
- A custodian accumulates monotonic uptime since local receipt in an atomic,
  fsynced, versioned ledger. Ordinary checkpoints occur at most once per
  minute; receipt, cancellation and seal boundaries force a checkpoint.
  Downtime and uncheckpointed time do not count. An unknown or torn ledger
  is preserved and prevents new seals. A missing ledger restarts the wait.
- A seal requires both the declared UTC deadline and the receiver's full
  accumulated monotonic waiting period. Only durably checkpointed seals may
  be published. A failed write cannot acknowledge erasure.
- Retain complete signed seal report objects under local witness refs.
  Verify signer, repository, accepted sealer history, decision identity and
  declared deadline before using them. A later report cannot silently
  revoke an immutable seal. Conflicting values prevent release.

## Consequences

A restart or forward wall-clock jump cannot shorten a receiver's wait.
Frequent restarts or long downtime can extend erasure beyond its advertised
UTC deadline; the interface must report waiting custodians honestly.

The evidence proves a lower bound on observed uptime, not elapsed downtime.
Tests may advance an injected monotonic fixture, but must not present that
as an observed month of retention. This decision does not resolve changing
custodian membership or refounded archives; those require separate model
and implementation evidence before M5 can be released.
