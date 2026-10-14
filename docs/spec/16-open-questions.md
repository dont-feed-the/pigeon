# 16. Open questions

> Mission: list what is not decided: the ADRs still proposed, the places where
> this specification departs from a source, every detail it adds that no ADR
> records yet (each marked **[no ADR]** in the text), and the design questions
> left open. It keeps Guillaume's rule "an ADR for every choice" checkable:
> once this file is empty, every choice has its ADR.

## 16.1 Decisions awaited

| Item | State | Who |
|---|---|---|
| ADR 0038: deciders sign decisions on suggestions | Proposed, for issue #7; [04 §4.3, §4.5](04-roster-and-authority.md) apply it | Guillaume |
| ADR 0039: freeing and keeping never show a pointer | Proposed, for issue #2; [08 §8.5](08-custody-loops.md) applies it, E15b asserts it | Guillaume |
| ADR 0040: install proves its own file; racing saves win | Proposed, for issue #1; [07 §7.5](07-capture-and-install.md) applies it, E19 asserts it | Guillaume |
| ADR 0041: captured versions stay immutable until protected | Proposed, for issue #3; [07 §7.8](07-capture-and-install.md) applies it, E18 asserts it | Guillaume |
| ADR 0042: purge releases references, completes after every seal | Proposed, for issues #4 and #5; [08 §8.6](08-custody-loops.md) applies it, the `Purge` model checks it | Guillaume |
| ADR 0043: every device serves through a filter; pull only; explicit copy counts | Proposed, for issue #9; [09 §9.4](09-network-and-joining.md), [03 §3.8](03-repository-and-roles.md) apply it, E12 and E13 assert it | Guillaume |
| ADR 0044: regular files and the execute bit are shared; the rest is listed | Proposed, for issue #14; [07 §7.9](07-capture-and-install.md), [04 §4.4](04-roster-and-authority.md) rule 6, [05 §5.2–5.4](05-catalogue-and-names.md) apply it, E20 asserts it | Guillaume |
| ADR 0045: git and git-annex read only pigeon's configuration | Proposed, for issue #14; [03 §3.3–3.7](03-repository-and-roles.md) apply it, E20 asserts it | Guillaume |
| ADR 0046: protected needs the commit and the disk | Proposed, for issue #11; [06 §6.2–6.3](06-status.md), [09 §9.4](09-network-and-joining.md), [03 §3.4](03-repository-and-roles.md), [04 §4.2](04-roster-and-authority.md) apply it, E21 asserts it | Guillaume |
| ADR 0047: recovery by rejoin, recovery sheets, refounding, export | Proposed, for issue #11; [09 §9.9](09-network-and-joining.md), [04 §4.3](04-roster-and-authority.md), [10 §10.1](10-interfaces.md) apply it, E21 asserts it | Guillaume |
| ADR 0048: versioned formats and rules; *unknown* is never guessed | Proposed, for issue #15; [04 §4.2, §4.4](04-roster-and-authority.md), [06 §6.3](06-status.md), [09 §9.3](09-network-and-joining.md), [14 §14.3](14-platforms-and-packaging.md) apply it; fixtures and the compatibility law in [13](13-verification.md) | Guillaume |
| ADR 0049: copies count once per failure domain | Proposed, for issue #12; [02 §2.1–2.2](02-guarantees.md), [04 §4.2](04-roster-and-authority.md), [06 §6.2–6.3](06-status.md), [08 §8.3](08-custody-loops.md) apply it, E22 asserts it | Guillaume |
| ADR 0050: capacity budgeted, space reserved, fewest copies first | Proposed, for issue #13; [08 §8.4, §8.9](08-custody-loops.md), [15 §15.2](15-resource-use.md), [03 §3.4](03-repository-and-roles.md) apply it, E22 asserts it | Guillaume |
| ADR 0051: the roster is a fold of its changes; forks merge without an admin | Proposed, for issue #6; [04 §4.4, §4.7, §4.8](04-roster-and-authority.md), [05 §5.2](05-catalogue-and-names.md) apply it, the `Roster` model checks it | Guillaume |
| ADR 0052: the catalogue is built from contributions and the conflicts they saw | Proposed, for issue #8; [05 §5.1–5.3](05-catalogue-and-names.md), [04 §4.4–4.5](04-roster-and-authority.md), [02](02-guarantees.md) INV-Merge apply it, `models/catalogue` checks it ([13 §13.4](13-verification.md)) | Guillaume |
| ADR 0053: trace logs reach TLC as generated TLA+ modules; TLC stays at 1.7.4 | Proposed, for issue #10; [13 §13.3](13-verification.md) and [12 §12.3](12-observability.md) apply it; `TraceSample` and `scripts/trace-to-tla --check` run in CI | Guillaume |
| ADR 0054: a copy counts only if it can be locked; custodians answer `CHECKPRESENT` with a lock | Proposed, for issue #10: over P2P git-annex's verified drop counts a copy locked for removal (E23); [09 §9.4](09-network-and-joining.md) and [08 §8.3](08-custody-loops.md) apply it, `AnnexDrop` checks it ([13 §13.2](13-verification.md)); report upstream | Guillaume |
| ADR 0055: install recovery rolls forward from a journalled proof | Proposed, for issue #1; [07 §7.5–7.6](07-capture-and-install.md), [11 §11.3](11-local-state.md) apply it, the `CaptureInstall` model checks it ([13 §13.2](13-verification.md)) | Guillaume |
| ADR 0056: every asserting experiment runs on Linux and macOS | Proposed, for issue #10; `lab.yml` runs it; [07 §7.5](07-capture-and-install.md), [08 §8.2, §8.5](08-custody-loops.md), [13 §13.7–13.8](13-verification.md), [14 §14.3](14-platforms-and-packaging.md) apply it | Guillaume |
| ADR 0057: the first milestone is a safety slice | Proposed, for issue #17; [13 §13.12](13-verification.md) applies it | Guillaume |

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
| Order of work | ADR 0037: protect, downloads, then repair | protect and repair are one class, fewest copies first, before downloads ([15 §15.2](15-resource-use.md)) | ADR 0050 (Proposed), issue #13: a custodian must not spend its last space on a new key while an old one has a single copy |
| Counting copies | ADRs 0002, 0003: K distinct custodians | K distinct failure domains ([06 §6.2](06-status.md)) | ADR 0049 (Proposed), issue #12 |
| Reading trace logs | ADR 0008: `ndJsonDeserialize`, said to be in our `tla2tools.jar` | a TLA+ module generated from the NDJSON log, read by the pinned TLC 1.7.4 ([13 §13.3](13-verification.md)) | ADR 0053 (Proposed), issue #10 |
| First week of the feasibility study | proposal §7: the roles and the six daemon loops, then fault injection | the safety slice first: no proposals, purge or scrub until it passes; one person, one laptop, two custodians ([13 §13.12](13-verification.md)) | ADR 0057 (Proposed), issue #17 |

