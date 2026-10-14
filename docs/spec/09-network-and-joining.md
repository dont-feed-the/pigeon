# 9. Network and joining

> Mission: specify how devices find, authenticate and talk to each other: the
> device key, the iroh transport helper, the `pigeond` endpoint and its local
> socket, append-only serving on custodians, relays, invitations, removing a
> computer, and recovering a group. Everything that crosses the network
> crosses it here.

## 9.1 The device key (ADR 0007)

- Each device has one **ed25519 key**, generated when it creates or joins a
  group. Its public key, written as 64 lowercase hex digits, is the **device
  ID** everywhere: iroh node ID, commit signer, roster key.
- One `pigeond` serves every group of the user on that device with the same key
  (ADR 0011: one node ID online per device). A device that is in two groups is
  therefore recognisable as the same device by both.
- The key is written once as an OpenSSH private key, mode 0600, in the
  configuration directory (`<config>/device-key`, [11](11-local-state.md)),
  with the `ssh-key` crate. git signs with it through `gpg.format=ssh` and
  `user.signingKey` (`ssh-keygen -Y sign -n git`); iroh loads the same key as its
  secret key.
- pigeon verifies signatures itself (SSHSIG, namespace `git`) against the
  roster at the commit's parents ([04 §4.4](04-roster-and-authority.md)).
- The key is not encrypted at rest: `pigeond` must start unattended. A stolen
  unlocked laptop can sign until it is removed (§9.7); that is outside the
  guarantees (keys are not stolen, [02 §2.2](02-guarantees.md)).

## 9.2 The transport helper `git-annex-p2p-iroh` (ADR 0006)

git-annex reaches peers through its generic P2P transport: the remote URL
`p2p-annex::iroh+<device-id>` ([03 §3.6](03-repository-and-roles.md)) makes it
run the helper `git-annex-p2p-iroh`, a small Rust binary installed next to
`pigeon`. The helper is a thin client of `pigeond` (ADR 0011) and has three
modes, fixed by git-annex:

| Called as | git-annex expects | The helper does |
|---|---|---|
| `git-annex-p2p-iroh address` | one or more lines: the addresses peers use for this repository | asks `pigeond` for this device's ID and prints it |
| `git-annex-p2p-iroh <address>` | a byte stream to that peer on stdin/stdout; exit non-zero if it cannot connect, zero when the peer closes | asks `pigeond` for a stream to `<address>` for this repository, then relays bytes |
| `git-annex-p2p-iroh <address> <socket>` | incoming peers served on git-annex's unix socket `<socket>` | registers `<socket>` with `pigeond` for this repository and stays until `pigeond` or git-annex ends it |

- `pigeond` enables listening once per repository with `git annex p2p --enable
  iroh`, then runs `git annex remotedaemon --foreground` as a child (ADR 0011).
- The helper identifies its repository from its environment (`GIT_DIR`, else
  its working directory) and sends the group ID with each request
  **[no ADR: routing by group ID]**.
- Without `pigeond`, or for an address that is not a device of the roster, the
  helper exits non-zero at once with a one-line message on stderr; git-annex
  then reports the remote as unavailable.

## 9.3 The endpoint of `pigeond` and the local socket (ADR 0011)

### The iroh endpoint

`pigeond` owns the only iroh endpoint of the device (QUIC, TLS 1.3, hole
punching, relays). It speaks these ALPN protocols **[no ADR: names]**:

| ALPN | Carries | Accepted from |
|---|---|---|
| `pigeon/annex/1` | one git-annex P2P session (git and content) per stream | devices of the roster of that group |
| `pigeon/join/1` | the join exchange (§9.6) | anyone holding a valid invitation |
| `pigeon/telemetry/1` | OTLP from devices of the group to its telemetry device (ADR 0025) | devices of the roster, on the telemetry device only |
| `pigeon/debug/1` | `pigeon debug bundle` requests ([12](12-observability.md)) | devices of the roster |

