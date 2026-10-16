# Nextcloud bridge

> Mission: run the bridge, explain its client identities and review workflow,
> and record the compatibility boundary of this initial implementation.

For a fresh server with no existing group, start with
[Quick start: your first custodian and Nextcloud bridge](nextcloud-quick-start.md).

`pigeon-bridge` exposes a Nextcloud-compatible file endpoint and a browser
portal. Every successful Nextcloud login-flow authorization creates a separate
Pigeon device of the signed-in person: its own signing key, iroh endpoint,
annex UUID, repository, sparse working tree, and daemon process. Other Pigeon
devices see these clients in their normal roster and machines list. Immutable
file bodies are shared; adding a client does not create another complete file
copy. Only that client's pending changes need a working-tree representation.

The bridge holds those keys and runs those peers on the bridge machine. The
Nextcloud apps communicate with the bridge over HTTP(S). They do not run iroh
or sign Git history themselves. Authorize every installation independently;
copying an app password copies access to the same peer. All these devices are
working peers, never custody copies or independent backup failure domains.

## Build and configure

Build the bridge and the matching Pigeon programs together:

```sh
cargo build --release -p pigeon-bridge -p pigeond -p pigeon -p git-annex-p2p-iroh
```

Keep `pigeond` and `git-annex-p2p-iroh` beside each other and install Pigeon's
matching git-annex as described in [development.md](development.md). The
bridge starts each client daemon with `--home` and a separate private socket;
it does not change the service user's home directory.

Active peers are restored at bridge startup. The bridge exchanges advisory
direct addresses among its co-hosted peers and control peers in the same group,
so they can communicate without a relay. Every connection still checks the
peer key and signed roster; an address hint grants no membership or authority.

Each bridge user needs an existing personal Pigeon **control peer** in the
group they want to expose. That peer can be an existing daemon, or a dedicated
one created with `pigeond --home /srv/pigeon/alice` and joined to the group
using `PIGEON_SOCKET=/srv/pigeon/alice/run/pigeond.sock pigeon join …`.
The control peer must belong to the actual person's roster identity. A bridge
login name is just an account name; it does not grant Pigeon authority.

The bridge uses that control peer to invite each new client as another device
of the same person, and to accept or decline group suggestions. The control
peer must be reachable during authorization and review. Already authorized
clients continue to use their own daemons independently.

By default that control peer is also the content provider. To reuse an existing
custodian, set `storage_socket` in the user's table to its private local socket.
It must serve the same group, run the matching Pigeon version, and be accessible
to the same operating-system user. Invitation and review still use the personal
control peer. The source may have other groups: requests name the exact group.

Copy [bridge.example.toml](../packaging/bridge.example.toml) into a private
configuration directory. Generate a portal password hash by piping a password
file into the executable:

```sh
pigeon-bridge hash-password < /path/to/private-password-file
pigeon-bridge --config /path/to/bridge.toml
```

Set an absolute `state_dir`, the absolute matching `pigeond` path, and one
`control_socket` and Argon2id `password_hash` per portal account. One default
Pigeon group is exposed per account. Run the bridge as a user that can access
these sockets. Pigeon's socket admission requires the same operating-system
user; use isolated daemon homes for the different persons.

`state_dir` is private (0700), and contains persistent client records and
daemon homes. One bridge instance holds an exclusive lock on it. Back up this
directory with the service stopped; it includes device keys, client credential
hashes, file IDs, and timestamps. Unknown or malformed state is refused instead
of being replaced with fresh identities. The bridge does not delete revoked
clients' retained files. The default content pool is inside `state_dir`; if
`content_dir` points elsewhere, include it in the backup too.

Loopback HTTP is supported for development. For access from other machines,
set `public_url` to the exact HTTPS origin and terminate TLS at a reverse
proxy. Preserve the original `Host`, allow DAV methods, and configure request
size and timeouts for large file transfers. Forwarded headers do not override
the configured public origin. The bridge validates Host and browser Origin,
uses HttpOnly session cookies with CSRF tokens, and adds Secure cookies on
HTTPS origins. Do not expose the per-daemon loopback web servers or sockets.
When using systemd, keep `KillMode=control-group` so an aborted service's
supervised processes are also stopped.

## Shared content and existing nodes

All managed clients use `content_dir`, defaulting to `state_dir/content`. It is
a private (0700), group-scoped pool keyed by SHA-256 content, independent of
filenames and client identities. Each client's annex retains hard links into
that pool. Accepted files stay hidden in its sparse index; the working tree
contains its pending suggestions or conflicts. Identical versions, including
files with different extensions, share bytes without sharing mutable state.

To reuse a custodian, set the optional storage provider separately from the
personal control peer:

```toml
[users.alice]
password_hash = "$argon2id$replace-with-the-generated-hash"
control_socket = "/srv/pigeon/alice/run/pigeond.sock"
storage_socket = "/srv/pigeon/storage/run/pigeond.sock"
```

