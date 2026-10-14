# 16. Open questions

> Mission: list what is not decided: the ADRs still proposed, the places where
> this specification departs from a source, the details it decides without an
> ADR (each marked **[no ADR]** in the text), and the design questions left
> open. Under the [ADR policy](../adr/README.md#when-to-write-an-adr) a
> **[no ADR]** detail is settled here; it is raised to an ADR only if it turns
> out costly to reverse, or a reviewer asks for one.

## 16.1 Decisions awaited

Four ADRs are Proposed; each is a real choice that is costly to reverse. The
rules they rest on, and the details that were ADRs before 14 October 2026,
are in the chapters cited ([16 §16.3](#163-details-decided-without-an-adr)).

| Item | State | Who |
|---|---|---|
| ADR 0046: protected means K custodians in K failure domains hold the content and its commit, on disk | Proposed, for issues #11 and #12; [06 §6.2–6.3](06-status.md), [02 §2.1–2.2](02-guarantees.md), [03 §3.4](03-repository-and-roles.md), [04 §4.2](04-roster-and-authority.md), [08 §8.3](08-custody-loops.md), [09 §9.4](09-network-and-joining.md) apply it; E21 and E22 assert it, the `Repair` model checks it; holds the former ADR 0049 | Guillaume |
| ADR 0047: recovery by rejoin, recovery sheets, refounding, export | Proposed, for issue #11; [09 §9.9](09-network-and-joining.md), [04 §4.3](04-roster-and-authority.md), [10 §10.1](10-interfaces.md) apply it, E21 asserts it | Guillaume |
| ADR 0048: versioned formats and rules; *unknown* is never guessed | Proposed, for issue #15; [04 §4.2, §4.4](04-roster-and-authority.md), [06 §6.3](06-status.md), [09 §9.3](09-network-and-joining.md), [14 §14.3](14-platforms-and-packaging.md) apply it; fixtures and the compatibility law in [13](13-verification.md) | Guillaume |
| ADR 0051: authority is a fold of signed changes; forks merge without an admin; deciders sign decisions on suggestions | Proposed, for issues #6 and #7; [04 §4.3–4.8](04-roster-and-authority.md), [05 §5.2](05-catalogue-and-names.md), [02](02-guarantees.md) INV-Revoke apply it; the `Roster` and `Authority` models check it; holds the former ADR 0038 | Guillaume |

## 16.2 Where this specification departs from a source

| Topic | Source says | This specification | Why |
|---|---|---|---|
| Who may Force | proposal §2: any member | an admin, with `Pigeon-Force:` ([04 §4.6](04-roster-and-authority.md)) | ux.md, the later text, names it "admin override" |
| Conflict names | ux.md §8.9: `budget (Paul, 13 Oct 14:02).xlsx` | `budget (Paul, 2026-10-13 14h02).xlsx` ([05 §5.3](05-catalogue-and-names.md)) | `:` is not portable (Windows, macOS Finder); an English month makes names depend on the device's language (P5) |
| Headline order | ADR 0027, "worst first": Needs you, Not safe yet, Working, Safe, Not checked recently | Needs you > Not safe yet > Not checked recently > Working > Safe ([06 §6.5](06-status.md)) | *Not checked recently* must never read better than what it hides |
| Json module | ADR 0008: present in our `tla2tools.jar` | absent from the pinned TLC 1.7.4 ([13 §13.3](13-verification.md)) | checked on the jar; ADR 0026 already expects a re-pin |
| Custody mechanism | proposal: pigeon's own custody tokens (`CustodyTokens`) | git-annex's verified, lock-based drop (ADR 0001; [13 §13.2](13-verification.md)) | the token models stay as evidence about hand-offs and their counterexamples |
| Unauthorised edits | proposal: restored automatically from the catalogue | kept as the person's divergence and suggestion ([07 §7.6](07-capture-and-install.md)) | ADR 0034 |
| Go criteria | proposal §7 includes Windows | Windows criteria move to v1.1 ([13 §13.11](13-verification.md)) | ADR 0005 |
| Merging the roster | ADR 0009: a merge of roster changes needs an admin valid under both parents | the roster of a merge is the fold of its changes; no signer needed ([04 §4.8](04-roster-and-authority.md)) | ADR 0051 (Proposed), issue #6: concurrent self-retirements leave no such admin |
| Order of work | ADR 0037: protect, downloads, then repair | protect and repair are one class, fewest copies first, before downloads ([15 §15.2](15-resource-use.md)) | [08 §8.9](08-custody-loops.md), issue #13: a custodian must not spend its last space on a new key while an old one has a single copy |
| Counting copies | ADRs 0002, 0003: K distinct custodians | K distinct failure domains ([06 §6.2](06-status.md)) | ADR 0046, issue #12 |
| Reading trace logs | ADR 0008: `ndJsonDeserialize`, said to be in our `tla2tools.jar` | a TLA+ module generated from the NDJSON log, read by the pinned TLC 1.7.4 ([13 §13.3](13-verification.md)) | [13 §13.3](13-verification.md), issue #10 |
| First week of the feasibility study | proposal §7: the roles and the six daemon loops, then fault injection | the safety slice first: no proposals, purge or scrub until it passes; one person, one laptop, two custodians ([13 §13.12](13-verification.md)) | issue #17 **[no ADR: milestone]** |

Accepted ADRs are not rewritten; each line above becomes either an ADR that
confirms this specification or a correction of this specification.

## 16.3 Details decided without an ADR

Each is decided by this specification alone: it is cheap to reverse, or local
to one part of pigeon, so the [ADR policy](../adr/README.md#when-to-write-an-adr)
asks for none. One becomes an ADR if it turns out costly to reverse, or a
reviewer asks for one. Grouped by subject, so that a few ADRs could cover them
if that day comes.

### A. Repository layout ([03](03-repository-and-roles.md))

- Custodians hold a **bare** repository at `/var/lib/pigeon/<group-id>.git` (§3.1).
- Refs `refs/pigeon/peers/<device>/main` and `refs/pigeon/status/<device>` (§3.2).
- The settings check suspends drops and re-applies the expected settings (§3.5).
- Remote names `pigeon-` + 12 hex digits of the device ID (§3.6).
- The batch processes' grace period, and the argument checks of
  `pigeon_git::allow::check`, with the two `-c` settings it accepts and
  remotedaemon in a process group of its own (§3.7).

### B. Roster and authority ([04](04-roster-and-authority.md))

- The field names of `roster.toml` (§4.2).
- The longest-prefix folder rule, and add-only with the admins as stewards
  where no rule applies (§4.3).
- Drop records by any member about their own device (§4.3); decisions on
  suggestions are now ADR 0051.
- The proposal shape: one per capture batch and set of deciders, chained per
  path (§4.5).
- Force for admins only (§4.6, and §16.2).
- Concurrent edits of one roster table: the first side wins, the other becomes
  a proposal to the admins (§4.7).

### C. Catalogue and names ([05](05-catalogue-and-names.md))

- The conflict-name form (§5.3, and §16.2).
- Name length limits, 255 bytes per name and 1024 per path, and holding a
  non-portable name back at capture (§5.4).

### D. Status ([06](06-status.md))

- `offline_after` 24 h, `stale_after` 1 h, the report period 10 minutes (§6.2, §6.7).
- Signed status reports under `refs/pigeon/status/` and their content (§6.3).
- Naming the custodians that would hold a key through `find --want-get` (§6.4).
- The headline precedence (§6.5, and §16.2).

### E. Capture, install and custody loops ([07](07-capture-and-install.md), [08](08-custody-loops.md))

- The aside kept one hour under `.git/pigeon/aside/` (§7.5, [11 §11.3](11-local-state.md)).
- The first build's departures from this specification, each to close before
  the slice's scenarios run ([13 §13.12](13-verification.md)), and its extra
  files `peers.json`, `p2p-token`, `freed.json` and `dead.json` ([11 §11.3](11-local-state.md)).
- A save of a diverging path becomes a proposal of its own (§7.6).
- Fetching every 5 minutes besides change notices (§8.1).
- The deadline of a P2P fetch or push (§8.2).
- The Drop command `free --without-checking` and its record (§8.7).
- A full scrub pass every 30 days (§8.8).

### F. Network and joining ([09](09-network-and-joining.md))

- The invitation encoding `pgn1-` + base32 (§9.6).
- The inviter keeps each invitation's secret, which checking the proof needs
  (§9.6, a correction); the join exchange carries a custodian's fingerprints,
  host and direct addresses, and the welcome the inviter's git-annex UUID (§9.6).
- A custodian's default `maxsize`: free space minus 10 % (§9.6).
- The safety words (§9.6).
- Never merging new commits signed by a device absent from the current
  roster (§9.7).
- `pigeon leave` (§9.7).

### G. Interfaces ([10](10-interfaces.md))

- Force as `accept --override`, and the verbs `held`, `settings`, `leave`,
  `erase --cancel`, `invite --list | --cancel` (§10.1).
- At most one notification per kind and day (§10.4).
- Isolation marks around arguments, and an unknown message id rendered as
  itself (§10.5).
- Problem codes shown as `P-` and capitals (§10.6); the first build's codes
  `usage`, `unknown-computer`, `receiving`, `not-a-pigeon-repository`,
  `peer-unreachable`, `peer-refused` and `internal-error` (§10.6).
- The clock-skew warning at 5 minutes (§10.7).

### H. Local state, observability, verification, platforms ([11](11-local-state.md), [12](12-observability.md), [13](13-verification.md), [14](14-platforms-and-packaging.md))

- `toml_edit` for writing `config.toml`, and its `[network]` and `[loops]`
  tables (§11.2); invitation records kept 30 days (§11.3).
- Telemetry files bounded to 50 MB; `debug bundle` asks each peer's person;
  the `PIGEON_TRACE` switch (§12.2, §12.3).
- The E17 runner with a hard disk (§13.8); the first milestone's scope, its
  deferrals and `not-in-this-build` (§13.12).
- Minimum OS versions (§14.1); the file-system list (§14.2); the `MemoryHigh`
  value (§14.4); the Homebrew tap and release signing (§14.5); the crates
  `hmac`, `sha2`, `data-encoding`, `qrcode`, and the platform crates `rustix`
  and `libc` (§14.7); the per-platform minimum versions of git and git-annex
  (§14.3). The crate layout, edition and pinned toolchain are [14 §14.6](14-platforms-and-packaging.md)
  (Proposed).

### I. Resource use ([15](15-resource-use.md)), within ADR 0037

- The shaper's constants (§15.3); history exchange on metered networks (§15.3).
- Disk-busy thresholds and back-off timings (§15.4).
- The preset values (§15.6).
- The reference tree and the memory budget (§15.7).

### Rules that were Proposed ADRs

Withdrawn on 14 October 2026 at Guillaume's request, under the stricter
[ADR policy](../adr/README.md#when-to-write-an-adr): each is a safety
invariant whose alternative breaks a promise or loses data, a refinement of an
accepted ADR, the scope of v1, a scheduling policy, or tooling that is cheap to
change. Each stays a rule of this specification, checked by the models and
tests cited there; the stub of each ADR links to its full text.

| Former ADR | Rule | Where it lives | Why no ADR |
|---|---|---|---|
| 0039 | freeing and keeping never show a pointer | [08 §8.5](08-custody-loops.md) | invariant (P2), refines ADR 0031 |
| 0040 | install proves its own file; a racing save wins | [07 §7.5](07-capture-and-install.md) | invariant (P2), refines ADR 0034 |
| 0041 | a captured version stays immutable until protected | [07 §7.8](07-capture-and-install.md) | invariant (P1, P2) |
| 0042 | purge by references and seals | [08 §8.6](08-custody-loops.md) | refines ADR 0018 |
| 0043 | every device serves through a filter, pull only | [09 §9.4](09-network-and-joining.md), [03 §3.8](03-repository-and-roles.md) | refines ADR 0014 |
| 0044 | regular files and the execute bit are shared; the rest is listed | [07 §7.9](07-capture-and-install.md) | scope of v1 |
| 0045 | git reads only the configuration pigeon writes | [03 §3.7](03-repository-and-roles.md), [07 §7.9](07-capture-and-install.md) | invariant, refines ADR 0010 |
| 0050 | capacity budgets, reservations, endangered content first | [08 §8.9](08-custody-loops.md) | scheduling policy, refines ADRs 0003 and 0037 |
| 0052 | the catalogue from contributions and the conflicts they saw | [05 §5.2](05-catalogue-and-names.md) | invariant (P5), refines ADR 0009 |
| 0053 | trace logs reach TLC as generated modules | [13 §13.3](13-verification.md) | tooling, refines ADR 0008 |
| 0054 | a copy counts only if it can be locked | [09 §9.4](09-network-and-joining.md), [08 §8.3](08-custody-loops.md) | invariant (P1), refines ADR 0002 |
| 0058 | the workspace layout, the pure core, the pinned toolchain | [14 §14.6](14-platforms-and-packaging.md) | structure cheap to change now |

ADR 0049 (failure domains) is merged into ADR 0046, and ADR 0038 (decisions
on suggestions) into ADR 0051.

## 16.4 Design questions left open

- **Rename tracking.** A rename is a deletion and an add ([05 §5.2](05-catalogue-and-names.md)). If Paul
  renames *budget.xlsx* while Alice edits it, Alice's edit stays at the old
  name and Paul's file at the new one: both kept, but apart. Should capture
  record a rename, so that a concurrent edit follows the file? git's
  similarity detection guesses from content, and fails on small files.
- **Editing your own file in an add-only folder.** Martine adds *tarte.docx*
  to *Recettes* (allowed), then edits it: the edit becomes a suggestion to the
  stewards ([04 §4.3](04-roster-and-authority.md)). Should the author of a
  path's only version be allowed to modify it until someone else changes it?
- **A per-group `.pigeon/ignore`** besides the default list ([07 §7.2](07-capture-and-install.md)).
- **Links, empty folders, extended attributes, live libraries** in a later
  version ([07 §7.9](07-capture-and-install.md) leaves them out and lists them); a **package list**
  maintained from macOS's own type declarations rather than fixed in the core.
- **Names:** hold non-portable names (this specification) or map them
  on Windows only, as Samba's `catia` module does ([05 §5.4](05-catalogue-and-names.md)).
- **K per folder** (more copies for photos than for downloads); the roster
  has one `k` today.
- **Places from addresses** (ADR 0046): two backup machines behind one public
  address are often in one home. pigeon could suggest declaring a `site`
  then, without ever counting the guess.
- **Expiring declined suggestions** ([08 §8.9](08-custody-loops.md)): today an admin erases them;
  a group setting could do it after a delay, as a purge decided in advance.
- **Recovery** ([09 §9.9](09-network-and-joining.md), ADR 0047): split a
  recovery sheet among several admins (Shamir shares) rather than one sheet per
  admin; let refounding require the surviving custodians' owners rather than
  anyone who holds a repository; test ADR 0046's sync points under power loss
  with a fault-injecting block device, which the lab cannot do unprivileged.
- **Privacy across groups:** one device key per device makes the same device
  recognisable in two groups ([09 §9.1](09-network-and-joining.md)); a key per
  group would hide it at the cost of one endpoint per group.
- **The device key at rest** is not encrypted, so that `pigeond` starts
  unattended; a keychain (macOS) or the Secret Service (Linux) could hold it.
- **Phones** as read-only viewers; **installers** for non-technical people
  (ux.md §11).
- **Short invitation codes** with a PAKE (ADR 0017); a **placement planner**
  (ADR 0003); **per-file freeing** (ADR 0031); **native Windows** (ADR 0005, E11);
  **macOS backup machines** and **NAS packages** (ADR 0032).
- **Thresholds** of [06 §6.7](06-status.md), to be tuned by the usability test,
  and `pending_max` ([07 §7.8](07-capture-and-install.md)).
- **Saves from stale memory** ([07 §7.5](07-capture-and-install.md)). An application that still holds a
  file's old content in memory and saves after an install replaces the new
  version, which then lives on only in the history. Lock files cover office
  suites and editors. Should a save within seconds of an install count as
  concurrent?
- **Saves through a shared mapping on macOS** (E19). Such a write leaves
  the stat unchanged until `msync` (E19), so capture does not see the save
  until the application syncs it. An install keeps it in the aside, so it is
  not lost, but it is not captured earlier. Hashing kept files periodically
  would close the gap, at a cost in disk reads (ADR 0037).
- **A P2P fetch that never ends** (E12). In git-annex's relay
  (`runRelayService`, `P2P/IO.hs`) the exit of `git upload-pack` and its last
  output race for one `MVar`; when the exit wins, `CONNECTDONE` goes out, the
  pack is dropped and the client waits for it forever (E12; 2 of about 100
  fetches in a loop with a concurrent notice). It also sends `CONNECTDONE`
  twice. remotedaemon fetches inside its notice loop without a deadline
  (`P2PGeneric.hs`), so the peer's notices stop until it restarts. `pigeond`
  works around it (08 §8.2); to report upstream, with the fix of draining the
  output before `CONNECTDONE`.
- **Corruption that no scrub has found yet** ([09 §9.4](09-network-and-joining.md)). A copy that rots
  still counts until the next scrub reads it, at most the scrub period
  (08 §8.8, 30 days by default), so *protected* can overstate in that time.
  The repair epoch counts the rot from when it happens (02 §2.1), and a
  custodian's drop no longer leans on such a copy; a laptop's drop may. A
  shorter period narrows the window at the cost of reading every key more
  often (ADR 0037).
- **Erased names.** A purge erases content, but the names, authors and times of
  the erased versions stay in the signed history ([08 §8.6](08-custody-loops.md)). Removing a name
  would need a new history signed again by the admins. Is that ever needed?

## 16.5 Review issues

Guillaume's design review of commit `bb4fe07` opened issues #1–#17 on
GitHub. Each is closed once its resolution is in this specification, with an
ADR for each architecturally significant choice it needs; acceptance criteria that need code or
experiments stay listed in [13](13-verification.md) and in the prototype (#17).

| Issue | Topic | Where | State |
|---|---|---|---|
| #1 | install racing application saves | [07 §7.5](07-capture-and-install.md), [07 §7.6](07-capture-and-install.md) | [07 §7.5](07-capture-and-install.md), E19 asserts; the `CaptureInstall` model checks it |
| #2 | pointer files during free and keep | [08 §8.5](08-custody-loops.md) | [08 §8.5](08-custody-loops.md), E15b asserts |
| #3 | `annex.thin` and unprotected versions | [07 §7.8](07-capture-and-install.md), [06 §6.2](06-status.md), [03 §3.4](03-repository-and-roles.md) | [07 §7.8](07-capture-and-install.md), E18 asserts; copy-on-write measurement to come |
| #4 | reference-aware purge | [08 §8.6](08-custody-loops.md), [05 §5.1](05-catalogue-and-names.md) | [08 §8.6](08-custody-loops.md); E14 asserts the three cases; purge laws (hegel-rust) to come |
| #5 | purge against concurrent restores and returning peers | [08 §8.6](08-custody-loops.md), [06 §6.3](06-status.md) | [08 §8.6](08-custody-loops.md); the `Purge` model checks it; E14 asserts a restore against the seals and a returning laptop |
| #6 | roster forks with disjoint admin sets | [04 §4.4, §4.7, §4.8](04-roster-and-authority.md), [05 §5.2](05-catalogue-and-names.md) | ADR 0051 (Proposed); the `Roster` model checks it |
| #7 | decisions by non-admin owners and stewards | [04 §4.3, §4.5](04-roster-and-authority.md) | ADR 0051 (Proposed); the `Authority` model checks it |
| #8 | catalogue merge algorithm and its laws | [05 §5.1–5.3](05-catalogue-and-names.md), [04 §4.4](04-roster-and-authority.md) | [05 §5.2](05-catalogue-and-names.md); `models/catalogue` checks the laws on every schedule of three devices |
| #9 | git-annex cannot bypass pigeon | [03 §3.5–3.8](03-repository-and-roles.md), [09 §9.4](09-network-and-joining.md) | [09 §9.4](09-network-and-joining.md); E12 and E13 assert |
| #10 | models and coverage | [13 §13.2, §13.10](13-verification.md) | coverage map and liveness assumptions written; `AnnexDrop` model and E23 found the P2P drop race ([09 §9.4](09-network-and-joining.md)); the `Repair` model found a rotten copy proving a drop and a full custodian stalling domain repair (ADR 0046, [09 §9.4](09-network-and-joining.md)); the `Evict` model and E24 found a hide losing a write through an earlier handle, drops turning kept and staged files into pointers, and a keep showing a replaced version ([08 §8.5](08-custody-loops.md)); the `Authority` model found a removed laptop's commits still counting, and deciders unable to adopt (ADR 0051, Proposed) |
| #11 | recovery onto fresh machines | [09 §9.9](09-network-and-joining.md), [06 §6.2–6.3](06-status.md), [02 §2.1](02-guarantees.md), [04 §4.2–4.3](04-roster-and-authority.md), [09 §9.4](09-network-and-joining.md) | ADRs 0046 and 0047 (Proposed); E21 is the disaster drill |
| #12 | independent failure domains | [02 §2.1–2.5](02-guarantees.md), [04 §4.2, §4.7](04-roster-and-authority.md), [06 §6.2–6.5](06-status.md), [08 §8.3, §8.10](08-custody-loops.md) | ADR 0046 (Proposed); E22 asserts |
| #13 | capacity admission and repair priority | [08 §8.4, §8.9](08-custody-loops.md), [15 §15.2, §15.4](15-resource-use.md), [03 §3.4](03-repository-and-roles.md), [07 §7.8](07-capture-and-install.md) | [08 §8.9](08-custody-loops.md); E22 asserts |
| #14 | file-system object model | [07 §7.9](07-capture-and-install.md), [04 §4.4](04-roster-and-authority.md), [05 §5.2–5.4](05-catalogue-and-names.md), [03 §3.3–3.7](03-repository-and-roles.md), [14 §14.2](14-platforms-and-packaging.md) | [07 §7.9](07-capture-and-install.md), [03 §3.7](03-repository-and-roles.md); E20 asserts on Linux and macOS |
| #15 | replicated-format compatibility | [04 §4.2, §4.4](04-roster-and-authority.md), [06 §6.3](06-status.md), [08 §8.2](08-custody-loops.md), [09 §9.3](09-network-and-joining.md), [14 §14.3](14-platforms-and-packaging.md), [13 §13.4, §13.6](13-verification.md) | ADR 0048 (Proposed); fixtures and mixed releases with the prototype (#17) |
| #16 | one authoritative specification | this specification; [README](README.md) precedence | done |
| #17 | minimal end-to-end prototype | [13 §13.12](13-verification.md), [14 §14.6, §14.8](14-platforms-and-packaging.md) | [14 §14.6](14-platforms-and-packaging.md): the Rust workspace; [13 §13.3](13-verification.md): runs of the code checked by TLC; decided in the specification **[no ADR]**: scope, setup, scenarios S1–S11 and the ledger (13 §13.12), readings fixed by pigeon-core's laws (13 §13.13), the transport (09 §9.2); open until the slice passes and `docs/reports/slice-1.md` is published |
