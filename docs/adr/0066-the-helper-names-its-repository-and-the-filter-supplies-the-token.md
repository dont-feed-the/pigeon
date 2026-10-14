# 0066. The helper names its repository by path, and the serving filter supplies git-annex's token

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume, in chat: "work on a first version of the implementation even if
  not all details are figured out yet"). Settles 09 §9.2's routing, 09 §9.3's
  ALPN names and 09 §9.8's header format, all marked **[no ADR]**, and
  corrects the remote URL of 03 §3.6 and 09 §9.2.

## Context

The first code of the transport (`pigeon-proto`, `git-annex-p2p-iroh`,
`pigeon_core::filter` and `pigeond`'s network module) met four gaps in the
specification.

- 09 §9.2 has the helper send the group ID, but 14 §14.6 lets it do I/O on
  the socket and stdio only, so it cannot read the group from git's
  configuration. `pigeond` knows its repositories.
- 03 §3.6 and 09 §9.2 write the remote URL as `p2p-annex::iroh+<device-id>`.
  git-annex parses `p2p-annex::<network>:<address>` (`P2P/Address.hs`,
  `formatP2PAddress` and `unformatP2PAddress`, lines 53–64). With `+`, the
  network name would be `iroh+<device-id>`, and the address would be empty.
- git-annex's P2P protocol opens with `AUTH <uuid> <token>`. remotedaemon
  accepts only tokens stored for its address in `.git/annex/creds/p2pauth`
  (`P2P/Auth.hs`, `loadP2PAuthTokens`). A client without a token sends an
  empty one (`Remote/P2P.hs`:154, `nullAuthToken`). By the time git-annex's
  bytes arrive, `pigeond` has already authenticated the peer by its device key
  and admitted it by the roster (09 §9.3).
- The filter of 09 §9.4 has to attribute each answer of git-annex to a
  request, so that it holds the right positive answers until their sync. A
  client that sends a request before the last is answered would break that
  attribution.

## Decision

1. **Routing by path.** The helper sends the absolute path of its repository:
   `GIT_DIR` if set, else its working directory. `pigeond` maps that path to
   the repository, and so to the group, as its git directory or its working
   tree or a path inside them. The helper's requests are `Address
   {repository}`, `Connect {repository, device}` and `Listen {repository,
   socket}`, after a `Hello`. A relative `socket` is made absolute by the
   helper.
2. **The remote URL** is `p2p-annex::iroh:<device-id>`, git-annex's own
   format.
3. **The token is the server's.** Each serving repository holds one token,
   which `git annex p2p --gen-addresses` stores for its address once. The
   filter replaces the token of the client's `AUTH` with it, and closes the
   session if the `AUTH` names another UUID than the roster's `annex-uuid`
   for the authenticated device. No token is ever exchanged between devices:
   the device key is the credential.
4. **Stream header.** The header of 09 §9.8 is one JSON line of at most 4096
   bytes, sent within 10 seconds of the stream's opening. The answer is
   `{"v":1,"ok":true}`, or `ok: false` with one of the refusals
   `not-in-roster`, `unknown-group`, `not-listening`, `bad-header`,
   `peer-too-old`. The ALPN names are those of the table of 09 §9.3.
5. **The filter is strict where attribution needs it.** On a custodian:
   - a request sent before the last is answered closes the session;
   - a positive answer from git-annex that answers no request closes it;
   - `ALREADY-HAVE` and `ALREADY-HAVE-PLUS` are held for a sync, as `SUCCESS`
     is;
   - a failed sync closes the session, so the held answer is never sent;
   - the probe keeps git-annex's lock until its sync, and its hash for a
     custodian peer (ADR 0059), are done, then unlocks and answers. A lock
     whose hash fails is released by the filter itself, since the client,
     answered `FAILURE`, will not release it;
   - a key from a peer reaches git-annex only if it is printable ASCII, does
     not start with `-` and holds no `/`.

   On every device, once `CONNECT git-upload-pack` is accepted, only `DATA`,
   `CONNECTDONE` and `ERROR` pass from the client.
6. **The local socket.** `PIGEON_SOCKET` overrides its path, for tests and
   the lab. On macOS, the per-user temporary directory is read from
   `$TMPDIR`. A client is served only if its peer credentials name the user
   who owns the socket file. A remotedaemon socket whose path is longer than
   100 bytes (`sun_path` holds 104 on macOS) is reached through a short
   symbolic link next to `pigeond`'s socket: git-annex picks a relative path
   for that reason (`getSocketFile` in `RemoteDaemon/Transport/P2PGeneric.hs`),
   but `pigeond` does not share its working directory.
7. **Dependencies.** `iroh` is built without default features, with
   `tls-ring`: no relays, no global discovery, mDNS when asked (ADR 0057).
   `cargo deny` checks the four targets of ADR 0005 only, so the relay
   client's wasm-only crates under the Unlicense are not counted. It allows
   `CDLA-Permissive-2.0` for `webpki-roots` and `webpki-root-certs` only, and
   ignores RUSTSEC-2024-0436, under which `paste`, a proc macro of iroh's
   `netwatch`, is unmaintained but not vulnerable. `Cargo.lock` is generated
   and exempt from the 1000-line rule.
8. **Layout.** `pigeond` has a library target (`src/lib.rs`), which its binary
   and the tests share. `git-annex-p2p-iroh` depends on `pigeond` for its
   integration test only.

## Consequences

- Moving a repository changes its path, and `pigeond` must know the new path
  before git-annex reaches a peer from it. The loops already watch their
  repositories.
- A stolen or copied token gains nothing: the filter replaces every token,
  and admission happens before git-annex sees a byte.
- A modified client that pipelines requests is cut off rather than
  mis-served. git-annex's own client waits for each answer
  (`P2P/Protocol.hs`, the client functions).
- The integration test `crates/git-annex-p2p-iroh/tests/annex_over_iroh.rs`
  runs on Linux and macOS in CI: copy, drop and get over iroh, a refused
  removal, a custodian peer's drop refused on a rotten copy with the object
  then moved out by fsck, and admission refused to a stranger and an unknown
  group. The filter's seven laws (`crates/pigeon-core/tests/filter_laws.rs`)
  and the protocol's five (`crates/pigeon-proto/tests/proto_laws.rs`) run
  beside it.
