# 10. Interfaces

> Mission: specify what people and scripts touch: the internal request API that
> every interface shares, the `pigeon` CLI, the local web interface,
> notifications and the launcher, the message catalogue, and how errors reach a
> person. The words are those of ux.md; the operations are those of the other
> files of this specification.

## 10.1 One request API (ADR 0021)

Every operation is one **request type** of `pigeond`'s internal API, defined
once in the `pigeon-proto` crate ([14](14-platforms-and-packaging.md)) with
serde, and carried as NDJSON over the local socket ([09 §9.3](09-network-and-joining.md)).
The CLI and the web interface are renderers of the same requests and
responses, so no operation exists in only one of them (ADR 0021). A response
is either a result, a stream of progress events then a result, or a `Problem`
(§10.6).

| Request | CLI | Web | Specified in |
|---|---|---|---|
| `Status {path?, not_shared}` | `pigeon status [PATH] [--not-shared]` | Home; the folder view's *Not shared* list | [06](06-status.md), [07 §7.9](07-capture-and-install.md) |
| `Create {name, folder, k, device_name}` | `pigeon create NAME` | Welcome → Create | [04 §4.1](04-roster-and-authority.md) |
| `Invite {person, admin, custodian}` | `pigeon invite [--admin] [--backup-machine] PERSON` | People → Invite someone | [09 §9.6](09-network-and-joining.md) |
| `Join {invitation, folder, keep}` | `pigeon join INVITATION [FOLDER]` | Welcome → Join | [09 §9.6](09-network-and-joining.md) |
| `Keep {folder}` · `Evict {folder}` | `pigeon keep PATH` · `pigeon free PATH` | Files → *Keep on this computer* | [08 §8.5](08-custody-loops.md) |
| `Drop {path}` | `pigeon free --without-checking PATH` | — (CLI only, ux.md §2.2) | [08 §8.7](08-custody-loops.md) |
| `History {path}` | `pigeon history PATH` | a file's page | [05](05-catalogue-and-names.md) |
| `View {path, version \| group}` | `pigeon view PATH [--version N \| --group]` | *Open a copy*, *Open Paul's version* | [07 §7.6](07-capture-and-install.md) |
| `Restore {path, version \| at}` | `pigeon restore PATH [--version N \| --at TIME]` | *Restore this version*, Deleted files | [05 §5.1](05-catalogue-and-names.md) |
| `Deleted {path?}` | `pigeon deleted [PATH]` | Files → Deleted files | [05 §5.1](05-catalogue-and-names.md) |
| `Proposals` · `Accept {id, override?}` · `Reject {id, note?}` | `pigeon suggestions` · `pigeon accept ID [--override REASON]` · `pigeon decline ID [--note TEXT]` | Suggestions | [04 §4.5, §4.6](04-roster-and-authority.md) |
| `Conflicts` · `Resolve {path, choice}` | `pigeon conflicts` · `pigeon resolve PATH --take-group \| --keep-copy DEST \| --suggest-again` | Suggestions → Yours, Home | [07 §7.6](07-capture-and-install.md) |
| `People` · `Machines` | `pigeon people` · `pigeon machines` | People & machines | [04 §4.2](04-roster-and-authority.md) |
| `RemoveDevice {name}` · `Leave` | `pigeon remove-computer NAME` · `pigeon leave` | *I lost a computer* · Settings | [09 §9.7](09-network-and-joining.md) |
| `RecoverySheet` · `Recover {sheet, folder, from?}` | `pigeon recovery-sheet` · `pigeon recover SHEET [FOLDER] [--from DEVICE]` | People → *Print a recovery sheet* · Welcome → *Recover a group* | [09 §9.9](09-network-and-joining.md) |
| `Refound {repository?}` | `pigeon refound [REPOSITORY]` | — (CLI only) | [09 §9.9](09-network-and-joining.md) |
| — | `pigeon export --to DIR [--at DATE\|COMMIT] [--deleted] [REPOSITORY…]` | — (without `pigeond`) | [09 §9.9](09-network-and-joining.md) |
| `Purge {path}` · `CancelPurge {id}` | `pigeon erase PATH` · `pigeon erase --cancel ID` | *Erase forever* | [08 §8.6](08-custody-loops.md) |
| `Held` · `ReleaseHeld {id, choice}` | `pigeon held` · `pigeon held ID --share \| --bring-back \| --rename` | Home → Needs you | [07 §7.4](07-capture-and-install.md) |
| `Pause {duration?}` · `Resume` | `pigeon pause [DURATION]` · `pigeon resume` | Settings | [15](15-resource-use.md) |
| `GetSettings` · `SetSettings {…}` | `pigeon settings [KEY [VALUE]]` | Settings | [11 §11.2](11-local-state.md) |
| `Doctor {details}` | `pigeon doctor [--details]` | Settings → Advanced | §10.7 |
| `DebugBundle {peers}` | `pigeon debug bundle [--from NAME…]` | Settings → Advanced | [12](12-observability.md) |
| `Subscribe` | — | live updates (SSE) | §10.3 |

