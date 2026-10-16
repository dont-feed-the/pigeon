# Checking a bridge alongside native peers

> Mission: describe the deployment properties checked by bridge models,
> implementation observations and real multi-peer regressions.

A bridge on a custodian runs several distinct working peers. Two installations
belonging to the same person have different keys and annex UUIDs, but the same
person's permissions. Their shared immutable bytes do not create independent
backup copies. Native peers exchange history with these managed peers while
Nextcloud clients issue requests and poll their own views.

## Coverage

The checks deliberately use several small contracts. Their composition is
exercised by real peers; there is no universal refinement proof of the entire
HTTP, daemon, Git, filesystem and network implementation.

| Contract | Evidence | Assumptions and limits |
| --- | --- | --- |
| A complete catalogue comes from one visible revision; a post-save refresh stays pinned | `BridgeCatalogue`, RPC observations and malformed-page mutants | Trusts the daemon's complete revision-labelled catalogue; see [catalogue traces](bridge-catalogue-traces.md) |
| DAV saves submit the observed preconditions, consume an actual Apply outcome, and use a pinned refresh when returning a write ETag | `BridgeSync` request observations; `BridgeSyncSystem` explores the native/DAV interface | An Apply reply is a daemon observation, not independently observed disk durability or a proof of HTTP delivery. Native reconciliation is an abstract input |
| Per-client identities persist and selected sockets respect local revocation and account ownership | `BridgeLifecycle`, persisted-state and raw daemon identity observations | Covers checked socket selection and selected UI dispatch. Provisioning/network internals and completion of roster removal remain outside the model |
| Authority follows persons; signatures and individual removal follow devices | `MultiDeviceAuthority`, weakening configurations and scenario witnesses | One already-fetched proposal and fixed folder authority; [full scope](multi-device-authority.md) |
| Mutable sources never alias immutable retained bytes; acknowledgement requires completed pool sync; aliases do not add custody domains | `BridgeSharedStorage`, interrupted-sync and private-alias regressions | One content key. Rename, file sync and directory sync are abstract filesystem contracts; no physical media-failure or power-loss proof |
| Sparse accepted conflicts do not become missing pending files; recovery prunes only a proven phantom | `BridgeSparseConflict`, signed incoming-install and recovery guard regressions | Abstract entry identities and filesystem observations; [scope and counterexamples](bridge-sparse-conflict.md) |

The native authority, catalogue, capture/install, retention and custody checks
remain necessary. These additions do not replace their proofs, TLC scenarios
or implementation traces.

## Failure scenarios

The real custodian shared-storage regression runs a control peer, a second
regular peer and two separately authorized Nextcloud clients. Both clients
write against the same known ETag concurrently. Because they have separate
daemons and asynchronous history exchange, both requests can capture a save.
The test checks retention of every acknowledged body and rejection of a stale
conditional retry, without prescribing a merge winner.

Fixture setup also reconciles an uncertain create: native gossip can race the
pinned refresh **after** Apply has committed, producing HTTP 412. The setup
reads back and requires the exact intended bytes; it retries a conditional
`If-None-Match: *` create only while the path is absent, within the existing
120-second bound. An unexpected body or other response fails immediately.
The concurrent writes themselves are never retried, and their retention and
stale-validator assertions are unchanged.

The second regular peer then saves while offline, while a client saves another
path through the bridge. Reconnection must deliver both changes to both
clients and the regular peer. Existing checks also verify shared inodes,
custody counts, provider-reference removal and identities across restart.

This scenario exposed an interaction between native conflict naming and sparse
client trees. An already accepted, hidden version could acquire a conflict name
and be mistaken for a live pending file at the original path. Compaction then
failed because that file was absent, blocking subsequent synchronization.
Incoming sparse updates now follow the accepted catalogue and conflict copy.
Existing phantom entries are repaired only when they have no observed local
stat, the original is absent and hidden, the complete indexed entry survives
at an accepted conflict name, and its immutable pool bytes verify. Real pending
files and uncertain cases are preserved. A signed-history regression and seven
negative recovery cases check these boundaries.

Socket-level DAV regressions control boundaries that are difficult to schedule
reliably with real networking: an Applied save followed by a failed pinned
refresh, a lost Apply reply, later reconciliation, stale retries and current
retries. An HTTP error after Apply does **not** imply rollback. The client must
resynchronize to discover the outcome; these checks do not promise exactly-once
requests or global linearizability across offline peers.

Lifecycle regressions persist a revoke while the control peer is unavailable,
restart the bridge, and verify the sibling's identity and access. They also
poll real legacy portal handlers until blocked on their client lock, persist
revocation under that lock and resume them. A queued handler must reload the
revocation state before selecting a peer. Requests already operating before
revocation are allowed to finish under the existing operation lock.

