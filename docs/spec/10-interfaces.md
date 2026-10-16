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
| `Health` | — | Home's headline, current and older counts, timestamped capacity and queues, suggestions and repair epochs | [12 §12.2](12-observability.md) |
| `Create {name, folder, k, device_name, person?}` | `pigeon create NAME [--person NAME]` | Welcome → Create (*Your name*) | [04 §4.1](04-roster-and-authority.md) |
| `Invite` | `pigeon invite` | People → Invite | [09 §9.6](09-network-and-joining.md) |
| `Join {invitation, folder, keep, name?, person?, existing, custodian}` | `pigeon join INVITATION [FOLDER] [--person NAME] [--name NAME] [--backup-machine]` | Welcome → Join | [09 §9.6](09-network-and-joining.md) |
| `Keep {folder}` · `Evict {folder, confirmed}` · `FreePreview {folder}` | `pigeon keep PATH` · `pigeon free PATH [--yes]` | Files → *Keep on this computer* · *Free up space*, confirmed | [08 §8.5](08-custody-loops.md) |
| `Drop {path, confirmed}` | `pigeon free --without-checking PATH` | — (CLI only, ux.md §2.2) | [08 §8.7](08-custody-loops.md) |
| `History {path}` | `pigeon history PATH` | a file's page | [05](05-catalogue-and-names.md) |
| `View {path, version \| group}` | `pigeon view PATH [--version N \| --group]` | *Open a copy*, *Open Paul's version* | [07 §7.6](07-capture-and-install.md) |
| `Restore {path, version \| at}` | `pigeon restore PATH [--version N \| --at TIME]` | *Restore this version*, Deleted files | [05 §5.1](05-catalogue-and-names.md) |
| `Deleted {path?}` | `pigeon deleted [PATH]` | Files → Deleted files | [05 §5.1](05-catalogue-and-names.md) |
| `Proposals` · `Accept {proposal, override?}` · `Reject {proposal, note?}` · `DecideFiles {proposal, paths, accept, note?, override?}` | `pigeon suggestions` · `pigeon accept ID [FILE…] [--override REASON]` · `pigeon decline ID [FILE…] [--note TEXT]` | Suggestions | [04 §4.5, §4.6](04-roster-and-authority.md), ADR 0077 |
| `Conflicts` · `Resolve {path, choice}` | `pigeon conflicts` · `pigeon resolve PATH --take-group \| --keep-copy DEST \| --suggest-again` | Suggestions → Yours, Home | [07 §7.6](07-capture-and-install.md) |
| `People` · `Machines` | `pigeon people` · `pigeon machines` | People & machines | [04 §4.2](04-roster-and-authority.md) |
| `RemoveDevice {name}` · `Leave` | `pigeon remove-computer NAME` · `pigeon leave` | *I lost a computer* · Settings | [09 §9.7](09-network-and-joining.md) |
| `RecoverySheet` · `Recover {sheet, folder, from?}` | `pigeon recovery-sheet` · `pigeon recover SHEET [FOLDER] [--from DEVICE]` | People → *Print a recovery sheet* · Welcome → *Recover a group* | [09 §9.9](09-network-and-joining.md) |
| `Refound {repository?}` | `pigeon refound [REPOSITORY]` | — (CLI only) | [09 §9.9](09-network-and-joining.md) |
| — | `pigeon export --to DIR [--at DATE\|COMMIT] [--deleted] [REPOSITORY…]` | — (without `pigeond`) | [09 §9.9](09-network-and-joining.md) |
| `Purge {path}` · `CancelPurge {id}` | `pigeon erase PATH` · `pigeon erase --cancel ID` | *Erase forever* | [08 §8.6](08-custody-loops.md) |
| `Held` · `ReleaseHeld {batch, choice}` | `pigeon held` · `pigeon held ID --share \| --bring-back \| --rename` | Home → Needs you | [07 §7.4](07-capture-and-install.md) |
| `Pause {duration?}` · `Resume` · `Paused` | `pigeon pause [DURATION]` · `pigeon resume` | Settings; the pause banner of every page | [15](15-resource-use.md) |
| `GetSettings` · `SetSettings {…}` | `pigeon settings [KEY [VALUE]]` | Settings | [11 §11.2](11-local-state.md) |
| `Doctor {details}` | `pigeon doctor [--details]` | Settings → Advanced | §10.7 |
| `DebugBundle {peers}` | `pigeon debug bundle [--from NAME…]` | Settings → Advanced | [12](12-observability.md) |
| `Folders` | `pigeon status` (by file) | Files | [08 §8.5](08-custody-loops.md) |
| `Files {folder}` | `pigeon status FOLDER` | Files → a folder | [06 §6.2](06-status.md) |
| `FolderRules {folder?}` · `SetFolderRule {folder, rule, confirmed}` | `pigeon folder-rule [PATH [--owner PERSON \| --add-only [--steward PERSON…] \| --per-person \| --default]]` | Files → a folder → Sharing rule | [04 §4.3](04-roster-and-authority.md) |
| `Deleted` | `pigeon restore PATH` | Files → Deleted files | [10 §10.3](10-interfaces.md) |
| `Subscribe` | — | live updates (SSE) | §10.3 |

