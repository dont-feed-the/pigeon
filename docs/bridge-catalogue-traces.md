# Checking bridge catalogue reads

> Scope: one bounded part of the Nextcloud protocol, observed in the bridge.

For client identity, write/response and shared-storage contracts, see
[checking a bridge alongside native peers](bridge-system-verification.md).

A Nextcloud client discovers native edits through a DAV listing and directory
ETags. Those listings depend on the bridge reading a complete native catalogue.
A catalogue can exceed an RPC response, and another peer may save between pages.
Publishing half an old revision and half a new one could hide files or report
an ETag for content that the client never saw. After a successful upload, the
refresh must also stay on the daemon's acknowledged revision.

`BridgeCatalogue.tla` models that read transaction. Its finite configurations
explore two paths, two revisions, malformed continuations, reordered pages,
RPC failures and up to three attempts. The weakened revision guard must produce
a counterexample. A separate pinned configuration forbids changing the revision
of a post-save read. The normal fast model jobs run all three configurations.

The implementation records actual RPC cursors and replies, each accumulated
continuation buffer, retries, failures, and the complete catalogue it returns.
`BridgeCatalogueTrace.tla` requires those observations to follow the same
transition relation. It supplies no missing page, retry or completion step.
Each read has its own log, so concurrent clients require no global trace lock.
The checker renames revisions and ETags by equality and paths by UTF-8 byte
order; it preserves file sizes, divergence flags, page boundaries and cursors.

Run the regression from a checkout with Rust and Java:

```sh
python3 scripts/check-bridge-catalogue-traces
```

This runs the real bridge reader against Unix-socket RPC fixtures. It requires
all eleven reads: a 4,000-file catalogue exceeding one RPC message, a revision
change partway through, retry exhaustion, a failing pinned read, two concurrent
pinned reads, and six malformed replies. TLC must accept every complete stream.
Mutations of those observations must be rejected by TLC, including missing
files, changed checksum metadata, a changed published revision, a skipped
cursor, a discarded first page, mixed revisions, stale retry buffers and an
unpinned post-save read. Empty and truncated logs fail before TLC. CI runs this
alongside the real-client integration workflow.

To record a running bridge, build with `--features pigeon-bridge/tla-trace`,
set `PIGEON_TRACE=1`, and optionally set `PIGEON_TRACE_DIR` to a private output
directory. Without that directory, logs go beside each peer socket in a
`*.bridge-traces` directory. Then replay them with:

```sh
python3 scripts/check-bridge-catalogue-traces /path/to/trace-directory
```

Tracing is opt-in, with no serialization or filesystem access in normal builds.
Directories are created with mode 0700 and files with mode 0600. Logs include
file names, sizes, ETags and revisions; treat them as private metadata. They
contain no credentials or file bodies. Trace writes are diagnostic and do not
change RPC behavior; missing expected logs and incomplete writes fail the
regression. Stop the bridge before replaying its production directory so an
in-flight read is not mistaken for a truncated log.

The native daemon supplies the contract that a revision identifies one complete,
immutable catalogue and a stale cursor returns 412. The model checks how the
bridge consumes those observations. It does not prove that native snapshots
contain every file, hash collision resistance, Unicode name portability, the
HTTP/XML parser, Login Flow v2, upload assembly, revocation, native durability,
or liveness. Native filesystem and roster traces retain their separate checks
in [daemon-traces.md](daemon-traces.md). The bridge's real Nextcloud-client
smoke test remains necessary for wire compatibility, including chunk uploads.