Accepted ADRs are not rewritten; each line above becomes either an ADR that
confirms this specification or a correction of this specification.

## 16.3 Details that need an ADR

Grouped as the ADRs that could record them, so that a few decisions cover
them all.

### A. Repository layout ([03](03-repository-and-roles.md))

- Custodians hold a **bare** repository at `/var/lib/pigeon/<group-id>.git` (§3.1).
- Refs `refs/pigeon/peers/<device>/main` and `refs/pigeon/status/<device>` (§3.2).
- The settings check suspends drops and re-applies the expected settings (§3.5).
- Remote names `pigeon-` + 12 hex digits of the device ID (§3.6).

### B. Roster and authority ([04](04-roster-and-authority.md))

- The field names of `roster.toml` (§4.2).
- The longest-prefix folder rule, and add-only with the admins as stewards
  where no rule applies (§4.3).
- Drop records by any member about their own device (§4.3); decisions on
  suggestions are now ADR 0038 (Proposed).
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
- A save of a diverging path becomes a proposal of its own (§7.6).
- Fetching every 5 minutes besides change notices (§8.1).
- The deadline of a P2P fetch or push (§8.2).
- The Drop command `free --without-checking` and its record (§8.7).
- A full scrub pass every 30 days (§8.8).

### F. Network and joining ([09](09-network-and-joining.md))

- Routing a stream by group ID, and the stream header (§9.2, §9.8).
- The ALPN names (§9.3).
- The invitation encoding `pgn1-` + base32 (§9.6).
- A custodian's default `maxsize`: free space minus 10 % (§9.6).
- The safety words (§9.6).
- Never merging new commits signed by a device absent from the current
  roster (§9.7).
