# 10. Interfaces

> Mission: what people and scripts touch: the shared request API, the CLI,
> the local web interface, notifications, messages and problems. Words are
> ux.md's; operations are the other files'.

## 10.1 One request API (ADR 0021)

Every operation is one **request type**, defined once in `pigeon-proto` and
carried over the local socket ([09 §9.3](09-network-and-joining.md);
`requests_round_trip`, `responses_round_trip`). The CLI and the web render
the same requests, so no operation exists in only one. A response is a
result, progress events then a result, or a `Problem` (§10.6).

| Request | CLI | Web | Specified in |
|---|---|---|---|
| `Status {path?, not_shared}` | `pigeon status [PATH] [--not-shared]` | Home; the folder view's *Not shared* list | [06](06-status.md), [07 §7.9](07-capture-and-install.md) |
| `Create {name, folder, k, device_name}` | `pigeon create NAME` | Welcome → Create | [04 §4.1](04-roster-and-authority.md) |
| `Invite {person, admin, custodian, name?}` | `pigeon invite [--admin] PERSON`, `pigeon invite --backup-machine [NAME]` | People → Invite someone | [09 §9.6](09-network-and-joining.md) |
| `Join {invitation, folder, keep, name?}` | `pigeon join INVITATION [FOLDER] [--name NAME]` | Welcome → Join | [09 §9.6](09-network-and-joining.md) |
| `Keep {folder}` · `Evict {folder}` | `pigeon keep PATH` · `pigeon free PATH` | Files → *Keep on this computer* | [08 §8.5](08-custody-loops.md) |
| `Drop {path}` | `pigeon free --without-checking PATH` | — (CLI only, ux.md §2.2) | [08 §8.7](08-custody-loops.md) |
| `History {path}` | `pigeon history PATH` | a file's page | [05](05-catalogue-and-names.md) |
| `View {path, version \| group}` | `pigeon view PATH [--version N \| --group]` | *Open a copy*, *Open Paul's version* | [07 §7.6](07-capture-and-install.md) |
| `Restore {path, version \| at}` | `pigeon restore PATH [--version N \| --at TIME]` | *Restore this version*, Deleted files | [05 §5.1](05-catalogue-and-names.md) |
| `Deleted {path?}` | `pigeon deleted [PATH]` | Files → Deleted files | [05 §5.1](05-catalogue-and-names.md) |
| `Proposals` · `Accept {proposal, override?}` · `Reject {proposal, note?}` | `pigeon suggestions` · `pigeon accept ID [--override REASON]` · `pigeon decline ID [--note TEXT]` | Suggestions | [04 §4.5, §4.6](04-roster-and-authority.md) |
| `Conflicts` · `Resolve {path, choice}` | `pigeon conflicts` · `pigeon resolve PATH --take-group \| --keep-copy DEST \| --suggest-again` | Suggestions → Yours, Home | [07 §7.6](07-capture-and-install.md) |
| `People` · `Machines` | `pigeon people` · `pigeon machines` | People & machines | [04 §4.2](04-roster-and-authority.md) |
| `RemoveDevice {name}` · `Leave` | `pigeon remove-computer NAME` · `pigeon leave` | *I lost a computer* · Settings | [09 §9.7](09-network-and-joining.md) |
| `RecoverySheet` · `Recover {sheet, folder, from?}` | `pigeon recovery-sheet` · `pigeon recover SHEET [FOLDER] [--from DEVICE]` | People → *Print a recovery sheet* · Welcome → *Recover a group* | [09 §9.9](09-network-and-joining.md) |
| `Refound {repository?}` | `pigeon refound [REPOSITORY]` | — (CLI only) | [09 §9.9](09-network-and-joining.md) |
| — | `pigeon export --to DIR [--at DATE\|COMMIT] [--deleted] [REPOSITORY…]` | — (without `pigeond`) | [09 §9.9](09-network-and-joining.md) |
| `Purge {path}` · `CancelPurge {id}` | `pigeon erase PATH` · `pigeon erase --cancel ID` | *Erase forever* | [08 §8.6](08-custody-loops.md) |
| `Held` · `ReleaseHeld {batch, choice}` | `pigeon held` · `pigeon held ID --share \| --bring-back \| --rename` | Home → Needs you | [07 §7.4](07-capture-and-install.md) |
| `Pause {duration?}` · `Resume` | `pigeon pause [DURATION]` · `pigeon resume` | Settings | [15](15-resource-use.md) |
| `GetSettings` · `SetSettings {…}` | `pigeon settings [KEY [VALUE]]` | Settings | [11 §11.2](11-local-state.md) |
| `Doctor {details}` | `pigeon doctor [--details]` | Settings → Advanced | §10.7 |
| `DebugBundle {peers}` | `pigeon debug bundle [--from NAME…]` | Settings → Advanced | [12](12-observability.md) |
| `Folders` | `pigeon status` (by file) | Files | [08 §8.5](08-custody-loops.md) |
| `Files {folder}` | `pigeon status FOLDER` | Files → a folder | [06 §6.2](06-status.md) |
| `Deleted` | `pigeon restore PATH` | Files → Deleted files | [10 §10.3](10-interfaces.md) |
| `Subscribe` | — | live updates (SSE) | §10.3 |