- Request names are the internal ones (ADR 0024 names `evict`, `purge`,
  `reject` as examples); the CLI verbs are ux.md's words (`free`, `erase`,
  `decline`). Both are stable; renaming either is an interface change.
- `Accept {override}` is Force: an admin accepts a suggestion that the folder
  rules do not let them decide, with a reason that becomes the
  `Pigeon-Force:` trailer ([04 §4.6](04-roster-and-authority.md)) **[no ADR: verb]**.
- `pigeon held`, `pigeon settings`, `pigeon leave`, `pigeon erase --cancel`
  and `pigeon invite --list | --cancel ID` complete ux.md §7 for operations
  this specification defines **[no ADR: verbs]**.
- Requests are versioned with the socket protocol: `pigeond` answers `Hello`
  with its version, and a CLI of another version refuses with
  `version-mismatch` and a hint to restart pigeon.

## 10.2 The command line (ADR 0024)

- `clap` 4 with derive; one subcommand per request; shell completions and man
  pages generated from the same definitions.
- **Output.** The human result on stdout, messages and progress on stderr,
  following the headline-then-detail pattern (ux.md §7). Colour only on a
  terminal, through `anstream`, which honours `NO_COLOR` and `CLICOLOR`;
  colour is never the only carrier of meaning (ux.md §3, principle 10).
  Progress bars (`indicatif`) only on a terminal.
- **`--json`** on every command: the response of the API in its schema on
  stdout; problems as JSON on stderr. Scripts depend on `--json` and problem
  codes, never on wording.
- **Questions.** Dangerous operations ask before acting: `erase`, `accept
  --override`, `refound`, `recover`, `free --without-checking`, `remove-computer`, `leave`, a held
  mass deletion. Without a terminal they need `--yes`; `--no-input` never asks
  and fails with `needs-confirmation` instead.
- **Exit codes:** 0 success; 1 failure, with a `Problem`; 2 usage error.
  `pigeon status` exits 0 whatever the headline, so that scripts read the
  level from `--json` (`"level": "needs-you" | "not-safe" | "not-checked" |
  "working" | "safe"`).
- The CLI holds no logic: it parses, sends a request, renders the response.
  Without `pigeond` it says so and how to start it (`pigeon doctor`). The one
  exception is `pigeon export`, which reads repositories itself, read-only,
  through the same core code, so that files can be read when `pigeond`
  cannot run (ADR 0047).

## 10.3 The web interface (ADR 0021, ux.md §6)

- Served by `pigeond` with `axum`, rendered on the server with `maud`, no
  JavaScript framework; a small script subscribes to server-sent events to
  refresh the headline and the activity. Without JavaScript the pages work,
  with a refresh link.
- **Binding.** 127.0.0.1 only, on a port chosen at start and written to the
  runtime directory. `pigeon open` (and the launcher, and a notification)
  opens `http://127.0.0.1:<port>/login?token=<random 256 bits>`; the token is
  single use and becomes an `HttpOnly`, `SameSite=Strict` session cookie.
  Requests without the cookie, or with a `Host` or `Origin` other than the
  loopback address and port, are refused (CSRF, DNS rebinding). Every
  state-changing request is a `POST` with a per-session form token.
- **Five sections** in a fixed navigation bar, each page starting with its own
  one-line answer (ux.md §6): Home, Files (with *Deleted files*), Suggestions
  (*For you to decide*, *Yours*), People & machines, Settings (with
  *Advanced*: diagnostics, logs, the telemetry dashboard of ADR 0025).
- **Accessibility.** WCAG 2.2 level AA (ux.md §3, principle 10): keyboard use
  throughout, visible focus, text alternatives, contrast, states written in
  words and never by colour alone. An automated check (axe-core in CI) runs on
  the mockup pages and the real pages; the usability test covers the rest.
- The web interface writes nothing itself: every action is a request of §10.1.

## 10.4 Notifications and the launcher (ADR 0029)

Notifications go through `notify-rust` (D-Bus on Linux, Notification Center on
macOS). They are calm:

| Notify | When |
|---|---|
| *Needs you* | an item becomes *Needs you* ([06 §6.5](06-status.md)) for this person |
| long *Not safe yet* | a version of this device stays *published* more than 24 h |
| a suggestion or a conflict waits for this person | when it arrives, then once a day at most |

- Never for success; grouped, never one per file; at most one notification per
  kind and day, repeated only if the situation changed **[no ADR: rate]**.
- Clicking a notification opens the web page that answers it.
- Notifications are off on headless machines and can be turned off in Settings
  (`notifications`, [11 §11.2](11-local-state.md)).