- `pigeon leave` (§9.7).

### G. Interfaces ([10](10-interfaces.md))

- Force as `accept --override`, and the verbs `held`, `settings`, `leave`,
  `erase --cancel`, `invite --list | --cancel` (§10.1).
- At most one notification per kind and day (§10.4).
- Problem codes shown as `P-` and capitals (§10.6).
- The clock-skew warning at 5 minutes (§10.7).

### H. Local state, observability, platforms ([11](11-local-state.md), [12](12-observability.md), [14](14-platforms-and-packaging.md))

- State and cache directories (§11.2); `toml_edit` for writing `config.toml`
  (§11.2); invitation records kept 30 days (§11.3).
- Telemetry files bounded to 50 MB; `debug bundle` asks each peer's person;
  the `PIGEON_TRACE` switch (§12.2, §12.3).
- The E17 runner with a hard disk (§13.8).
- Minimum OS versions (§14.1); the file-system list (§14.2); the `MemoryHigh`
  value (§14.4); the Homebrew tap and release signing (§14.5); the crates
  `hmac`, `sha2`, `data-encoding`, `qrcode`, `rustix` (§14.7). The crate
  layout, edition and pinned toolchain went to ADR 0058 (Proposed).

### I. Resource use ([15](15-resource-use.md)), within ADR 0037

- The shaper's constants (§15.3); history exchange on metered networks (§15.3).
- Disk-busy thresholds and back-off timings (§15.4).
- The preset values (§15.6).
- The reference tree and the memory budget (§15.7).

## 16.4 Design questions left open

- **Rename tracking.** A rename is a deletion and an add (ADR 0052). If Paul
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
  version (ADR 0044 leaves them out and lists them); a **package list**
  maintained from macOS's own type declarations rather than fixed in the core.
- **Names:** hold non-portable names (this specification) or map them
  on Windows only, as Samba's `catia` module does ([05 §5.4](05-catalogue-and-names.md)).
- **K per folder** (more copies for photos than for downloads); the roster
  has one `k` today.
- **Places from addresses** (ADR 0049): two backup machines behind one public
  address are often in one home. pigeon could suggest declaring a `site`
  then, without ever counting the guess.
- **Expiring declined suggestions** (ADR 0050): today an admin erases them;
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
  and `pending_max` (ADR 0041).
- **Saves from stale memory** (ADR 0040). An application that still holds a
  file's old content in memory and saves after an install replaces the new
  version, which then lives on only in the history. Lock files cover office
  suites and editors. Should a save within seconds of an install count as
  concurrent?
- **Saves through a shared mapping on macOS** (ADR 0056). Such a write leaves
  the stat unchanged until `msync` (E19), so capture does not see the save
  until the application syncs it. An install keeps it in the aside, so it is
  not lost, but it is not captured earlier. Hashing kept files periodically
  would close the gap, at a cost in disk reads (ADR 0037).
- **A P2P fetch that never ends** (ADR 0056). In git-annex's relay
  (`runRelayService`, `P2P/IO.hs`) the exit of `git upload-pack` and its last
  output race for one `MVar`; when the exit wins, `CONNECTDONE` goes out, the
  pack is dropped and the client waits for it forever (E12; 2 of about 100
  fetches in a loop with a concurrent notice). It also sends `CONNECTDONE`
  twice. remotedaemon fetches inside its notice loop without a deadline
  (`P2PGeneric.hs`), so the peer's notices stop until it restarts. `pigeond`
  works around it (08 §8.2); to report upstream, with the fix of draining the
  output before `CONNECTDONE`.
- **Erased names.** A purge erases content, but the names, authors and times of
  the erased versions stay in the signed history (ADR 0042). Removing a name
  would need a new history signed again by the admins. Is that ever needed?

## 16.5 Review issues

