# 16. Open questions

> Mission: list what is not decided: the ADRs still proposed, the places where
> this specification departs from a source, every detail it adds that no ADR
> records yet (each marked **[no ADR]** in the text), and the design questions
> left open. It keeps Guillaume's rule "an ADR for every choice" checkable:
> once this file is empty, every choice has its ADR.

## 16.1 Decisions awaited

| Item | State | Who |
|---|---|---|
| ADR 0004: Rust, driving the git-annex CLI | Proposed; the specification assumes it ([03 §3.7](03-repository-and-roles.md), [14](14-platforms-and-packaging.md)) | Guillaume or Mario |
| ADR 0037: background work stays out of the way | Proposed; [15](15-resource-use.md) details it | Guillaume |
| ADR 0038: deciders sign decisions on suggestions | Proposed, for issue #7; [04 §4.3, §4.5](04-roster-and-authority.md) apply it | Guillaume |
| ADR 0039: freeing and keeping never show a pointer | Proposed, for issue #2; [08 §8.5](08-custody-loops.md) applies it, E15b asserts it | Guillaume |
| TLC with the `Json` module for trace validation | ADR 0026 foresees a new ADR pinning it by SHA-256 ([13 §13.3](13-verification.md)) | before the first `*Trace.tla` |

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

Accepted ADRs are not rewritten; each line above becomes either an ADR that
confirms this specification or a correction of this specification.

## 16.3 Details that need an ADR

Grouped as the ADRs that could record them, so that a few decisions cover
them all.

### A. Repository layout ([03](03-repository-and-roles.md))

- Custodians hold a **bare** repository at `/var/lib/pigeon/<group-id>.git` (§3.1).
- Refs `refs/pigeon/peers/<device>/main` and `refs/pigeon/status/<device>` (§3.2).
- Empty folders, symbolic links, permissions other than the executable bit
  and extended attributes are not versioned in v1 (§3.3).
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

- No merge commit for a tip that brings only merge commits (§5.2).
- Roster conflicts in a merge (§5.2, with B).
- The conflict-name form (§5.3, and §16.2).
- Name length limits, 255 bytes per name and 1024 per path, and holding a
  non-portable name back at capture (§5.4).

### D. Status ([06](06-status.md))

- `offline_after` 24 h, `stale_after` 1 h, the report period 10 minutes (§6.2, §6.7).
- Signed status reports under `refs/pigeon/status/` and their content (§6.3).
- Naming the custodians that would hold a key through `find --want-get` (§6.4).
- The headline precedence (§6.5, and §16.2).

### E. Capture, install and custody loops ([07](07-capture-and-install.md), [08](08-custody-loops.md))

- The default ignore list (§7.2).
- The aside kept one hour under `.git/pigeon/aside/` (§7.5, [11 §11.3](11-local-state.md)).
- A save of a diverging path becomes a proposal of its own (§7.6).
- Fetching every 5 minutes besides change notices (§8.1).
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
  value (§14.4); the Homebrew tap and release signing (§14.5); the crate
  layout (§14.6); the crates `hmac`, `sha2`, `data-encoding`, `qrcode`,
  `rustix` (§14.7); edition 2024 and the pinned toolchain (§14.8).

### I. Resource use ([15](15-resource-use.md)), once ADR 0037 is accepted

- The shaper's constants (§15.3); history exchange on metered networks (§15.3).
- Disk-busy thresholds and back-off timings (§15.4).
- The preset values (§15.6).
- The reference tree and the memory budget (§15.7).

## 16.4 Design questions left open

- **Editing your own file in an add-only folder.** Martine adds *tarte.docx*
  to *Recettes* (allowed), then edits it: the edit becomes a suggestion to the
  stewards ([04 §4.3](04-roster-and-authority.md)). Should the author of a
  path's only version be allowed to modify it until someone else changes it?
- **A per-group `.pigeon/ignore`** besides the default list ([07 §7.2](07-capture-and-install.md)).
- **Empty folders, symbolic links, extended attributes** in a later version.
- **Names:** hold non-portable names (this specification) or map them
  on Windows only, as Samba's `catia` module does ([05 §5.4](05-catalogue-and-names.md)).
- **K per folder** (more copies for photos than for downloads); the roster
  has one `k` today.
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
- **Thresholds** of [06 §6.7](06-status.md), to be tuned by the usability test.

## 16.5 Review issues

Guillaume's design review of commit `bb4fe07` opened issues #1–#17 on
GitHub. Each is closed once its resolution is in this specification, with an
ADR for every choice it needs; acceptance criteria that need code or
experiments stay listed in [13](13-verification.md) and in the prototype (#17).

| Issue | Topic | Where | State |
|---|---|---|---|
| #1 | install racing application saves | [07 §7.5](07-capture-and-install.md) | journal and aside specified; ADR to come |
| #2 | pointer files during free and keep | [08 §8.5](08-custody-loops.md) | ADR 0039 (Proposed), E15b asserts |
| #3 | `annex.thin` and unprotected versions | [02](02-guarantees.md), [03 §3.4](03-repository-and-roles.md) | open: needs a decision |
| #4 | reference-aware purge | [08 §8.6](08-custody-loops.md) | open |
| #5 | purge against concurrent restores and returning peers | [08 §8.6](08-custody-loops.md) | open |
| #6 | roster forks with disjoint admin sets | [04 §4.7](04-roster-and-authority.md) | open |
| #7 | decisions by non-admin owners and stewards | [04 §4.3, §4.5](04-roster-and-authority.md) | ADR 0038 (Proposed) |
| #8 | catalogue merge algorithm and its laws | [05 §5.2](05-catalogue-and-names.md) | specified; ADR to come |
| #9 | git-annex cannot bypass pigeon | [03 §3.7](03-repository-and-roles.md), [09 §9.4](09-network-and-joining.md) | E12, E13 to run |
| #10 | models and coverage | [13 §13.2, §13.10](13-verification.md) | coverage map and liveness assumptions written |
| #11 | recovery onto fresh machines | — | open |
| #12 | independent failure domains | — | open |
| #13 | capacity admission and repair priority | [08 §8.9](08-custody-loops.md), [15 §15.2](15-resource-use.md) | open |
| #14 | file-system object model | [03 §3.3](03-repository-and-roles.md), [14 §14.2](14-platforms-and-packaging.md) | open |
| #15 | replicated-format compatibility | — | open |
| #16 | one authoritative specification | this specification; [README](README.md) precedence | done |
| #17 | minimal end-to-end prototype | — | after the design issues |
