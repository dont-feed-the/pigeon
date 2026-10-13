# 9. Network and joining

> Mission: specify how devices find, authenticate and talk to each other: the
> device key, the iroh transport helper, the `pigeond` endpoint and its local
> socket, append-only serving on custodians, relays, invitations, and removing a
> computer. Everything that crosses the network crosses it here.

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
- an unknown message, a malformed `DATA` length or a line over 4096 bytes closes
  the session. A length is checked before anything is forwarded, and a payload
  passes by length, never parsed. The server's output is parsed only for its
  framing, so that the filter's own replies fall between frames.

Every repository also has pigeon's `pre-receive` hook, which refuses every
push for any transport (ADR 0043). The filter is a pure state machine with
property tests (ADR 0012). It never forwards a removal on a custodian, it
forwards every allowed message unchanged, and it never loses its place across
`DATA` boundaries, whatever the read sizes. E13 checks it against git-annex.

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
   git-annex UUID, and connects to the inviter on `pigeon/join/1`. The inviter
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
   reported free space minus 10 %, which an admin can change later
   **[no ADR: default maxsize]**.
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
