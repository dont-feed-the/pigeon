# 9. Network and joining

> Mission: the device key, the transport helper, the endpoint and local
> socket, append-only serving, relays, invitations, removal and recovery.
> The `filter`, `pigeon-proto`, codec and network laws and the
> `Authority.tla` invariants cited below are normative.

## 9.1 The device key (ADR 0007)

- Each device has one **ed25519 key**, made when it creates or joins a
  group. Its public key in 64 lowercase hex digits is the **device ID**:
  iroh node ID, commit signer, roster key. One `pigeond` serves the
  user's group with it (ADR 0011); a computer is in one group only
  (§9.6).
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
| `pigeon/join/1` | the join exchange (§9.6) | anyone holding the group's invitation |
| `pigeon/telemetry/1` | OTLP from devices of the group to its telemetry device (ADR 0025) | devices of the roster, on the telemetry device only |
| `pigeon/debug/1` | `pigeon debug bundle` requests ([12](12-observability.md)) | devices of the roster |

- **Versions** (ADR 0048): a device offers every version of each ALPN it
  supports; with none in common, both show `peer-too-old` or `peer-too-new`
  and the release to install. The header (§9.8) and invitations (`pgn2-`)
  are versioned likewise.
- **Admission:** on `annex`, `telemetry` and `debug`, only a peer whose
  node ID is a device of the roster of the current `main` of the group the
  header names. Anything else is closed with a reason, logged at `warn`,
  and never reaches git-annex (ADR 0006).
- Connections are reused for all streams and groups; each git-annex session
  is one bidirectional QUIC stream.

### The local socket

The CLI, the web server and the helper reach `pigeond` over one per-user
socket, mode 0600: `pigeond.sock` in the runtime directory of
[11 §11.2](11-local-state.md), `$XDG_RUNTIME_DIR/pigeon` on Linux and
`~/Library/Caches/pigeon/run` on macOS, which the clients take from
`pigeon-sys` as the daemon does; a client uses a backup machine's
`/run/pigeon/pigeond.sock` when only that one answers, and `PIGEON_SOCKET`
overrides both. It carries one JSON
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
| **probe** (custodians): `CHECKPRESENT k` is answered by taking and releasing `LOCKCONTENT k`, since the stock git-annex's presence check ignores a removal lock (E23; pigeon's git-annex checks it, ADR 0069, and the probe stays); a probe that meets the custodian's own drop starting makes that drop fail, to be retried | `custodian_answers_only_after_sync` |
| **hash before proving** (custodians): another custodian's lock, and a probe's, succeed only after the filter, holding the lock, has hashed the object against its key; on a mismatch it answers `FAILURE` and queues a scrub of the key; laptops' locks are not hashed | `custodian_answers_only_after_sync` |
| **strict framing**: on custodians, a request sent before the last is answered, or a positive answer to no request, closes the session; everywhere, so do an unknown message, a malformed `DATA` length or a line over 4096 bytes; payloads pass by length, unparsed | `unattributed_success_closes`, `malformed_input_closes` |
| a key from a peer is printable ASCII, does not start with `-` and holds no `/` | `pigeond/tests/net_laws.rs` `implausible_keys_are_refused` |

While a restored repository is `checking` ([06 §6.3](06-status.md)),
`LOCKCONTENT` and `CHECKPRESENT` get `FAILURE` (ADR 0046). E13 checks the
filter against git-annex, and E23 the probe.

## 9.5 Relays and discovery (ADR 0016)

By default `pigeond` uses n0's public relays and address lookup, and mDNS
for one LAN; `[network] relays` ([11 §11.2](11-local-state.md)) names
another relay, or `off` for direct addresses and mDNS only, as the lab
runs (13 §13.12). A roster `relay = "<url>"` ([04 §4.2](04-roster-and-authority.md))
replaces n0's relays, as an admin change: a `pigeond` set up with n0's
uses every group's relay instead, and n0's again once no roster names one,
while a person's own `[network] relays` wins (`net::relays`); an
invitation carries the relay, so the new device reaches the admitters
through it. Without relay or lookup, LAN and
direct connections still work, and versions waiting for a peer carry
`group-unreachable` ([06 §6.4](06-status.md)). Relays see only metadata
([02 §2.4](02-guarantees.md)).

