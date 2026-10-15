# 16. Open questions

> Mission: what is not decided: departures from accepted ADRs, details
> decided without an ADR (marked **[no ADR]** in the text), and open design
> questions. Under the [ADR policy](../adr/README.md#when-to-write-an-adr) a
> **[no ADR]** detail is settled here; it becomes an ADR only if it turns
> out costly to reverse, or a reviewer asks for one.

## 16.1 Decisions awaited

No ADR is Proposed: Guillaume accepted ADRs 0046, 0047, 0048 and 0051 on
14 October 2026. Rules that were Proposed ADRs before that day are listed in
§16.3.

## 16.2 Where this specification departs from a source

Accepted ADRs come first ([README](README.md)) and are never rewritten:
each departure waits for a new ADR that confirms this specification, or
for a correction of it. Departures from ux.md and the proposal need nothing.

| Topic | Accepted ADR says | This specification | Why |
|---|---|---|---|
| Headline order | ADR 0027, "worst first": Needs you, Not safe yet, Working, Safe, Not checked recently | Needs you > Not safe yet > Not checked recently > Working > Safe ([06 §6.5](06-status.md)) | *Not checked recently* must never read better than what it hides |
| Reading trace logs | ADR 0008: the Json module and `ndJsonDeserialize`, said to be in our `tla2tools.jar` | absent from the pinned TLC 1.7.4; a TLA+ module generated from the log ([13 §13.3](13-verification.md)) | checked on the jar; ADR 0026 expects a re-pin; issue #10 |
| Order of work | ADR 0037: protect, downloads, then repair | protect and repair are one class, fewest copies first, before downloads (`queue`, [15 §15.2](15-resource-use.md)) | issue #13: a custodian must not spend its last space on a new key while an old one has a single copy |
| Released keys | ADR 0018: `pigeond` marks them with git-annex metadata (`pigeon-released=<decision>`) that custodians' preferred content excludes | release is a pure function, never stored, with no git-annex metadata ([08 §8.6](08-custody-loops.md)) | per-key transfers never fetch a released key (E14); to confirm by an ADR or correct |

ADR 0009's roster merge rule is superseded by ADR 0051 (`Roster_adr0009`).

## 16.3 Details decided without an ADR

Each is cheap to reverse or local to one part of pigeon, so the ADR policy
asks for none. Grouped by file, so that a few ADRs could cover them.

| File | Decided here |
|---|---|
| [03](03-repository-and-roles.md) | custodians' **bare** repository `/var/lib/pigeon/<group-id>.git` (§3.1); refs `refs/pigeon/peers/<device>/main`, `refs/pigeon/status/<device>` (§3.2) |
| 03 | the settings check suspends drops and re-applies the settings (§3.5); remote names `pigeon-` + 12 hex digits of the device ID (§3.6) |
| 03 | the batch processes' grace period; `pigeon_git::allow::check` with its two accepted `-c` settings; remotedaemon in its own process group (§3.7) |
| [04](04-roster-and-authority.md) | the field names of `roster.toml` (§4.2); the longest-prefix folder rule, add-only with the admins as stewards where none applies (§4.3) |
| 04 | drop records by any member about their own device (§4.3; decisions on suggestions are ADR 0051) |
| 04 | one proposal per capture batch and set of deciders, chained per path (§4.5); force for admins only (§4.6, §16.2) |
| 04 | concurrent edits of one roster table: the first side wins, the other becomes a proposal to the admins (§4.7) |
| [05](05-catalogue-and-names.md) | the conflict-name form (§5.3); limits of 255 bytes per name and 1024 per path, and holding a non-portable name back at capture (§5.4) |
| [06](06-status.md) | `offline_after` 24 h, `stale_after` 1 h, a report every 10 minutes (§6.2, §6.7); signed reports under `refs/pigeon/status/` (§6.3) |
| 06 | naming the custodians that would hold a key through `find --want-get` (§6.4); the headline precedence (§6.5, §16.2) |
| [07](07-capture-and-install.md) | the aside kept until the first full scan 10 minutes after `done` (§7.5, [11 §11.3](11-local-state.md)); a save of a diverging path is a proposal of its own (§7.6) |
| 07 | the first build's departures, each to close before the slice's scenarios run ([13 §13.12](13-verification.md)), and its files `peers.json`, `p2p-token`, `freed.json`, `dead.json` (11 §11.3) |
| 07, 11 | durable `kept` and `preserved` journal records; non-replacing recovery names and capture manifests retained for manual recovery (07 §7.5, 11 §11.3) |
| [08](08-custody-loops.md) | fetching every 5 minutes besides change notices; repair in the protect class of the queue (§8.1, [15 §15.2](15-resource-use.md), issue #13) |
| 08 | the deadline of a P2P fetch or push (§8.2); `free --without-checking` and its record (§8.7); a full scrub every 30 days (§8.8) |
| [09](09-network-and-joining.md) | the invitation encoding `pgn1-` + base32; the inviter keeps each secret, which checking the proof needs (a correction) (§9.6) |
| 09 | the join exchange carries a custodian's direct addresses, the welcome the inviter's git-annex UUID; default `maxsize` free space minus 10 %; the safety words (§9.6); `pigeon leave` (§9.7) |
| [10](10-interfaces.md) | force as `accept --override`; the verbs `held`, `settings`, `leave`, `erase --cancel`, `invite --list \| --cancel` (§10.1); one notification per kind and day (§10.4) |
| 10 | isolation marks around arguments, an unknown message id rendered as itself (§10.5); the clock-skew warning at 5 minutes (§10.7) |
| 10 | problem codes as `P-` and capitals; the first build's `usage`, `unknown-computer`, `receiving`, `not-a-pigeon-repository`, `peer-unreachable`, `peer-refused`, `internal-error` (§10.6) |
| [11](11-local-state.md) | `toml_edit` for writing `config.toml`, its `[network]` and `[loops]` tables (§11.2); invitation records kept 30 days (§11.3) |
| [12](12-observability.md) | telemetry bounded to 50 MB; `debug bundle` asks each peer's person; the `PIGEON_TRACE` switch (§12.2, §12.3) |
| [13](13-verification.md) | the E17 runner with a hard disk (§13.8); the first milestone's scope, deferrals and `not-in-this-build` (§13.12) |
| 13 | the first build's port kept in `<state>/port`, direct addresses in a stream's header and a join's welcome, re-thinning by hard link on every file system, of a file unchanged for a minute and hashed before it is moved aside, `disk-full` when free space is below `annex.diskreserve`; a backup machine writes and signs a recovery's roster commit itself, where ADR 0047 has the sheet key sign (§13.12) |
| 13 | mandatory storage-boundary observations, their corrupted variants and missing-coverage checks; these do not claim full filesystem-driver refinement (§13.3) |
| [14](14-platforms-and-packaging.md) | minimum OS versions (§14.1); the file-system list (§14.2); minimum git and git-annex per platform, and the platforms pigeon's git-annex is built for (§14.3); the `MemoryHigh` value (§14.4) |
| 14 | the Homebrew tap and release signing (§14.5); the crate layout, edition and pinned toolchain (§14.6); `hmac`, `sha2`, `data-encoding`, `qrcode`, `rustix`, `libc` (§14.7) |
| [15](15-resource-use.md), within ADR 0037 | the shaper's constants and history exchange on metered networks (§15.3); disk-busy thresholds and back-off (§15.4); presets (§15.6); the reference tree and memory budget (§15.7) |

### Rules that were Proposed ADRs

Withdrawn on 14 October 2026 at Guillaume's request, under the stricter ADR
policy. Each stays a rule of this specification, checked by the models and
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
| 0055 | install recovery rolls forward from a journalled proof | [07 §7.5](07-capture-and-install.md), [11 §11.3](11-local-state.md) | invariant (P2), refines ADR 0036 |
| 0056 | every experiment runs on Linux and macOS | [13 §13.7, §13.8](13-verification.md) | tooling |
| 0057 | the first milestone is a safety slice | [13 §13.12](13-verification.md) | order of work |
| 0060 | the platform layer uses `rustix` and `libproc` | [14 §14.7](14-platforms-and-packaging.md) | crate choice |
| 0061 | freeing hides as an install and guards every drop | [08 §8.5](08-custody-loops.md) | invariant (P2), refines ADR 0031 |
| 0064 | pigeon-core fixes the readings its laws need | [13 §13.13](13-verification.md) | readings, fixed by laws |
| 0066 | the helper names its repository; the filter supplies the token | [09 §9.2](09-network-and-joining.md) | interface detail |
| 0067 | the git driver checks every command and bounds every call | [03 §3.7](03-repository-and-roles.md) | refines ADR 0010 |
| 0068 | messages are listed with their variables and checked in every bundle | [10 §10.5](10-interfaces.md) | tooling, refines ADR 0028 |

Merged: 0049 (failure domains) into ADR 0046, then dropped with failure
domains; 0038, 0062 and 0063 (decisions, revocation) into ADR 0051; 0059
into 0054; 0065 into 0053.

## 16.4 Design questions left open

- **Scope:** a per-group `.pigeon/ignore` ([07 §7.2](07-capture-and-install.md));
  links, empty folders, extended attributes, live libraries, and packages
  from macOS's type declarations (§7.9); mapping non-portable names on
  Windows only, as Samba's `catia` does ([05 §5.4](05-catalogue-and-names.md));
  K per folder.
- **Expiring declined suggestions** ([08 §8.9](08-custody-loops.md)) by a
  group setting, as a purge decided in advance.
- **Recovery** (ADR 0047, [09 §9.9](09-network-and-joining.md)): Shamir
  shares of a sheet among admins; refounding only with the surviving
  custodians' owners; ADR 0046's sync points under power loss, tested with a
  fault-injecting block device.
- **Keys:** one key per device makes a device recognisable in two groups
  ([09 §9.1](09-network-and-joining.md)), a key per group costs an endpoint
  per group; the device key could rest in the macOS keychain or the Secret
  Service.
- **Later:** phones as viewers; installers (ux.md §11); short invitation
  codes with a PAKE (ADR 0017); a placement planner (ADR 0003); per-file
  freeing (ADR 0031); native Windows (ADR 0005, E11); macOS backup machines
  and NAS packages (ADR 0032).
- **Thresholds** of [06 §6.7](06-status.md), from the usability test, and
  `pending_max` ([07 §7.8](07-capture-and-install.md)).
- **Shared mappings on macOS** change the stat only at `msync` (E19), so
  capture sees them late; the kept aside covers them. Hashing kept files
  periodically would close the gap, at a cost in reads.
- **A P2P fetch that never ends** (E12, E25): git-annex's relay
  (`runRelayService`, `P2P/IO.hs`) can send `CONNECTDONE` before the pack,
  and remotedaemon fetches without a deadline. pigeon's git-annex drains
  the output first (patch 0002, ADR 0069); stock builds still hang, so
  `pigeond` keeps its workaround ([08 §8.2](08-custody-loops.md)). To
  report upstream.
- **Unfound corruption** ([09 §9.4](09-network-and-joining.md)): a rotten
  copy counts until the next scrub, at most 30 days (08 §8.8). A custodian's
  drop no longer leans on it; a laptop's may. A shorter period costs reads
  (ADR 0037).
- **Erased names** stay in the signed history (08 §8.6). Is a re-signed
  history ever needed?

## 16.5 Review issues

Guillaume's review of commit `bb4fe07` opened issues #1–#17. Their
resolutions are in the sections they name and in ADRs 0046–0048 and 0051;
#12 (failure domains) was closed without a model by ADR 0046. What remains
open:

| Issue | Open part | Where |
|---|---|---|
| #3 | the copy-on-write cost of pending history on APFS and btrfs | [07 §7.8](07-capture-and-install.md), E18 |
| #4, #5 | purge laws; `Purge` trace validation | [08 §8.6](08-custody-loops.md), §13.4 |
| #10 | the laws and trace validations still planned | [13 §13.10](13-verification.md) |
| #15 | format fixtures and mixed releases, with the prototype | [13 §13.6](13-verification.md), ADR 0048 |
| #17 | the slice passing, and `docs/reports/slice-1.md` | [13 §13.12](13-verification.md) |