The bridge registers its pool with that existing node. Custodian objects can be
pinned directly, and future admitted or received versions reuse the same pool.
Normal admission, reserve checks, hash verification and durable custody
acknowledgements still apply. Client aliases are untrusted laptop devices and
never add independent custody copies, even when their bytes are shared with a
custodian. A single custodian plus several clients cannot satisfy K = 2.

A registered laptop provider also shares its immutable annex objects. Its
working files remain writable and separate from the pool: registration disables
re-thinning into writable hard links. Already thinned, writable objects are
detached using a copy-on-write snapshot when supported, otherwise one retained
copy. Subsequent in-place laptop saves cannot change any client's retained
version. Thus a laptop may need an immutable snapshot in addition to its live
file; adding clients does not add complete file copies.

The pool and managed client repositories must be on the same filesystem. For
direct reuse, place the provider's annex on that filesystem too. A provider on
another filesystem cannot register a shared pool; normal Pigeon retrieval can
still populate one shared bridge copy per version. Existing cached versions
remain available when the provider is offline. Upload and capture may require
temporary private copies. Downloads pin immutable bytes without copying on the
same filesystem. Executable pending files keep a private representation so
per-path execute permissions cannot change a shared inode.

For an existing bridge, stop the service and its supervised daemons, rebuild
the matching programs, then restart. Managed peer configurations are upgraded
in place, preserving keys, annex UUIDs, authorization records and DAV metadata.
Migration captures outstanding saves, shares retained objects, and journals the
hiding of accepted files before removing their working copies. Recovery finishes
that journal before capture; a hidden file never becomes an accidental deletion.

Provider registration is stored in each source repository's
`pigeon/shared-provider.json` under its Git directory. Keep that metadata with
the source backup. Registered and managed repositories require matching new
binaries; their immutable references must be detached before removing a laptop
provider registration or reverting to an older implementation.

## Connect and review

1. In a Nextcloud app, add an account using the configured `public_url`.
2. The app opens the bridge's login flow in the browser. Sign in to the portal,
   choose a client name, and grant access.
3. The bridge provisions that client's Pigeon peer and issues a unique random
   app password. The app receives it by polling, once. The persistent bridge
   database stores only its hash.
4. Repeat for each desktop or phone, including several clients of one person.

The portal at `/` lists the person's clients and their public peer IDs. A
client page shows Pigeon protection status, downloadable files, and that
client's divergences. `/suggestions` lists group suggestions; owners or stewards
can accept or decline, and admins can supply a reason when an override is
required. Pigeon checks the authority of every decision. A client page offers
*Take the group's version*, *Keep mine as a copy*, and *Suggest again*.

The bridge follows Pigeon's personal-folder semantics. A change that requires
approval is captured as a signed suggestion. That client's DAV view overlays
its captured version on the accepted catalogue, so an upload reads back
consistently while awaiting review. Other clients keep the accepted version.
A declined suggestion remains the submitting peer's file until the person
resolves it. The portal exposes this state; ordinary Nextcloud clients do not
understand Pigeon approval or protection status. Successful HTTP upload means
captured and retained locally, with replication and custody progressing
asynchronously.

*Revoke this client* immediately disables its app credential, stops its managed
daemon, then asks the control peer to remove its exact device from the roster.
If the control peer is unavailable, access stays disabled and the portal reports
that roster removal needs attention. Other clients retain their identities and
credentials. A fresh authorization creates a new peer.

## File protocol and persistence

Implemented: login flow v2, status and capabilities, OCS user lookup and app
password deletion; DAV OPTIONS, PROPFIND (Depth 0/1), GET/HEAD, PUT, MKCOL,
DELETE, MOVE and COPY; Nextcloud chunk uploads; conditional writes, checksums,
byte ranges, modification-time acknowledgement, file IDs, and recursive
directory ETags. Unsupported properties get a 404 propstat. Unsupported
PROPPATCH properties are reported as such rather than silently applied.

Complete HTTP request bodies and chunks are staged outside shared trees. Only
finalized files become peer writes. The daemon serializes a batch with capture
and install, checks the visible revision, journals the explicit changes,
captures accepted changes or suggestions with its own key, and syncs content
before acknowledging success. An interrupted batch is replayed before normal
loops resume; a subsequent conflicting local save blocks recovery rather than
being overwritten. Nextcloud's If-Match protects versions already known at the
gateway; concurrent changes arriving later from other Pigeon devices still use
Pigeon's normal merge and conflict rules.

Private DAV metadata preserves IDs and upload times across bridge restarts.
MOVE preserves IDs, and unique incoming content-preserving renames can retain
them. Directory ETags cover descendants, so clients polling a root discover
deep changes. Each authenticated client has an isolated chunk namespace with a
pending-byte budget and an eight-upload limit; abandoned uploads expire after
24 hours when that client's upload namespace is next accessed.