## 9.6 The group's invitation and joining (ADR 0075)

A group has one invitation: the roster's `invitation`, 256 random bits that
`pigeon create` writes in the genesis and no roster change can change
(`diff` answers none, [04 §4.8](04-roster-and-authority.md)). `pigeon
invite` (People → *Invite*) prints it, on any member, inside the line to
share:

```sh
curl -fsSL https://github.com/dont-feed-the/pigeon/releases/latest/download/pigeon-install | sh -s -- --join pgn2-…
```

The installer installs or updates pigeon, then runs `pigeon join` with the
rest of the line on the terminal ([14 §14.5](14-platforms-and-packaging.md)).
`pigeon create` prints the same line. The code is `pgn2-` plus the lowercase
unpadded base32 of its JSON (`invitations_round_trip`):

| Field | Value |
|---|---|
| `group` | the group ID (the genesis commit ID, [04 §4.1](04-roster-and-authority.md)) |
| `name`, `k` | the group's name, and K for a backup machine's `wanted` |
| `admitters` | the device that wrote it, then the admins' laptops and backup machines: each one's device ID, person's name and the direct addresses that device knew |
| `secret` | the roster's `invitation` |
| `relay` | the group relay, if any (§9.5) |
| `refounded` | for a refounded group, what the joiner accepts (§9.9) |

`pigeon join INVITATION [FOLDER] [--person NAME] [--name NAME]
[--backup-machine]` (Welcome → *Join a group*) asks, on a terminal, what
the line did not give: the folder (the group's name in the home folder by
default; none for a backup machine) and the person's name (the account's,
`$USER`, by default). `--name` names the computer in the roster, else
`<computer name>-<6 hex digits of its device ID>`, the computer name being
the one of Sharing settings on macOS and the host name up to its first dot
elsewhere (`pigeon-sys::host`) **[no ADR]**.
A name another device has gets made unique at step 3, and the join's
answer (`joined-renamed`) tells the new device's person the name it got.
A join that fails takes back what it made: the group is served only once
its welcome is checked, and its folder holds nothing of the attempt
(`pigeond/src/join_attempt.rs`, `tests/join_retry.rs`), so a retry in the
same folder starts clean **[no ADR]**.

| # | Who | Step |
|---|---|---|
| 1 | new device | decodes the invitation, makes its key if it has none (§9.1) and its repository (an existing repository can rejoin in place only under §9.9), and connects to each admitter in turn on `pigeon/join/1`, retrying for ten minutes |
| 2 | new device | sends `Join {v, invitation_hash, proof, person, existing, device_name, role, annex_uuid, capacity?, addresses}`, with `proof = HMAC-SHA256(secret, "pigeon-join-1" ‖ new device ID ‖ admitter device ID)`, both IDs from the QUIC handshake, so a recorded exchange cannot be replayed; `addresses` let the admitter reach it before mDNS does **[no ADR]** |
| 3 | admitter | finds the group whose invitation hashes to `invitation_hash`, checks the proof, and decides the change (`pigeon-core::invitation::admit`, laws in `tests/invitation.rs`): a `person` the roster lacks adds a person who is not an admin, with an ID of the name's ASCII letters and digits (`person` if none) and a number if a person has it; a person's name or ID, ignoring case, adds the device to that person only with `existing`, else it answers `person-exists`; a device of the roster with the same role and annex UUID is welcomed again with no change; a removed device, or one asking another role or UUID, is refused. A device name the roster has gets `-2`, `-3`… It signs the roster commit under the authority rule ([04 §4.7](04-roster-and-authority.md)); a custodian's `maxsize` is its reported free space minus 10 %, at most that minus `annex.diskreserve` ([08 §8.9](08-custody-loops.md)) |
| 4 | admitter | answers `Welcome {commit, annex_uuid, peers}` (its own UUID), or a refusal that becomes a Problem ([10 §10.6](10-interfaces.md)): `person-exists`, `invite-unknown`, `invite-not-allowed`. On `invite-not-allowed` the new device asks the next admitter: an admitter that is not an admin may add only devices of its own person |
| 5 | new device | on `person-exists`, asks whether this computer is that person's (`--yes` answers yes) and sends again with `existing`; once welcomed, fetches over `pigeon/annex/1`, validates the history from the genesis named in the invitation (another root is refused), checks that `commit` adds its own ID, and installs the folders to keep ([07](07-capture-and-install.md)) |