- Request names are internal (ADR 0024: `evict`, `purge`, `reject`), CLI
  verbs ux.md's (`free`, `erase`, `decline`); renaming either is an
  interface change. `Accept {override}` is Force, its reason the
  `Pigeon-Force:` trailer ([04 §4.6](04-roster-and-authority.md)) **[no ADR: verb]**.
- `pigeon held`, `settings`, `leave` and `erase --cancel` complete ux.md §7
  **[no ADR: verbs]**.
- `pigeond` answers `Hello` with its version; a CLI of another version
  refuses with `version-mismatch`.

`restore FILE` without a version selects its latest saved version,
including the version before deletion. `restore FOLDER` selects the latest
nonempty accepted snapshot of the folder; `--at` applies an inclusive
author-time cutoff. A folder uses one snapshot, never a mixture of per-file
times. Files absent from that snapshot are not deleted. `--version` is a
file-history number; it is refused for folders with instructions to use
`--at`. Missing nested working directories can be rebuilt, while symlinks
and nested repositories are refused. Capture records a successful restore
as new history, keeping earlier bytes and modes.

Restores use the install journal and preserve saved executable modes.
Changed or open files win over the restore. A folder's incomplete result
is one final Problem with restored/unchanged counts and paths to check;
the CLI exits unsuccessfully rather than stopping at an earlier success.
Deleted files offers whole-folder actions only for wholly deleted subtrees;
individual Restore selects the latest saved version. Free and keep confirm
scheduled work; Files reports requested local copies still arriving.
Evidence: [folder-restore-5](../reports/folder-restore-5.md).

Folder rules are readable by every current member. Only an admitted admin
computer may change them; recovery keys cannot. The editor resolves current
person names or IDs, refuses ambiguous names, and supports `me`. An empty
add-only steward set follows the current admins. Reset removes one direct
rule and inherits the closest ancestor, leaving deeper rules untouched.
The daemon signs a roster-only change from the accepted tree, judges it
before publishing, and preserves unrelated working and index contents.
Rules version 1 retains its literal prefix matching; the editor refuses a
case or Unicode alias of an existing scope. It does not reinterpret old
signed history. Evidence: [folder-rules-5](../reports/folder-rules-5.md).

## 10.2 The command line (ADR 0024)

- One subcommand per request; completions and man pages from the same
  definitions. Result on stdout; messages and progress on stderr, headline
  then detail (ux.md §7); colour and bars only on a terminal, never alone.
- **Completion.** `COMPLETE=SHELL pigeon` prints the script a shell's
  startup file sources (zsh, bash, fish, elvish, PowerShell).
  `pigeon completions SHELL` prints the same script so startup files from
  earlier versions continue to work; the shell then
  runs `pigeon` with `COMPLETE` set, and it answers from the same
  definitions, with the values `pigeond` lists: suggestions, held batches,
  the group's other machines, settings and their choices. A
  daemon that does not answer within half a second gives no values.
- **`--json`** on every command: the response on stdout, problems on stderr.
  Scripts rely on `--json` and problem codes, never on wording.
- **Questions.** `erase`, `accept --override`, `refound`, `recover`, `free
  --without-checking`, `remove-computer`, `leave` and a held mass deletion
  ask first; without a terminal they need `--yes`; `--no-input` fails with
  `needs-confirmation`. `join` asks for what its line lacks (09 §9.6), and
  whether to join a person of the group (`person-exists` without a
  terminal; `--yes` joins them).
- **Exit codes:** 0 success, 1 a `Problem`, 2 usage. `pigeon status` exits 0
  whatever the headline; scripts read `"level": "needs-you" | "not-safe" |
  "not-checked" | "working" | "safe"`.
- The CLI holds no logic; without `pigeond` it points to `pigeon doctor`.
  Only `pigeon export` reads repositories itself, read-only, with the same
  core code (ADR 0047).

## 10.3 The web interface (ADR 0021, ux.md §6)

