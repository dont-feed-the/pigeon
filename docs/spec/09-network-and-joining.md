# 9. Network and joining

> Mission: the device key, the transport helper, the endpoint and local
> socket, append-only serving, relays, invitations, removal and recovery.
> The `filter`, `pigeon-proto`, codec and network laws and the
> `Authority.tla` invariants cited below are normative.

## 9.1 The device key (ADR 0007)

- Each device has one **ed25519 key**, made when it creates or joins a
  group. Its public key in 64 lowercase hex digits is the **device ID**:
  iroh node ID, commit signer, roster key. One `pigeond` serves all the
  user's groups with it (ADR 0011), so two groups see the same device.
- It is stored once as an OpenSSH private key, mode 0600, at
  `<config>/device-key` ([11](11-local-state.md)). git signs with it (SSH
  signatures) and iroh uses it as its secret key; pigeon verifies signatures
  itself (SSHSIG, namespace `git`) against the roster at the commit's
  parents ([04 §4.4](04-roster-and-authority.md)).
- The key is not encrypted at rest, so `pigeond` starts unattended; stolen
  keys are outside the guarantees ([02 §2.2](02-guarantees.md)).

## 9.2 The transport helper `git-annex-p2p-iroh` (ADR 0006)

The remote URL `p2p-annex::iroh:<device-id>`
([03 §3.6](03-repository-and-roles.md)) makes git-annex run the helper, a
thin client of `pigeond` (ADR 0011), in the three modes git-annex fixes:

| Called as | The helper |
|---|---|
| `git-annex-p2p-iroh address` | prints this device's ID, asked of `pigeond` |
| `git-annex-p2p-iroh <address>` | relays stdin/stdout to a stream `pigeond` opens to that peer for this repository; exits non-zero if it cannot connect, zero when the peer closes |
| `git-annex-p2p-iroh <address> <socket>` | registers git-annex's unix socket with `pigeond` for this repository, and stays until either ends it |

`pigeond` enables listening once per repository and runs git-annex's
`remotedaemon` as a child. The helper names its repository by absolute path
in each request and reads no git configuration. Each serving repository
keeps one git-annex token for its own address, which the filter puts in the
client's `AUTH` (`auth_is_rewritten_and_checked`), so no token travels.
Without `pigeond`, or for a device not in the roster, the helper exits
non-zero at once with its problem code (`peer-unreachable`).

## 9.3 The endpoint of `pigeond` and the local socket (ADR 0011)

### The iroh endpoint

`pigeond` owns the device's only iroh endpoint (QUIC, TLS 1.3, hole
punching, relays), with these ALPN protocols:

| ALPN | Carries | Accepted from |
|---|---|---|
| `pigeon/annex/1` | one git-annex P2P session (git and content) per stream | devices of the roster of that group |
| `pigeon/join/1` | the join exchange (§9.6) | anyone holding a valid invitation |
| `pigeon/telemetry/1` | OTLP from devices of the group to its telemetry device (ADR 0025) | devices of the roster, on the telemetry device only |
| `pigeon/debug/1` | `pigeon debug bundle` requests ([12](12-observability.md)) | devices of the roster |

- **Versions** (ADR 0048): a device offers every version of each ALPN it
  supports; with none in common, both show `peer-too-old` or `peer-too-new`
  and the release to install. The header (§9.8) and invitations (`pgn1-`)
  are versioned likewise.
- **Admission:** on `annex`, `telemetry` and `debug`, only a peer whose
  node ID is a device of the roster of the current `main` of the group the
  header names. Anything else is closed with a reason, logged at `warn`,
  and never reaches git-annex (ADR 0006).
- Connections are reused for all streams and groups; each git-annex session
  is one bidirectional QUIC stream.

### The local socket