- **Versions** (ADR 0048). A device offers every version of each ALPN it
  supports. A peer with none in common is refused with a reason, shown on both
  sides as `peer-too-old` or `peer-too-new` with the release to install. The
  stream header (§9.8) and invitations (`pgn1-`, §9.6) are versioned the
  same way.
- **Admission.** For `pigeon/annex/1`, `pigeon/telemetry/1` and `pigeon/debug/1`,
  `pigeond` accepts a connection only if the peer's authenticated node ID is a
  device of the roster of the current `main` of the group named in the stream
  header (§9.8). Anything else is closed with a reason, logged at `warn`, and
  never reaches git-annex (ADR 0006).
- **Reuse.** Connections to a peer are reused for all streams and groups;
  each git-annex session is one bidirectional QUIC stream.
- A device removed from the roster is disconnected when the removal is
  accepted (§9.7).

### The local socket

The CLI, the web server and the helper talk to `pigeond` over one socket,
readable by the user only (mode 0600): `$XDG_RUNTIME_DIR/pigeon/pigeond.sock` on
Linux, the per-user temporary directory (`confstr(_CS_DARWIN_USER_TEMP_DIR)`)
on macOS. Messages are newline-delimited JSON objects, one request and one or
more responses each, matched by an `id` field. The request types are those of
the internal API ([10 §10.1](10-interfaces.md)); the helper uses three more:
`Address`, `Connect {group, device}`, `Listen {group, socket}`. After a
`Connect` or `Listen` reply, the connection carries raw bytes only.

`pigeond` refuses a second instance (a lock file next to the socket), and a
client checks the peer credentials of the socket (`SO_PEERCRED`,
`getpeereid`) are the same user.

## 9.4 Serving peers: pull only, append-only on custodians (ADRs 0014, 0043)

git-annex cannot serve a P2P network append-only. `remotedaemon` serves it
read-write whatever the environment (E13; `serveAuthed ServeReadWrite` in
its source). `git-annex-shell p2pstdio` is append-only but relays no git. And
by default any peer can push refs over P2P, a custodian's `main` included
(E12). So `pigeond`, which relays every incoming stream (§9.3), passes each
session to the device's `remotedaemon` socket through a **filter**, on
every device:

- the client's messages must be on an allow-list for protocol versions 0 to 4:
  `AUTH`, `VERSION`, `BYPASS`, `CHECKPRESENT`, `LOCKCONTENT`, `UNLOCKCONTENT`,
  `GET`, `PUT`, `DATA`, `VALID`, `INVALID`, `DATA-PRESENT`, `SUCCESS`,
  `FAILURE`, `GETTIMESTAMP`, `NOTIFYCHANGE`, `CONNECT git-upload-pack`,
  `CONNECTDONE`, `ERROR`;
- `VERSION n` is forwarded as `VERSION min(n, 4)`, the highest version the filter
  knows;
- on custodians, `REMOVE` and `REMOVE-BEFORE` are answered `FAILURE` without
  reaching git-annex, and logged at `error` with the peer's device ID: pigeon
  never sends one, so it signals a bug or a modified client. Laptops keep
  git-annex's default, since their copies never count (ADRs 0002, 0014);
- a `CONNECT` to anything but `git-upload-pack` is answered with `ERROR`, and the
  session closes: no device accepts a push;
- on custodians, the `SUCCESS` that answers `PUT`, `CHECKPRESENT` or
  `LOCKCONTENT` for a key waits until `pigeond` has synced that object and its
  directories to disk (`git annex contentlocation`, then `fsync`, or
  `F_FULLFSYNC` on macOS), since git-annex never syncs. While a restored
  repository is being re-checked (`checking`, [06 §6.3](06-status.md)),
  `LOCKCONTENT` is answered `FAILURE` (ADR 0046);