Roster removal sends the saved device key through `RemoveDeviceId`, rather
than looking up a computer name and resolving it again in a later RPC. The
lifecycle trace checks that attempted target, and a native signed-roster test
checks the actual removal and idempotent retry. A computer name that looks like
another device's key cannot redirect the ID operation. Native roster names are
unique; bridge display labels and mutable names do not define peer identity.

A signed-history regression removes a proposal's signing client while leaving
another device of its person active. The proposal must retain its historical
author attribution, the sibling may withdraw it, and neither the removed key
nor an unrelated person gains authority. The acting device is still judged
against the current roster.

The pool regression injects a directory-sync failure after publication has
already renamed its object. A retry that finds that object must still complete
directory sync before reporting success. Separate checks remove the provider's
and clients' private references while retaining a pool pin and an open reader.
The pool remains append-only; this does not introduce coordinated garbage
collection.

## Running checks

Build matching programs with the bridge observation feature and daemon lab
support. Keep the matching git-annex available as described in
[development](development.md):

```sh
cargo build --locked -p pigeon-bridge -p pigeond -p pigeon -p git-annex-p2p-iroh \
  --features pigeond/lab,pigeon-bridge/tla-trace
scripts/run-tlc
python3 scripts/check-bridge-catalogue-traces
python3 scripts/check-bridge-sync-traces
python3 scripts/check-bridge-lifecycle-traces
cargo test --locked -p pigeon-bridge --features tla-trace --lib
cargo test --locked -p pigeond --lib shared_pool
cargo test --locked -p pigeond --lib shared_overlay
cargo test --locked -p pigeond --test install sparse_accepted_conflict
cargo test --locked -p pigeond --test device_revocation
```

Record the native multi-peer scenario into a private, empty directory:

```sh
mkdir -m 700 /tmp/pigeon-bridge-observations
PIGEON_TRACE=1 PIGEON_TRACE_DIR=/tmp/pigeon-bridge-observations \
  python3 scripts/check-nextcloud-shared-storage --provider custodian
python3 scripts/check-bridge-sync-traces /tmp/pigeon-bridge-observations
python3 scripts/check-bridge-lifecycle-traces /tmp/pigeon-bridge-observations
```

Each checker without a path runs its required regression and corrupts actual
observations. Incorrect identities, stale authorization, incorrect conditions,
missing transaction steps and mismatched returned metadata must be refused.
The normal model runner also requires deliberate policy weakenings to fail;
separate reachability configurations ensure important scenarios are possible.

The Nextcloud CI workflow runs both controlled regressions and the real
custodian deployment observations. Negative controls are part of the checks,
rather than optional diagnostics.

## Observation boundaries

Tracing is opt-in at build time and requires `PIGEON_TRACE=1` at runtime.
Feature-disabled code performs no trace filesystem access or serialization.
`PIGEON_TRACE_DIR` selects the output directory; otherwise traces stay in
private directories within the bridge's state or beside peer sockets.
Directories and files are created with modes 0700 and 0600. Logs contain
client/device identities, local socket paths, filenames, sizes and validators.
They omit app passwords, password hashes, cookies and file bodies. Treat the
logs as private metadata.

DAV transactions use separate logs and must end in an observed constructed
response. Stop the bridge before replaying its directory: an in-flight request
is an incomplete transaction. Responses record what the server constructed,
not whether the client received it. Lifecycle streams append across restarts
and check observed prefixes; an idle prefix does not establish that an expected
operation occurred. Regression coverage checks provide that requirement.

Diagnostic writes cannot change request behavior. A trace I/O failure may stop
recording; successful replay alone does not prove complete observation of a
production run. The per-request and lifecycle streams do not supply a global
distributed event order or a cryptographic linkage to every signed commit.

## Remaining boundaries

The bridge interface model assumes native authority and merge results. It does
not prove arbitrary partitions converge through the full native/DAV pipeline.
The focused authority model checks fair history exchange among surviving
devices under its finite bounds, rather than Nextcloud polling liveness.
The integration scenarios provide finite end-to-end evidence.

HTTP/XML parsing, every desktop/mobile client's retry algorithm, arbitrary
chunk-order and crash combinations, all directory ETag/checksum transformations,
cross-filesystem behavior and arbitrary partial I/O failures still need their
separate tests. The existing deep ETag, checksum, range and chunk regressions
remain in place. Empty directories and source times remain bridge metadata,
rather than replicated native catalogue entries.

The DAV model checks native PUT's request path and submitted changes. It does
not formalize MOVE's Destination header or its complete subtree transformation.
The Rust regressions exercise HTTP conditional headers; the model consumes the
resulting native Apply conditions rather than independently parsing If-Match.

Larger bounds and TLAPS proofs can strengthen these contracts, but cannot
establish behavior omitted from their state and actions. Future extensions
should start from a concrete scenario, record its actual boundaries, and include
a negative control that demonstrates the new check detects the intended error.