The CLI, the web server and the helper reach `pigeond` over one per-user
socket, mode 0600 (`$XDG_RUNTIME_DIR/pigeon/pigeond.sock` on Linux, under
`$TMPDIR` on macOS; `PIGEON_SOCKET` overrides it). It carries one JSON
request or response per line, matched by `id`: the internal API
([10 §10.1](10-interfaces.md)) plus the helper's `Address`, `Connect` and
`Listen`, after whose `Connected` or `Listening` reply only raw bytes pass
(`pigeon-proto` laws `an_envelope_is_one_line`, `requests_round_trip`,
`responses_round_trip`, `raw_bytes_after_the_last_line_are_kept`). A
remotedaemon socket path over 100 bytes is reached through a short link
beside it **[no ADR]**. `pigeond` creates the socket's directory (0700)
only if absent and refuses a second instance; clients check that the peer
is the same user.

## 9.4 Serving peers: pull only, append-only on custodians (ADR 0014)

`remotedaemon` serves P2P read-write whatever the environment, and any peer
can push refs over it (E12, E13). So `pigeond` passes every incoming session
through a **filter**, the pure state machine `pigeon-core::filter`, whose
laws in `tests/filter_laws.rs` are normative:

| Rule | Law |
|---|---|
| **allow-list** (protocol versions 0 to 4): `AUTH`, `VERSION`, `BYPASS`, `CHECKPRESENT`, `LOCKCONTENT`, `UNLOCKCONTENT`, `GET`, `PUT`, `DATA`, `VALID`, `INVALID`, `DATA-PRESENT`, `SUCCESS`, `FAILURE`, `GETTIMESTAMP`, `NOTIFYCHANGE`, `CONNECT git-upload-pack`, `CONNECTDONE`, `ERROR`; `VERSION n` is forwarded as `VERSION min(n, 4)`; after `CONNECT git-upload-pack` only `DATA`, `CONNECTDONE` and `ERROR` pass | `allowed_streams_pass_unchanged`, `chunking_does_not_matter` |
| **no push anywhere**: any other `CONNECT` gets `ERROR` and closes; each repository's `pre-receive` hook also refuses every push | `custodians_never_forward_removals_or_pushes` |
| **no removal on custodians**: `REMOVE` and `REMOVE-BEFORE` get `FAILURE`, logged at `error` with the peer's ID; laptops keep git-annex's default, since their copies never count (ADR 0002) | `custodians_never_forward_removals_or_pushes` |
| **sync before success** (custodians): a positive answer for `PUT`, `CHECKPRESENT` or `LOCKCONTENT` of a key waits until the object and its directories are synced; a failed sync closes the session | `custodian_answers_only_after_sync` |
| **probe** (custodians): `CHECKPRESENT k` is answered by taking and releasing `LOCKCONTENT k`, since git-annex's presence check ignores a removal lock (E23); a probe that meets the custodian's own drop starting makes that drop fail, to be retried | `custodian_answers_only_after_sync` |
| **hash before proving** (custodians): another custodian's lock, and a probe's, succeed only after the filter, holding the lock, has hashed the object against its key; on a mismatch it answers `FAILURE` and queues a scrub of the key; laptops' locks are not hashed | `custodian_answers_only_after_sync` |
| **strict framing**: on custodians, a request sent before the last is answered, or a positive answer to no request, closes the session; everywhere, so do an unknown message, a malformed `DATA` length or a line over 4096 bytes; payloads pass by length, unparsed | `unattributed_success_closes`, `malformed_input_closes` |
| a key from a peer is printable ASCII, does not start with `-` and holds no `/` | `pigeond/tests/net_laws.rs` `implausible_keys_are_refused` |

While a restored repository is `checking` ([06 §6.3](06-status.md)),
`LOCKCONTENT` and `CHECKPRESENT` get `FAILURE` (ADR 0046). E13 checks the
filter against git-annex, and E23 the probe.

## 9.5 Relays and discovery (ADR 0016)