- on custodians, the filter answers `CHECKPRESENT k` itself. Over P2P
  git-annex answers it with `inAnnex` (`P2P/Annex.hs`), which ignores a removal
  lock, and a drop whose `LOCKCONTENT` failed counts that answer (E23). The
  filter sends `LOCKCONTENT k` instead; on `SUCCESS` it sends `UNLOCKCONTENT`
  and answers `SUCCESS`, otherwise `FAILURE`. So a copy the custodian holds
  locked for its own drop is never said present, and two custodians dropping
  at once never count each other (ADR 0054). The answer waits for the sync and
  is `FAILURE` while `checking`, as for `LOCKCONTENT`. A probe that meets the
  custodian's own drop starting makes that drop fail, to be retried;
- an unknown message, a malformed `DATA` length or a line over 4096 bytes closes
  the session. A length is checked before anything is forwarded, and a payload
  passes by length, never parsed. The server's output is parsed only for its
  framing, so that the filter's own replies fall between frames.

Every repository also has pigeon's `pre-receive` hook, which refuses every
push for any transport (ADR 0043). The filter is a pure state machine with
property tests (ADR 0012). It never forwards a removal on a custodian, it
forwards every allowed message unchanged but a custodian's `CHECKPRESENT`, no
positive answer for a key leaves it before that key's sync has completed, a
positive `CHECKPRESENT` only for a copy it could lock, and it never loses its
place across `DATA` boundaries, whatever the read sizes. E13 checks it against
git-annex, and E23 the probe.

## 9.5 Relays and discovery (ADR 0016)

- By default `pigeond` uses n0's public relays and address lookup, and mDNS on
  the local network (`iroh-mdns-address-lookup`), so devices on one LAN find
  each other without the Internet.
- If the roster names `relay = "<url>"` ([04 §4.2](04-roster-and-authority.md)),
  every device of that group uses it instead of n0's relays. It is an admin
  change like any roster change. Hosting `iroh-relay` is documented, not
  required.
- When the relay or lookup service is unreachable, peers on the LAN and direct
  connections keep working. Versions waiting for a peer then carry the reason
  `group-unreachable` with the detail "the relay service cannot be reached;
  computers on other networks cannot connect" ([06 §6.4](06-status.md)).
- What relay operators see (which node IDs are online, from where, who talks
  to whom) is stated in [02](02-guarantees.md); content and history are
  end-to-end encrypted.
- Network use is shaped by `pigeond` (ADR 0037, [15](15-resource-use.md)).

## 9.6 Invitations and joining (ADR 0017)

### The invitation

`pigeon invite [--admin] [--backup-machine] PERSON` (People → *Invite
someone*) creates a one-time invitation:

| Field | Value |
|---|---|
| `group` | the group ID (the genesis commit ID, [04 §4.1](04-roster-and-authority.md)) |
| `inviter` | the inviter's device ID and its current direct addresses |
| `relay` | the group relay, if any (§9.5) |
| `secret` | 256 random bits |
| `purpose` | `new-person {name, admin}`, or `new-device {person}`; `--backup-machine` adds `role = "custodian"` |
| `expires` | creation time + 24 hours by default |

- It is written as `pgn1-` followed by the lowercase base32 (no padding) of its
  binary encoding, and shown as text and as a QR code **[no ADR: encoding]**.
- **Who may invite.** A new person needs an admin's device; a new device of
  person P needs an admin's device or one of P's devices (ADR 0017). A device
  that may not invite says so and suggests asking an admin.
- The inviter stores only `SHA-256(secret)`, the purpose and the expiry in
  `invitations.json` ([11](11-local-state.md)). An invitation works once: the
  record is marked used before the roster commit is signed, so a crash cannot
  let it work twice. `pigeon invite --list` and `--cancel` manage pending ones.

### The join exchange

`pigeon join INVITATION [FOLDER]` (Welcome → *Join a group*):