Coded so far (`crates/pigeon-web`, `pigeond/src/web.rs`): the binding, the
login token and cookie, the `Host`/`Origin` check, Home, Suggestions and
Machines pages read through the same handler as the socket, each request
in a task of its own that runs to its end even when the browser leaves, as
on the socket, and an SSE headline; Files lists the top-level folders (`Folders`) with *Keep on this
computer* or *Free up space*; Settings changes `language` and
`notifications` (`GetSettings`, `SetSettings`, also `pigeon settings`), a
new language showing at once; People & machines shows the line that joins
the group (`Invite`), with *Copy*, and removes a machine, asking again
first; *Accept*, *Decline*, keep, free, *Save* and *Remove* are `POST`s carrying
a per-session form token. A folder's page lists its files (`Files`), a file's page
its versions (`History`) with *Restore this version* (`Restore`);
Deleted files lists what `main` deleted (`Deleted`), each with *Restore*.
Settings also pauses for an hour, a day or until *Share again* (`Pause`,
`Resume`, also `pigeon pause` and `pigeon resume`, 15 §15.6), offering
only the action that applies; while paused, every page opens with a banner
saying until when, with *Share again* (`Paused`, asked before each page).
*Free up space* asks first: which files leave this computer, the backup
machines they stay on, and that *Keep on this computer* brings them back
(`FreePreview`); while no backup machine has been heard from within the
hour that makes a check recent, `Evict` answers `backups-quiet` and the
page asks a second time, saying when they were last in touch, before
`Evict {confirmed}`. `pigeon free` asks that second question too, or takes
`--yes`. A freed file reads "Safe, not on this computer" in Files, on its
folder's page and on its own page (`Files` marks it `freed`).
Times to come read as local times ("until 14:38", "expires tomorrow at
09:00") and sizes as "29.3 GB", in the person's language.
People & machines lists the people first (`People`, also `pigeon people`);
`pigeon deleted` lists Deleted files.
Home lists the held deletions (`Held`) with *Delete for everyone*, asked
again, and *Bring them back* (`ReleaseHeld`); Settings has *Leave the
group*, asked again (`Leave`).
Home also lists the files whose copy here differs from the group's
(`Conflicts`, 07 §7.6), each a link to its page; there *Take the group's
version*, *Keep mine as a copy* (named next to the file) and, unless the
suggestion still waits, *Suggest again* are `Resolve`, each saying where
both versions go. A file's page marks its current version. Every page opens
with a skip link to `main`, and a repeated button names its row (version,
computer, suggestion or folder) in visually hidden text.
axe-core (`scripts/check-a11y`, in CI) checks the mockups and every page of
the test backend in English and French, without colour contrast, which
needs a layout engine. Not yet: the other settings, the other actions.

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
- **Bridge reuse:** `pigeon-bridge` mounts these same pages and handlers at
  `/clients/ID/peer/`, behind its browser session, origin, ownership, revocation
  and form-token checks. Each RPC uses the selected client's identity-checked
  daemon; live polls repeat session and revocation checks. The account's
  `/peer/` mount instead uses its personal control peer. Managed client setup,
  revocation and conflicts stay in bridge management, and offline pinning stays
  in the Nextcloud app. See [the bridge guide](../nextcloud-bridge.md).
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
through `notify-rust`, a click opening the web (Linux). `pigeon-install`
writes the launcher: `<data>/applications/pigeon.desktop` on Linux,
`~/Applications/pigeon.app` on macOS, each running `pigeon open`.

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
  "code": "inviter-offline",
  "message": {"id": "problem-inviter-offline", "args": {"inviter": "Paul"}},
  "hint": {"id": "hint-inviter-offline", "args": {"inviter": "Paul"}},
  "details": "join: no admitter answered"
}
```

- `code`: stable, public kebab-case, shown as `P-INVITER-OFFLINE` (ux.md §7);
  [`docs/problems.md`](../problems.md) documents each, with its English
  message and when it is raised (`docs_problems_lists_every_code`).
- `message` and `hint`: Fluent ids and arguments rendered at the edge
  (`every_problem_has_a_message_and_ids_are_kebab_case`,
  `a_problem_message_round_trips_as_json`). `details`, for helpers: the
  operation, the object, a failed call's command, status and stderr tail.
- Only the request layer of `pigeond` turns crate errors into Problems.

Codes used: `invite-unknown`, `already-in-group`, `in-another-group`,
`person-exists`, `invite-not-allowed`, `inviter-offline`, `not-in-group`,
`not-allowed`,
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

- `pigeon join --backup-machine`, and `pigeon create` for a
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

Doctor also checks that `git-annex-p2p-iroh --version` runs in the same
child environment used by git-annex and matches the daemon release. A
missing, failed, stale or timed-out helper gives
`transport-helper-unavailable`, with a reinstall remedy. An installation
or group problem suppresses the final everything-in-place reassurance.

A pending suggestion can be reviewed with `ViewSuggestion {proposal, path}`
(`pigeon view PATH --suggestion ID`). Only a changed path in one currently
pending verified proposal is eligible. The daemon returns a read-only copy
outside the shared folder, or explains that the proposal deletes that path.
Review does not adopt the proposal or replace accepted bytes. The web uses
a checked POST before downloading the copy.

The Suggestions page (ADR 0077) groups the suggestions this person
decides, their own, and the others'. Each lists its files with what
changes in each, in words and sizes, and *Review* for each file still
there afterwards. On one this person decides every file has a checkbox,
ticked: *Accept selected* accepts the ticked files and declines the
others, *Decline selected* the reverse, with a note for the author; an
admin override asks its reason before accepting only. With several,
*Accept all* and *Decline all* decide every file of each, except one that
needs an override, which is named. The result says which files were
accepted and which declined, and that the author will be told. The
author sees their suggestions decided in the last 30 days, file by file,
with the note, and a file's page shows their suggestion of it that waits
or was decided. Every form works without JavaScript: the review buttons
belong to forms of their own, and Enter in the note submits nothing.