Not yet coded: the slice uses direct addresses and mDNS (13 §13.12).
By default `pigeond` uses n0's public relays and address lookup, and mDNS
for one LAN. A roster `relay = "<url>"` ([04 §4.2](04-roster-and-authority.md))
replaces n0's relays, as an admin change. Without relay or lookup, LAN and
direct connections still work, and versions waiting for a peer carry
`group-unreachable` ([06 §6.4](06-status.md)). Relays see only metadata
([02 §2.4](02-guarantees.md)).

## 9.6 Invitations and joining (ADR 0017)

`pigeon invite [--admin] PERSON` (People → *Invite someone*), or `pigeon
invite --backup-machine [NAME]` for a backup machine of the inviter's own
person (ux.md §8) **[no ADR: reading]**, creates a one-time invitation:

| Field | Value |
|---|---|
| `group` | the group ID (the genesis commit ID, [04 §4.1](04-roster-and-authority.md)) |
| `inviter` | the inviter's device ID and its current direct addresses |
| `relay` | the group relay, if any (§9.5) |
| `secret` | 256 random bits |
| `purpose` | `new-person {name, admin}`, or `new-device {person}`; `--backup-machine` adds `role = "custodian"` and the machine's name, if given |
| `expires` | creation time + 24 hours by default |

- It is written `pgn1-` plus the lowercase unpadded base32 of its binary
  encoding, shown as text and QR code **[no ADR: encoding]**.
- **Who may invite:** for a new person, an admin's device; for a new device
  of person P, an admin's device or one of P's (ADR 0017).