1. The new device decodes the invitation, generates its key if it has none
   (§9.1), creates its repository (`git init`, `git annex init`) to obtain its
   git-annex UUID (a repository already in FOLDER, such as a custodian's
   surviving disk, gets a new UUID first, §9.9), and connects to the inviter on `pigeon/join/1`. The inviter
   must be online; while it is not, the CLI and web say "Waiting for Paul's
   computer…" and retry until the invitation expires.
2. It sends `Join {invitation_hash, proof, device_name, role, annex_uuid,
   capacity?}`, where `proof = HMAC-SHA256(secret, "pigeon-join-1" ‖ new
   device ID ‖ inviter device ID)`. Both device IDs are those authenticated by
   the QUIC handshake, so the secret is bound to this connection and a
   recorded exchange cannot be replayed.
3. The inviter checks the proof, the expiry and that the invitation is unused,
   marks it used, and signs a roster commit that adds the device (and the
   person, for `new-person`) under the authority rule
   ([04 §4.7](04-roster-and-authority.md)). A custodian's `maxsize` is its
   reported free space minus 10 %, and at most that free space minus
   `annex.diskreserve`, which an admin or its owner can change later
   (ADR 0050).
4. The inviter answers `Welcome {commit}`, or a refusal whose code becomes a
   Problem ([10 §10.6](10-interfaces.md)): `P-INVITE-EXPIRED`, `P-INVITE-USED`,
   `invite-unknown`, `invite-not-allowed`.
5. The new device, now in the roster, fetches over `pigeon/annex/1`, validates
   the history from the genesis named in the invitation (a history with another
   root is refused), checks that `commit` adds its own device ID, and installs
   the folders the person chose to keep ([07](07-capture-and-install.md)).

**Safety words** (ux.md §8.2): both screens show three words taken from a fixed
list of 2048 by the first 33 bits of `SHA-256("pigeon-words-1" ‖ secret ‖ new
device ID)`, together with "You are joining **Famille Geiger**, invited by
**Paul**". Both people can compare them over the phone. They do not gate the
join; they let the inviter notice that someone else used the invitation, and
the notification "Martine's laptop joined the group" on the inviter's computer
repeats them **[no ADR: safety words]**.

## 9.7 Removing a computer

`pigeon remove-computer NAME` (People → *I lost a computer*) removes a device
from the roster.