- **Whoever holds the line joins**, under any person's name, with that
  person's rights (ADR 0075). The invitation never changes: a person
  removed (§9.7) can join again with it; only refounding (§9.9) gives a new
  one. Every join is a signed, visible roster commit.
- A device already in the group refuses to join it again
  (`already-in-group`).
- A new device is **in the group once step 5 checked its welcome**, and
  only then **[no ADR]**: `config.toml` records the group and its loops
  start last ([11 §11.2](11-local-state.md)). Until then its repository's
  `joining` names the group ([11 §11.3](11-local-state.md)), and a join
  cut short by a crash resumes in that repository when the same
  invitation is given again: its annex UUID is the one the admitter's
  roster may already hold, which step 3 welcomes again. A group that
  `config.toml` lists without `main`, as releases up to 0.5.13 left a
  failed join, becomes such a join again when `pigeond` starts
  (`pigeond/src/join.rs`, `tests/join_resume.rs`).
- A computer is in **one group only** **[no ADR]**: the web pages and the
  commands without a path show one group. While it is in a group,
  `create`, `join` and `recover` of another one are refused before anything
  is written (`in-another-group`), naming the group and saying how to leave
  it (§9.7). Joining in place the refounding of the group it is in
  replaces that group, and goes on (§9.9).

A join either completes or takes back what it made **[no ADR]**
(`pigeond/src/join_attempt.rs`, `tests/join_retry.rs`): the new device
serves the group, in `config.toml` and to its peers, only once step 5 has
passed; until then the group is pending and no round reads it. On any
failure, a refusal or an inviter that stays unreachable included, it
forgets the group and removes what it wrote: the repository, pigeon's own
files, the files of the group's tree it wrote that are unchanged, and the
folder if the join made it. A join starts only in an empty or new folder,
so nothing that was there before is touched, and a retry in the same folder
starts clean. The annex UUID is kept in `<state>/joins/<group>`
([11 §11.2](11-local-state.md)) until the join succeeds, so that a retry
after step 3 resumes the welcome with the same UUID. The web's requests
run to their end even when the browser leaves (10 §10.3).

**Safety words** (ux.md §8.2) **[no ADR: safety words]**: both screens show
three words from a fixed list of 2048, by the first 33 bits of
`SHA-256("pigeon-words-1" ‖ secret ‖ new device ID)`, with "You are joining
**Famille Geiger**, invited by **Paul**". They do not gate the join; they
let the admitter's person notice who joined, and the admitter's "Martine's
laptop joined the group" notification repeats them.

## 9.7 Removing a computer

`pigeon remove-computer NAME` (People → *I lost a computer*) signs a roster
commit removing a device, valid from an admin or another device of the same
person (ADR 0075), and visible in the history.

- Each device that accepts it closes connections to it, deletes its remote
  and marks its UUID `dead` ([03 §3.5](03-repository-and-roles.md)). A
  custodian's copies stop counting and repair starts
  ([08 §8.4](08-custody-loops.md)); the CLI first says how many files will
  have fewer safe copies meanwhile.