- The inviter keeps the secret, its SHA-256, purpose and expiry in
  `invitations.json` ([11 §11.3](11-local-state.md); `invitations_round_trip`),
  readable by its user only. The hash finds the record; the secret checks
  the proof of step 2 **[no ADR: correction]** (issue #17). It is marked
  used before the roster commit is signed. `pigeon invite --list` and
  `--cancel` manage pending ones.

`pigeon join INVITATION [FOLDER]` (Welcome → *Join a group*):

| # | Who | Step |
|---|---|---|
| 1 | new device | decodes the invitation, makes its key if it has none (§9.1) and its repository (one already in FOLDER gets a new UUID first, §9.9), and connects to the inviter on `pigeon/join/1`, retrying ("Waiting for Paul's computer…") until the invitation expires |
| 2 | new device | sends `Join {v, invitation_hash, proof, device_name, role, annex_uuid, capacity?, addresses}`, with `proof = HMAC-SHA256(secret, "pigeon-join-1" ‖ new device ID ‖ inviter device ID)`, both IDs from the QUIC handshake, so a recorded exchange cannot be replayed; `addresses` let the inviter reach it before mDNS does **[no ADR]** |
| 3 | inviter | checks proof, expiry and that it is unused, marks it used, and signs a roster commit adding the device (and the person, for `new-person`) under the authority rule ([04 §4.7](04-roster-and-authority.md)); a custodian's `maxsize` is its reported free space minus 10 %, at most that minus `annex.diskreserve` ([08 §8.9](08-custody-loops.md)) |
| 4 | inviter | answers `Welcome {commit, annex_uuid}` (its own UUID), or a refusal that becomes a Problem ([10 §10.6](10-interfaces.md)): `P-INVITE-EXPIRED`, `P-INVITE-USED`, `invite-unknown`, `invite-not-allowed` |
| 5 | new device | fetches over `pigeon/annex/1`, validates the history from the genesis named in the invitation (another root is refused), checks that `commit` adds its own ID, and installs the folders to keep ([07](07-capture-and-install.md)) |

**Safety words** (ux.md §8.2) **[no ADR: safety words]**: both screens show
three words from a fixed list of 2048, by the first 33 bits of
`SHA-256("pigeon-words-1" ‖ secret ‖ new device ID)`, with "You are joining
**Famille Geiger**, invited by **Paul**". They do not gate the join; they
let the inviter notice that someone else used the invitation, and the
inviter's "Martine's laptop joined the group" notification repeats them.

## 9.7 Removing a computer

`pigeon remove-computer NAME` (People → *I lost a computer*) signs a roster
commit removing a device, valid from an admin or another device of the same
person (ADR 0017), and visible in the history.

- Each device that accepts it closes connections to it, deletes its remote
  and marks its UUID `dead` ([03 §3.5](03-repository-and-roles.md)). A
  custodian's copies stop counting and repair starts
  ([08 §8.4](08-custody-loops.md)); the CLI first says how many files will
  have fewer safe copies meanwhile.
- Commits it signed that the removal had seen stay valid; those it had not
  seen are **revoked** wherever the removal arrives, with the keys they
  added and what those keys sign (04 §4.4, ADR 0051; `Authority.tla`
  `RemovalRevokesAdded`, `RemovedSilenced`). They count in no catalogue or
  roster and show as suggestions of the device's person. *I lost a
  computer* offers to remove the devices the lost one added.
- Removal cannot unshare what the device already fetched (ADR 0017).
  `pigeon leave` **[no ADR]** signs the device's own removal, then deletes
  its key and, if asked, its copy of the folder.

## 9.8 The stream header and trace context (ADR 0025)

Each stream `pigeond` opens to a peer starts, before any git-annex byte,
with one header line of at most 4096 bytes, within 10 seconds
(`stream_headers_round_trip`):

```json
{"v":1,"group":"<group id>","traceparent":"00-<trace-id>-<span-id>-01"}
```

`group` routes the stream and is checked for admission (§9.3);
`traceparent` (W3C) makes one action one trace across devices
([12](12-observability.md)). The peer answers `{"v":1,"ok":true}` or a
refusal (`not-in-roster`, `unknown-group`, `not-listening`, `bad-header`,
`peer-too-old`). Then the stream carries git-annex's protocol; the header
never reaches git-annex, and the filter (§9.4) starts after it.

## 9.9 Recovering a group (ADRs 0046, 0047)

Once versions are protected, everything a group needs is on its custodians:
content, `main` and proposals (ADR 0046). Recovery also needs authority, a
key that can sign a roster commit:

| What survives | Recovery |
|---|---|
| a device of person P, such as P's custodian | P's new machine joins by an invitation from it (§9.6); `pigeon invite` runs on the custodian, directly or over ssh |
| a device of an admin | the admin invites new devices of anyone |
| an admin's recovery sheet | `pigeon recover SHEET` on a fresh machine (below) |
| no key of an admin, no sheet | `pigeon refound` (below), or `pigeon export` to read the files |

Each new machine then fetches from the custodians and validates from the
genesis, as on a join, and its person removes the lost devices (§9.7). The
tools are those of ADR 0047, which fixes their formats:

- **Recovery sheets:** `pigeon recovery-sheet` (admins) adds a device of
  the admin with `role = "recovery"` and prints its key; no computer keeps
  it. `pigeon recover SHEET [FOLDER] [--from DEVICE]` signs with it a roster
  commit adding the machine as the admin's device
  ([04 §4.3](04-roster-and-authority.md)); custodians admit the sheet's key
  for git only. The People page recommends a sheet or a second device while
  an admin has a single signing device.
- **Refounding:** `pigeon refound [REPOSITORY]` signs a new genesis with the
  last accepted tree and the refounder as only admin, keeping the old
  history replicated and its keys in `keys()`; existing folders join the new
  group in place, objects reused by key.
- **Export without pigeon:** `pigeon export --to DIR [--at DATE|COMMIT]
  [--deleted] [REPOSITORY…]` needs no `pigeond`, network or key
  ([10 §10.2](10-interfaces.md)) and checks every object's SHA-256. Each
  custodian keeps `RECOVERY.txt` beside its repository
  ([11 §11.3](11-local-state.md)), with the commands E21 runs.
- **Replacing a custodian:** the failed one is removed (§9.7) and its
  replacement joins as a new device. A surviving disk gets a new UUID and
  counts only after a hashing fsck of everything (E21). A custodian whose
  repository is older than its last status report relayed by peers reports
  `checking` and is not counted until that fsck has corrected its location
  log (ADR 0046).