Guillaume's design review of commit `bb4fe07` opened issues #1–#17 on
GitHub. Each is closed once its resolution is in this specification, with an
ADR for every choice it needs; acceptance criteria that need code or
experiments stay listed in [13](13-verification.md) and in the prototype (#17).

| Issue | Topic | Where | State |
|---|---|---|---|
| #1 | install racing application saves | [07 §7.5](07-capture-and-install.md), [07 §7.6](07-capture-and-install.md) | ADR 0040 (Proposed), E19 asserts; `CaptureInstall` model to come (#10) |
| #2 | pointer files during free and keep | [08 §8.5](08-custody-loops.md) | ADR 0039 (Proposed), E15b asserts |
| #3 | `annex.thin` and unprotected versions | [07 §7.8](07-capture-and-install.md), [06 §6.2](06-status.md), [03 §3.4](03-repository-and-roles.md) | ADR 0041 (Proposed), E18 asserts; copy-on-write measurement to come |
| #4 | reference-aware purge | [08 §8.6](08-custody-loops.md), [05 §5.1](05-catalogue-and-names.md) | ADR 0042 (Proposed); E14 asserts the three cases; purge laws (hegel-rust) to come |
| #5 | purge against concurrent restores and returning peers | [08 §8.6](08-custody-loops.md), [06 §6.3](06-status.md) | ADR 0042 (Proposed); the `Purge` model checks it; E14 asserts a restore against the seals and a returning laptop |
| #6 | roster forks with disjoint admin sets | [04 §4.4, §4.7, §4.8](04-roster-and-authority.md), [05 §5.2](05-catalogue-and-names.md) | ADR 0051 (Proposed); the `Roster` model checks it |
| #7 | decisions by non-admin owners and stewards | [04 §4.3, §4.5](04-roster-and-authority.md) | ADR 0038 (Proposed) |
| #8 | catalogue merge algorithm and its laws | [05 §5.1–5.3](05-catalogue-and-names.md), [04 §4.4](04-roster-and-authority.md) | ADR 0052 (Proposed); `models/catalogue` checks the laws on every schedule of three devices |
| #9 | git-annex cannot bypass pigeon | [03 §3.5–3.8](03-repository-and-roles.md), [09 §9.4](09-network-and-joining.md) | ADR 0043 (Proposed); E12 and E13 assert |
| #10 | models and coverage | [13 §13.2, §13.10](13-verification.md) | coverage map and liveness assumptions written; `AnnexDrop` model and E23 found the P2P drop race (ADR 0054, Proposed) |
| #11 | recovery onto fresh machines | [09 §9.9](09-network-and-joining.md), [06 §6.2–6.3](06-status.md), [02 §2.1](02-guarantees.md), [04 §4.2–4.3](04-roster-and-authority.md), [09 §9.4](09-network-and-joining.md) | ADRs 0046 and 0047 (Proposed); E21 is the disaster drill |
| #12 | independent failure domains | [02 §2.1–2.5](02-guarantees.md), [04 §4.2, §4.7](04-roster-and-authority.md), [06 §6.2–6.5](06-status.md), [08 §8.3, §8.10](08-custody-loops.md) | ADR 0049 (Proposed); E22 asserts |
| #13 | capacity admission and repair priority | [08 §8.4, §8.9](08-custody-loops.md), [15 §15.2, §15.4](15-resource-use.md), [03 §3.4](03-repository-and-roles.md), [07 §7.8](07-capture-and-install.md) | ADR 0050 (Proposed); E22 asserts |
| #14 | file-system object model | [07 §7.9](07-capture-and-install.md), [04 §4.4](04-roster-and-authority.md), [05 §5.2–5.4](05-catalogue-and-names.md), [03 §3.3–3.7](03-repository-and-roles.md), [14 §14.2](14-platforms-and-packaging.md) | ADRs 0044 and 0045 (Proposed); E20 asserts on Linux and macOS |
| #15 | replicated-format compatibility | [04 §4.2, §4.4](04-roster-and-authority.md), [06 §6.3](06-status.md), [08 §8.2](08-custody-loops.md), [09 §9.3](09-network-and-joining.md), [14 §14.3](14-platforms-and-packaging.md), [13 §13.4, §13.6](13-verification.md) | ADR 0048 (Proposed); fixtures and mixed releases with the prototype (#17) |
| #16 | one authoritative specification | this specification; [README](README.md) precedence | done |
| #17 | minimal end-to-end prototype | [13 §13.12](13-verification.md), [14 §14.6, §14.8](14-platforms-and-packaging.md) | ADR 0057 (Proposed): scope, setup, scenarios S1–S11 and the ledger; ADR 0058 (Proposed): the Rust workspace; open until the slice passes and `docs/reports/slice-1.md` is published |