The bridge reads catalogues in pages of at most 1,000 files and 256 KiB per
socket reply, including the continuation cursor. A cursor contains the visible
revision and last path. If paths, versions or divergence status change between
pages, the daemon refuses the continuation and the bridge discards its partial
listing. Ordinary reads retry up to three listings, then return a temporary
unavailability error. Partial listings never update persistent DAV metadata.
Write acknowledgements contain a revision and suggestion count, rather than a
complete catalogue, so saving a file also works with a large tree. Post-save
refreshes require that exact revision: if another change arrives first, the
client must synchronize again rather than receive an ETag for different bytes.
The save has already been captured in that case.

Pagination removes the catalogue's single-message size limit. The bridge still
builds an in-memory index and keeps persistent metadata for the complete tree;
the daemon computes its visible catalogue for each page. Directory ETags hash
files into their ancestors without scanning the entire tree per directory.

## Current limits

- This is an initial file-sync bridge, not a full Nextcloud server. It does not
  advertise shares, calendars, contacts, activity, previews, trash/version APIs,
  Nextcloud E2EE, or DAV locking. Legacy browser login flow v1 is not implemented.
- Each client still has its own daemon and complete Git catalogue. File bodies
  are shared, but process and metadata costs grow with the number of clients.
  Clients are limited per portal account in configuration.
- The content pool retains versions conservatively. Revocation and ordinary
  annex drops do not automatically reclaim pool content. Coordinated garbage
  collection across peer histories and pending changes is not implemented.
- Empty directories and source mtimes are bridge metadata; Pigeon does not
  replicate them. Empty directories created on one client are not created on
  another peer until files give the directory a Pigeon representation. Directory
  metadata should be included when backing up the bridge.
- A batch is limited to 4096 file changes and must fit the local socket's 1 MiB
  request limit. Unavailable catalogues fail the request; continuously changing
  catalogues may need a later retry. There is no automatic gateway failover.
- A batch deleting more than 100 existing files, or deleting paths Pigeon is
  holding for review, is refused with HTTP 409 before changing the working
  tree. Split large deletions into smaller requests and review held deletions
  in the affected peer. The bridge portal does not yet offer that review.
- Local filesystem access can bypass an HTTP precondition. Treat managed peer
  working trees as private implementation state and use the clients or portal.
  Retained conflicting saves are surfaced by Pigeon rather than discarded.
- Desktop/mobile application releases need separate interoperability checks;
  capability responses advertise the implemented Nextcloud 35 API subset, not
  compatibility with every app or every server feature.

## Verification

```sh
cargo test -p pigeon-bridge --lib
cargo test -p pigeon-git --test backpressure
cargo clippy -p pigeon-bridge -p pigeond -p pigeon-git -p pigeon-sys --all-targets -- -D warnings
cargo test -p pigeond --features lab --test bridge
cargo test -p pigeond --features lab --test shared
scripts/check-nextcloud-bridge
scripts/check-nextcloud-shared-storage
scripts/check-nextcloud-shared-storage --provider laptop
```

The integration script starts real loopback Pigeon control peers and three
separately authorized clients. It checks their roster identities and annex
UUIDs, signed replication, conditional uploads, ranges, rename, a suggestion
accepted through the portal, a declined suggestion resolved by its author,
user isolation, chunk staging, deep ETag propagation, restart persistence and
individual revocation. Its fixtures and daemon processes are temporary.
It also checks shared object inodes, sparse accepted files, private suggestion
overlays, and in-place saves on an existing laptop provider.

The shared-storage script uses an existing custodian as the configured provider.
It verifies that provider, pool and two independently signed client peers share
one inode, including new uploaded versions, and that client aliases cannot meet
K = 2 with only one custodian. Cached versions survive provider removal and
bridge restart. Its laptop mode also verifies that normal in-place source saves
cannot mutate retained versions. The native shared test migrates an existing
peer, including a previously writable thinned inode, and forces
crashes after setting the hidden flag and after removing the working file;
recovery preserves catalogue versions and identities, and hidden paths remain
writable and deletable through the bridge API.

The daemon bridge tests page a real 1,200-file catalogue exceeding 1 MiB, check
that every page and write reply stays bounded, and reject a cursor after a save.
They also abort a peer after an explicit batch has reached disk
but before capture. Restart replays and captures the batch; a stale conditional
save is refused while an unrelated edit can be safely rebased.
Socket tests cover longer Unicode paths, empty catalogues, concurrent listing
changes, retry exhaustion, and invalid continuations.
The Git batch regression exercises simultaneous input and output larger than
pipe buffers; catalogue reads must keep both directions moving.

Optionally set `NEXTCLOUDCMD=/path/to/nextcloudcmd` to exercise a real Nextcloud
command client against another independently authorized peer. Its app password
is supplied on stdin, not through command arguments. `PIGEON_BIN` selects the
Pigeon binary directory; the default is `target/debug`. Keep git-annex on PATH
when running these checks.

Catalogue consistency can also be checked from real bridge observations; see
[Checking bridge catalogue reads](bridge-catalogue-traces.md).