- The **launcher** named "pigeon" (an XDG `.desktop` file on Linux, a small app
  bundle on macOS) runs `pigeon open`. The status icon (`pigeon-tray`, a
  `tray-icon` client of `pigeond`) comes in v1.1 and only renders the headline
  level (ADRs 0027, 0029).

## 10.5 Messages and languages (ADR 0028)

- All user-facing text lives in Fluent files,
  `crates/pigeon-i18n/i18n/en/pigeon.ftl` and `…/fr/pigeon.ftl`, embedded at
  build time and loaded with `fluent-bundle` in the `pigeon-i18n` crate;
  `fluent-langneg` picks the language from the `language` setting, then the
  system locale (`LANGUAGE`, then the first of `LC_ALL`, `LC_MESSAGES` and
  `LANG`, as gettext reads them), then English.
- Message ids are shared by the CLI, the web interface, notifications and the
  tray. Plural and gender rules are Fluent's.
- A test checks that every message id exists in every language, that each
  message formats with sample arguments, and that no Rust code holds English
  text meant for people (a lint on string literals in renderers). Every id is
  listed with its variables in `pigeon_i18n::ids`, which the test compares
  with each bundle. An argument is wrapped in Unicode isolation marks in the
  web interface, not in a terminal; an unknown id renders as the id itself
  and never fails, which the test keeps from happening. **[no ADR: messages]**
- Times use `jiff`: relative under a day ("4 minutes ago"), absolute beyond
  ("Monday 18:02"), in local time; `--json` gives RFC 3339 instants (ux.md §4).
- The vocabulary table of ux.md §2.2 is normative for both languages: *safe* /
  *en sécurité*, *shared, not safe yet* / *partagé, pas encore en sécurité*,
  *only on this computer* / *seulement sur cet ordinateur*, and so on.

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

- `code` is stable, kebab-case, documented in `docs/problems.md`, and part of
  the public interface. People see it as `P-` and the code in capitals
  (`Code: P-INVITE-EXPIRED`, ux.md §7).
- `message` and `hint` are a Fluent id and arguments, rendered at the edge in
  the reader's language; logs and `--json` keep the code and the arguments.
- `details` is for people helping: the operation, the object, and for a failed
  `git` or `git annex` call the command, exit status and end of stderr.
- Internally, each crate has `thiserror` enums with the cause as `#[source]`;
  only the request layer of `pigeond` turns them into Problems. `anyhow` only
  in tests and tools.

Problem codes this specification uses include: `invite-expired`,
`invite-used`, `invite-unknown`, `invite-not-allowed`, `inviter-offline`,
`not-in-group`, `not-allowed` (a request the roster does not allow),
`needs-confirmation`, `backup-machine-needs-linux` (§10.7), `disk-full`
([08 §8.9](08-custody-loops.md)), `same-domain`, `independence-unknown`, `repository-lost` (ADR 0046),
`daemon-not-running`, `version-mismatch`, `git-annex-too-old`,
`git-annex-untested`, `update-needed`, `pigeon-too-old`, `peer-too-old`,
`peer-too-new` (ADR 0048),
`settings-check` ([03 §3.5](03-repository-and-roles.md)), and in the slice
`not-in-this-build` (13 §13.12) and `file-system-not-in-this-build` (13 §13.12).
The first build adds **[no ADR]**: `usage` (the arguments do not fit, exit
code 2), `unknown-computer` (no device of that name), `receiving` (`keep`
while a copy is still arriving), `not-a-pigeon-repository`,
`peer-unreachable` and `peer-refused` (the helper's requests, 09 §9.3), and
`internal-error` (anything else, its details kept for the debug bundle).

## 10.7 Checks before acting

- `pigeon join --backup-machine` (an invitation with `custodian` purpose) and
  `pigeon create` on a machine meant as a backup machine are refused on
  anything but Linux with systemd, with `backup-machine-needs-linux` (ADR 0032).
- `pigeon doctor` checks, and explains in words: `pigeond` running and
  supervised; git ≥ 2.38 and git-annex ≥ the minimum version
  ([14](14-platforms-and-packaging.md)); `ssh-keygen` present; the key file
  mode; the working tree's file system ([14 §14.2](14-platforms-and-packaging.md));
  free disk space against `annex.diskreserve`; on backup machines, the failure
  domain: which other backup machines share its disk or machine, and whether
  it runs in a virtual machine or container with no declared `domain`
  (ADR 0046); the settings check; reachability of the relay and of each peer; clock skew against peers above 5 minutes
  **[no ADR: skew threshold]**. `--details` adds versions, paths and the last
  errors.
- Every request that changes the catalogue checks the roster first and fails
  with `not-allowed` rather than producing a commit that would become a
  surprise proposal; capture, which cannot ask, makes proposals instead
  ([07 §7.3](07-capture-and-install.md)).
