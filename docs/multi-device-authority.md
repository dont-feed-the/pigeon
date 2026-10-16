# Authority across a person's native peers and bridge clients

> Mission: explain what the same-person authority model checks and where its
> abstraction stops.

`models/tla/MultiDeviceAuthority.tla` complements `Authority.tla` without
changing its assumptions or proof contracts. Its four signing devices are an
owner's native peer, a member's native peer, and two bridge clients belonging
to that member. A proposal from the first client changes the owner's folder.
All devices initially know that proposal, and otherwise exchange complete
causal histories of accepted decision and removal events.

The important distinction is between a **person's permission** and a
**device's signature**. Either of the member's other devices can withdraw
their person's proposal. They cannot accept or reject changes to the owner's
folder. Removing one client suppresses that device's unseen actions, without
removing the sibling's authority or actions. A decision already seen by its
signer's removal keeps counting. Concurrent acceptance, rejection and
withdrawal use the existing accepted-before-rejected-before-withdrawn
precedence.

The policy corresponds to `pigeon-core/src/authority.rs`'s signer-to-person
lookup and `decision`, and `pigeon-core/src/roster.rs`'s `may_sign` for device
removal. The causal cutoff follows `Authority.tla`'s `Unseen`/`Counting` rules:
removals survive, and a device's unseen actions stop counting when histories
join. Client transport is deliberately absent from the permission predicate;
it cannot grant a client's request the bridge control peer's permission.

## Checks and counterexamples

The `good` configuration checks all interleavings of two decision/removal
events and history exchanges; `full` increases the bound to three events,
restricted to at most one removal, targeting the first bridge client.
Both check valid causal histories, correctly authorized actions, availability
of sibling withdrawal while pending, individual removal, retention of seen
events, and suppression of unseen events from the removed key.

Four weakening configurations must produce counterexamples:

| Configuration | Regression | Expected violated invariant |
| --- | --- | --- |
| `device` | Compare the acting device with the proposal signer instead of their persons | `SiblingCanWithdraw` |
| `members` | Let any member accept/reject the owner's suggestion | `ActionsAuthorised` |
| `person` | Remove every device of the target's person | `RemovalOnlyTarget` |
| `unseen` | Keep counting a removed signer's unseen decision | `RemovedUnseenSilent` |

Three reachability configurations intentionally negate required scenarios.
Their counterexamples demonstrate that the good model actually reaches
competing decisions on disconnected devices (`concurrent`), a sibling
withdrawing after observing individual removal (`sibling`), and suppression
of a removed client's stale withdrawal while retaining the surviving
client's later withdrawal (`isolation`). They are coverage witnesses, not
failures in the intended policy.

The separate `live` configuration checks eventual agreement of the complete
observed histories of all surviving devices. Its assumptions are finite event
creation and weakly fair history exchange between every pair of survivors;
removed devices need not reconnect or agree. This checks the abstract delivery
model, rather than establishing that the running bridge satisfies those
connectivity and scheduling assumptions.

Run registered configurations with `scripts/run-tlc MultiDeviceAuthority_good`
or another configuration name. The two-event good and temporal graphs each
have 19,717 distinct states. The focused three-event graph has 137,742
distinct states (nine seconds locally with two TLC workers). A passing
bounded check establishes only the chosen finite world; these checks are not
TLAPS proofs for arbitrary networks.

The signed-Git regression `pigeond/tests/decision_author.rs` checks a concrete
part of this boundary: removing the proposal author's client key preserves
the pending proposal's person attribution, and a surviving sibling's
withdrawal is accepted. Withdrawals signed by the removed key or an unrelated
member are refused, as is a member's rejection requiring the owner's
permission. Pending suggestion attribution reads the verified proposal's
accepted base roster, matching the decision judge, while authorization of
the acting device still uses its current accepted parent's roster.

## Boundaries

This is a model of authority and causal decision observations. It assumes
the proposal was fetched, signatures identify the named devices, accepted
events were validated against their observed rosters, and exchanges preserve
complete causal histories. It does not implement the full native commit
judge, catalogue/content merge, client authentication, bridge request routing,
device provisioning, additions, recovery keys, crash recovery or HTTP.
Changing a model's request signer cannot establish that
the running bridge selected that signer; runtime request/daemon traces must
make that connection separately.

The fixed folder owner and initially fetched proposal also exclude ownership
transfer, changing decider sets, proposal revisions and adoption conflicts.
Those remain covered in part by the existing native models, rather than
being proved by this focused extension. In particular, passing this model
does not establish a refinement of the complete distributed implementation.