- The question before it is done (`confirm-remove-computer`,
  `web-remove-confirm`) says what becomes of the files: a computer's stay
  on it but are shared no more, and it can be invited again. A backup
  machine has its own question (`confirm-remove-backup`,
  `web-remove-backup-confirm`), and when fewer backup machines than K
  would be left it says plainly that the files will not be safe until one
  is added (`remove-backup-unsafe`; `Machines` tells K as `copies`)
  **[no ADR]**.
- Commits it signed that the removal had seen stay valid; those it had not
  seen are **revoked** wherever the removal arrives, with the keys they
  added and what those keys sign (04 §4.4, ADR 0051; `Authority.tla`
  `RemovalRevokesAdded`, `RemovedSilenced`). They count in no catalogue or
  roster and show as suggestions of the device's person. *I lost a
  computer* offers to remove the devices the lost one added.
- The removed device learns it **[no ADR]** (`pigeond/src/removed.rs`,
  `tests/status_truth.rs`): once the roster of its accepted `main` no
  longer lists it, or a peer refuses it as `not-in-roster` although the
  roster last fetched from that peer listed it (as for `pigeon leave`, a
  peer that never learned its join proves nothing), no backup machine
  counts on it any more ([06 §6.5](06-status.md)): its headline says, at
  *Needs you*, that it was removed, that its files stay in its folder but
  are no longer shared or backed up, and to ask someone in the group for
  a new invitation; its lists of people and machines say they are the
  group as it last knew it. A device leaving or finishing an in-place join
  is not removed.
- Removal cannot unshare what the device already fetched (ADR 0017), and
  does not change the group's invitation (ADR 0075).
  `pigeon leave` **[no ADR]** signs the device's own removal, then deletes
  its key and, if asked, its copy of the folder.

Coded so far (`pigeond/src/leave.rs`, `tests/leave.rs`): a computer that
is the group's only machine (recovery sheets aside) has nobody to tell: it
forgets the group at once, as below, without signing a removal. So does a
computer whose group's roster is in a format before this release's, which
it never reads again (`left-unreadable`); a later format asks for an
update (`update-needed`). Otherwise
`pigeon leave` refuses the group's last admin computer (`leave-last-admin`:
no device of an admin, recovery sheets included, would remain), signs the
removal and records it in `.git/pigeon/leaving`
([11 §11.3](11-local-state.md)). Until a peer has it, the group's rounds
do nothing else, and the device still serves it. A peer has it once its
fetched `main` holds it or it refuses the device as `not-in-roster`
(§9.3). Then the group leaves `config.toml`, pointer files of content not
here are deleted, `.git` and `.pigeon` move to
`<state>/left/<group>-<time>/`, so the folder keeps its files as plain
files, and the device key goes once no group is left; the running
`pigeond` keeps it, and writes it back for the next group it serves, so
the device ID that group's roster names survives a restart. A custodian's
repository stays in place. Deleting the folder, and saying first how many
files will have fewer safe copies, are not coded.

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
| a device of person P, such as P's custodian | P's new machine joins with the invitation that device prints (§9.6), as P; `pigeon invite` runs on the custodian, directly or over ssh |
| a device of an admin | any new machine joins with the invitation; that device admits it |
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
  an admin has a single signing device. Admins can prepare the printable
  sheet from People & machines: localized instructions, the exact text
  code and an SVG QR of that code, on a `no-store` response. Oversized
  sheets keep the complete text with an explicit QR fallback. Public roster
  history identifies when all a person's sheets name older backup devices,
  and the page recommends a replacement. A sheet is refused before adding
  a key if no backup has joined, or if none answers a fetch at once
  (`recovery-backup-offline`): nothing is signed then. If no backup
  receives its roster commit within two minutes, the unprinted key is
  revoked (`recovery-not-received`); adding, waiting and revoking run in a
  task of their own, so a client that leaves does not stop them. The
  sheet's device is named `YYYY-MM-DD-xxxxxx` after the day it was printed
  and 6 hex digits of its ID; the sheet prints that name (`label` in its
  code) and the People page lists it as a paper recovery sheet printed on
  that day **[no ADR]**.