- Request names are internal (ADR 0024: `evict`, `purge`, `reject`), CLI
  verbs ux.md's (`free`, `erase`, `decline`); renaming either is an
  interface change. `Accept {override}` is Force, its reason the
  `Pigeon-Force:` trailer ([04 §4.6](04-roster-and-authority.md)) **[no ADR: verb]**.
- `pigeon held`, `settings`, `leave`, `erase --cancel` and `invite --list |
  --cancel ID` complete ux.md §7 **[no ADR: verbs]**.
- `pigeond` answers `Hello` with its version; a CLI of another version
  refuses with `version-mismatch`.

## 10.2 The command line (ADR 0024)

- One subcommand per request; completions and man pages from the same
  definitions. Result on stdout; messages and progress on stderr, headline
  then detail (ux.md §7); colour and bars only on a terminal, never alone.
- **`--json`** on every command: the response on stdout, problems on stderr.
  Scripts rely on `--json` and problem codes, never on wording.
- **Questions.** `erase`, `accept --override`, `refound`, `recover`, `free
  --without-checking`, `remove-computer`, `leave` and a held mass deletion
  ask first; without a terminal they need `--yes`; `--no-input` fails with
  `needs-confirmation`.
- **Exit codes:** 0 success, 1 a `Problem`, 2 usage. `pigeon status` exits 0
  whatever the headline; scripts read `"level": "needs-you" | "not-safe" |
  "not-checked" | "working" | "safe"`.
- The CLI holds no logic; without `pigeond` it points to `pigeon doctor`.
  Only `pigeon export` reads repositories itself, read-only, with the same
  core code (ADR 0047).

## 10.3 The web interface (ADR 0021, ux.md §6)

Coded so far (`crates/pigeon-web`, `pigeond/src/web.rs`): the binding, the
login token and cookie, the `Host`/`Origin` check, Home, Suggestions and
Machines pages read through the same handler as the socket, and an SSE
headline; Files lists the top-level folders (`Folders`) with *Keep on this
computer* or *Free up space*; Settings changes `language` and
`notifications` (`GetSettings`, `SetSettings`, also `pigeon settings`), a
new language showing at once; People & machines invites a person or a
backup machine and removes a machine, asking again first; *Accept*,
*Decline*, keep, free, *Save*, *Invite* and *Remove* are `POST`s carrying
a per-session form token. A folder's page lists its files (`Files`), a file's page
its versions (`History`) with *Restore this version* (`Restore`);
Deleted files lists what `main` deleted (`Deleted`), each with *Restore*.
Settings also pauses for an hour, a day or until *Share again* (`Pause`,
`Resume`, also `pigeon pause` and `pigeon resume`, 15 §15.6).
People & machines lists the people first (`People`, also `pigeon people`);
`pigeon deleted` lists Deleted files.
`pigeon invite --list` and `--cancel ID` (`Invitations`, `CancelInvitation`)
show and withdraw the invitations not yet used.
Home lists the held deletions (`Held`) with *Delete for everyone*, asked
again, and *Bring them back* (`ReleaseHeld`); Settings has *Leave the
group*, asked again (`Leave`).
Not yet: the other settings, the other actions, axe-core.

- Served by `pigeond`, rendered on the server; server-sent events refresh
  the headline and activity; pages work without JavaScript. It writes
  nothing itself: every action is a request of §10.1.
- **Binding:** 127.0.0.1 only, on a port written to the runtime directory.
  `pigeon open` opens `http://127.0.0.1:<port>/login?token=<random 256
  bits>`; the single-use token becomes an `HttpOnly`, `SameSite=Strict`
  cookie. Requests without it, or with another `Host` or `Origin`, are
  refused; every state change is a `POST` with a per-session form token.