- It is a signed roster commit, valid from an admin or from another device of
  the same person (ADR 0017), and visible in the history ("Martine removed
  *Martine's old laptop*").
- Each device that accepts it closes connections to the removed device, deletes
  its remote, and marks its git-annex UUID `dead` (settings check,
  [03 §3.5](03-repository-and-roles.md)). If it was a custodian, its copies
  stop counting and repair starts ([08 §8.4](08-custody-loops.md)); the CLI
  says beforehand how many files will have fewer safe copies until then.
- Commits the removed device signed before its removal stay valid (the rule
  judges a commit by the roster at its parents). A removed device could still
  sign commits on an old parent; `pigeond` therefore never merges into `main`
  a commit signed by a device absent from the roster of its current `main`,
  and keeps it as a proposal for the admins **[no ADR: merge policy]**. Keys
  are assumed not stolen ([02](02-guarantees.md)); this narrows the damage when
  they are.
- Removal cannot unshare what the device already fetched (ADR 0017).
- A device removes itself with `pigeon leave` **[no ADR]**: it signs its own
  removal, then deletes its key and, if asked, its copy of the folder.

## 9.8 The stream header and trace context (ADR 0025)

Each stream that `pigeond` opens to a peer starts with one header line before
any git-annex byte **[no ADR: format]**:

```json
{"v":1,"group":"<group id>","traceparent":"00-<trace-id>-<span-id>-01"}
```

- `group` routes the stream to the right repository on the peer (§9.2) and is
  checked against the roster for admission (§9.3).
- `traceparent` is W3C trace context, so one action (an upload started by a
  capture, a fetch started by a change notice) is one trace across devices
  ([12](12-observability.md)). The peer answers with one line `{"v":1,"ok":true}`
  or a refusal, then the stream carries git-annex's protocol unchanged.
- The header is never passed to git-annex; the append-only filter (§9.4) starts
  after it.

## 9.9 Recovering a group (ADRs 0046, 0047)

Once its versions are protected, everything a group needs is on its
custodians: content, `main` and proposals (ADR 0046). The git-annex branch,
status reports and every device's local state are rebuilt or not needed. What
recovery takes in addition is authority, a key that can sign a roster commit:

| What survives | Recovery |
|---|---|
| a device of person P, such as P's custodian | P's new machine joins by an invitation from it (§9.6); `pigeon invite` runs on the custodian, directly or over ssh |
| a device of an admin | the admin invites new devices of anyone |
| an admin's recovery sheet | `pigeon recover SHEET` on a fresh machine (below) |
| no key of an admin, no sheet | `pigeon refound` (below), or `pigeon export` to read the files |

After each case the new machine fetches from the custodians and validates the
history from the genesis, as on a join. Its person then removes the lost
devices (§9.7).

- **Recovery sheets.** `pigeon recovery-sheet` (admins; People → *Print a
  recovery sheet*) adds a device of the admin with `role = "recovery"` and
  prints its key: base32 with a checksum and a QR code, with the group's name
  and ID and the device IDs of the custodians. No computer keeps the key.
  `pigeon recover SHEET [FOLDER] [--from DEVICE]` runs `pigeond` with the
  sheet's key as its node ID. Custodians admit it on `pigeon/annex/1` for git
  only, since a recovery device has no content. It validates `main` from the
  genesis named on the sheet, generates the machine's own key, and signs with
  the sheet's key a roster commit adding the machine as a device of the
  admin ([04 §4.3](04-roster-and-authority.md)). It stays reachable until a
  custodian has fetched that commit, then continues under its own key. It
  offers to remove the used sheet and the lost devices, and to print a new
  sheet. The People page recommends a sheet, or a second device, while an
  admin has a single device that can sign.
- **Refounding.** `pigeon refound [REPOSITORY]`, on any device that holds a
  repository of the group, signs a new genesis. Its tree is the last accepted
  tree, its roster names the refounder as the only admin, and it carries the
  trailer `Pigeon-Refound: <old group id> <old main>`. The old history is kept
  as `refs/pigeon/archive/<old group id>/main`; it replicates, and its keys
  stay in `keys()`. Other members join only by an invitation from the new
  group, which says who refounded it and when. An existing folder joins in
  place, and its objects are reused by key.
- **Export without pigeon.** `pigeon export --to DIR [--at DATE|COMMIT]
  [--deleted] [REPOSITORY…]` runs without `pigeond`, network or key
  ([10 §10.2](10-interfaces.md)). It reads one or more local repositories,
  checks each object's SHA-256, and writes plain files with a `MANIFEST.tsv`
  (path, size, SHA-256, commit, author, date), listing what it could not find.
  Each custodian also keeps `RECOVERY.txt` next to its repository
  ([11 §11.3](11-local-state.md)): the group's name, the other custodians,
  and the git and git-annex commands that read the files without pigeon, as
  E21 runs them (`git clone`, `git annex init`, `git remote add` and `git
  fetch` per other custodian, `git annex get .`, and `get --all` for the
  history).
- **Replacing a custodian.** The failed one is removed (§9.7), and its
  replacement joins as a new device. A surviving disk is reused under a new
  UUID (`git config --unset annex.uuid`, then `git annex init`), and its
  copies count only after a hashing `git annex fsck --all` (E21).
  A custodian that finds its repository older than its own last status report,
  as relayed by peers (restored from a backup, rolled back), reports
  `checking` and is not counted until the same fsck has corrected its location
  log (ADR 0046).