- **Refounding:** `pigeon refound [REPOSITORY]` signs a new genesis with the
  last accepted tree and the refounder as only admin, keeping the old
  history replicated and its keys in `keys()`; existing folders join the new
  group in place, objects reused by key.

  Coded so far (`pigeond/src/refound.rs`, `join_in_place.rs`,
  `tests/refound.rs`): `Refound {repository?}` asks first (`confirm-refound`),
  then commits a root whose tree is `main`'s with a roster keeping the
  group's name, K, retention, relay and folder boundaries, and only the refounder's
  person (admin) and device, with `Pigeon-Refound: <old group> <old main>`.
  It sets `refs/pigeon/archive/<old group>/main`, forgets the old group and
  serves the repository as the new one with the same annex UUID and token.
  Owned folders belong to the surviving person; add-only folders keep
  only surviving stewards, with an empty set meaning the new admins.
  Peers fetch `refs/pigeon/archive/*`; a device keeps only the archive its
  genesis names, and `retention::keys` holds its keys. Invitations carry
  `refounded {from, by, at}`; `pigeon join` asks `confirm-join-refounded`,
  and a laptop folder or bare backup whose root is `from` joins in place.
  The web requires a separate confirmation too. The welcome and signed
  archive links are verified in a private staging repository with no
  old local objects and no inbound annex listener. A rejected welcome
  leaves the existing repository and configuration unchanged. After
  validation a durable private intent lets startup finish locally across
  crashes, without redeeming the invitation again. The old transport is
  stopped before importing the new history. The exact signed archive tip
  becomes canonical; other local old tips and proposals are preserved
  under the old archive namespace. The index takes the new tree without
  rewriting user files (absent annexed files hidden, other differences
  captured as saves). Annex content, UUID and token are reused.
- **Export without pigeon:** `pigeon export --to DIR [--at DATE|COMMIT]
  [--deleted] [REPOSITORY…]` needs no `pigeond`, network or key
  ([10 §10.2](10-interfaces.md)) and checks every object's SHA-256. Each
  custodian keeps `RECOVERY.txt` beside its repository
  ([11 §11.3](11-local-state.md)), with the commands E21 runs.
  Coded so far (`pigeon/src/export.rs`, `repo.rs`, pigeon-core `export`,
  `pigeon/tests/export.rs`): export reads bare and non-bare repositories,
  takes the newest `main` or `--at`'s commit (a date gives the last commit of
  `main` before it), and finds each object in any repository's
  `annex/objects` or, for an unlocked file, its working tree; a file takes
  its name only once its SHA-256 matches the key. `--deleted` writes the
  last version of each deleted file under `.deleted/`; `MISSING.tsv` lists
  what is missing, damaged or blocked by a destination conflict. Deleted
  discovery follows the signed refounding chain, requiring each canonical
  archive ref and old genesis to match the root's signed link. A broken
  link refuses deleted-file export explicitly. This offline rescue checks
  link signatures and content hashes; it does not run the daemon's complete
  authority fold. Output files never replace another exported file, and
  exported symlinks cannot become destination directories. Metadata names
  avoid user paths under portable case folding. `history` and read-only
  `view` also accept virtual file paths below a bare backup repository.
  A backup machine writes `RECOVERY.txt` in its
  repository directory each round the note changes (`pigeond/src/recovery_txt.rs`).
- **Replacing a custodian:** the failed one is removed (§9.7) and its
  replacement joins as a new device. A surviving disk gets a new UUID and
  counts only after a hashing fsck of everything (E21). A custodian whose
  repository is older than its last status report relayed by peers reports
  `checking` and is not counted until that fsck has corrected its location
  log (ADR 0046).