- **Five sections**, each page opening with its one-line answer: Home, Files
  (with *Deleted files*), Suggestions (*For you to decide*, *Yours*), People
  & machines, Settings (*Advanced*: diagnostics, logs, ADR 0025's dashboard).
- **Accessibility:** WCAG 2.2 AA (ux.md §3); axe-core in CI checks the
  mockups and the real pages.

## 10.4 Notifications and the launcher (ADR 0029)

| Notify | When |
|---|---|
| *Needs you* | an item becomes *Needs you* ([06 §6.5](06-status.md)) for this person |
| long *Not safe yet* | a version of this device stays *published* more than 24 h |
| a suggestion or conflict waits for this person | on arrival, then at most once a day |

Never for success; grouped; at most one per kind and day, repeated only if
the situation changed **[no ADR: rate]**; a click opens the page that
answers it; off on headless machines and by the `notifications` setting
([11 §11.2](11-local-state.md)). The **launcher** (XDG `.desktop`, macOS app
bundle) runs `pigeon open`; the status icon `pigeon-tray`, in v1.1, renders
only the headline level (ADRs 0027, 0029).

Coded so far: `pigeon-core::notify` decides (laws in
`tests/notify.rs`: never when off or for an absent kind, one per kind and
round, a day apart, repeated only for a changed situation, and told once
the day has passed); `pigeond/src/notices.rs` reads the situation after
the rounds, at most every five minutes, and shows one grouped notification
through `notify-rust`, a click opening the web (Linux). Not yet: the
launcher.

## 10.5 Messages and languages (ADR 0028)

- All user-facing text is in `crates/pigeon-i18n/i18n/{en,fr}/pigeon.ftl`,
  embedded at build time. Language: the `language` setting, else `LANGUAGE`,
  then the first of `LC_ALL`, `LC_MESSAGES`, `LANG`, else English.
- **[no ADR: messages]** Ids are shared by every interface and listed in
  `pigeon_i18n::ids`; tests: `every_bundle_has_every_listed_message_and_nothing_else`,
  `every_message_renders_with_sample_arguments`, `plurals_and_isolation`
  (isolation marks on the web, not in a terminal). An unknown id renders as
  itself; renderers hold no English literals.
- Times: relative under a day, absolute beyond, local; `--json` gives RFC
  3339 (ux.md §4). ux.md §2.2's vocabulary is normative in both languages.

## 10.6 Problems (ADR 0023)

An error that reaches a person is a **Problem**:

```json
{
  "code": "invite-expired",
  "message": {"id": "problem-invite-expired", "args": {"until": "2026-10-12T14:10:00+02:00", "inviter": "Paul"}},
  "hint": {"id": "hint-ask-new-invitation", "args": {"inviter": "Paul", "person": "Martine"}},
  "details": "join: inviter answered Refused(invite-expired)"
}
```

- `code`: stable, public kebab-case, shown as `P-INVITE-EXPIRED` (ux.md §7);
  [`docs/problems.md`](../problems.md) documents each, with its English
  message and when it is raised (`docs_problems_lists_every_code`).
- `message` and `hint`: Fluent ids and arguments rendered at the edge
  (`every_problem_has_a_message_and_ids_are_kebab_case`,
  `a_problem_message_round_trips_as_json`). `details`, for helpers: the
  operation, the object, a failed call's command, status and stderr tail.
- Only the request layer of `pigeond` turns crate errors into Problems.

Codes used: `invite-expired`, `invite-used`, `invite-unknown`,
`invite-not-allowed`, `inviter-offline`, `not-in-group`, `not-allowed`,
`needs-confirmation`, `backup-machine-needs-linux` (§10.7), `disk-full`
and `no-capacity` ([08 §8.9](08-custody-loops.md)), `commit-unsigned`,
`commit-forged`, `commit-unknown-signer`, `commit-not-allowed`,
`commit-malformed` and `commit-waits` ([04 §4.4](04-roster-and-authority.md)),
`repository-lost` (ADR 0046),
`daemon-not-running`, `version-mismatch`, `git-annex-too-old`,
`git-annex-untested`, `update-needed`, `pigeon-too-old`, `peer-too-old`,
`peer-too-new` (ADR 0048), `settings-check`
([03 §3.5](03-repository-and-roles.md)), and in the slice
`not-in-this-build` and `file-system-not-in-this-build` (13 §13.12). The
first build adds **[no ADR]**: `usage` (exit code 2), `unknown-computer`,
`receiving` (`keep` while a copy arrives), `not-a-pigeon-repository`,
`peer-unreachable` and `peer-refused` (the helper, 09 §9.3),
`not-installed` (`pigeon update` without the installer of 14 §14.5), and
`internal-error` (details kept for the debug bundle).

## 10.7 Checks before acting

- `pigeon join` with a custodian invitation, and `pigeon create` for a
  backup machine, are refused except on Linux with systemd
  (`backup-machine-needs-linux`, ADR 0032).
- `pigeon doctor` checks and explains: `pigeond` running and supervised; git
  ≥ 2.38 and git-annex ≥ the minimum ([14](14-platforms-and-packaging.md));
  `ssh-keygen`; the key file's mode; the working tree's file system
  ([14 §14.2](14-platforms-and-packaging.md)); free space against
  `annex.diskreserve`; the settings check; reachability of relay and peers;
  clock skew over 5 minutes **[no ADR: skew threshold]**. `--details` adds
  versions, paths and the last errors.
- A request that changes the catalogue checks the roster first and fails
  with `not-allowed` rather than make a surprise proposal; capture, which
  cannot ask, makes proposals ([07 §7.3](07-capture-and-install.md)).
