# 11. Local state

> Mission: specify every file `pigeond` keeps on a device besides git and
> git-annex: where it lives, its format, who writes it, how it is written so
> that a crash at any instant leaves it usable, and what is deliberately not
> stored (ADR 0036).

## 11.1 Principles (ADR 0036)

- **git and git-annex hold the truth.** The catalogue, the roster, proposals,
  decisions, status reports, where each copy is and when it was checked all
  live in git or git-annex. "git status is the truth" for the working tree
  (ADR 0013). Anything that can be rebuilt from them is not stored: authority
  verdicts, version states, the work queue, observations, the headline.
- **Plain files.** What remains is small: settings, the device key, and per
  repository a journal and three tables. They are TOML (written by people) or
  JSON (written by `pigeond`), read with `serde`.
- **`format = N`** in every file. `pigeond` refuses a newer format with a
  Problem (`state-format-too-new`) instead of guessing, and upgrades an older
  one by rewriting it atomically after a successful read.
- **One writer.** `pigeond` is the only process that writes these files
  (ADR 0011). The CLI and the web interface ask it. The one exception is
  `config.toml`, which people may also edit by hand (§11.2).
- **Atomic whole-file writes.** Write a temporary file in the same directory
  (`tempfile`), fsync it, rename it over the old file, fsync the directory. A
  reader sees the old file or the new one, never a mix.

## 11.2 Per-user files

| Path | Content |
|---|---|
| `<config>/config.toml` | the person's settings for this device (below) |
| `<config>/device-key` · `device-key.pub` | the device key, OpenSSH format, mode 0600 ([09 §9.1](09-network-and-joining.md)) |
| `<state>/logs/` | JSON log lines, daily files, seven kept (ADR 0022, [12](12-observability.md)) |
| `<state>/telemetry/` | the bounded local OTLP-JSON files (ADR 0025) |
| `<state>/traces/` | trace events, only with the `tla-trace` feature (ADR 0008) |
| `<cache>/view/` | read-only copies opened by `pigeon view` (ADR 0035) |
| `<runtime>/pigeon/` | the socket, its lock file and the web port ([09 §9.3](09-network-and-joining.md)) |

`<config>` is `$XDG_CONFIG_HOME/pigeon` on Linux and
`~/Library/Application Support/pigeon` on macOS; `<state>` is
`$XDG_STATE_HOME/pigeon` and `~/Library/Logs/pigeon`; `<cache>` is
`$XDG_CACHE_HOME/pigeon` and `~/Library/Caches/pigeon`; all are found with the
`dirs` crate (ADR 0036). `<runtime>` is `$XDG_RUNTIME_DIR/pigeon` on Linux
and `<cache>/run` on macOS, which has no runtime directory (ADR 0060,
Proposed). On a backup machine `pigeond` runs as a system service and these
are under `/var/lib/pigeon`, `/var/log/pigeon`, `/var/cache/pigeon` and
`/run/pigeon`.

### `config.toml`

```toml
format = 1
language = "auto"                 # auto | en | fr (ADR 0028)
notifications = true              # ADR 0029

[resources]                       # ADR 0037, see 15-resource-use.md
preset = "gentle"                 # gentle | balanced | fast
upload = "auto"                   # "auto" or a rate such as "2 MB/s"
download = "auto"
on_battery = "pause"              # pause | continue, below battery_percent
battery_percent = 20
on_metered = "pause"              # pause | continue
full_speed = []                   # local time windows, e.g. ["12:00-13:00"]
scrub_window = "01:00-06:00"      # backup machines only
paused_until = 2026-10-13T18:00:00+02:00   # written by pigeon pause; absent if not paused

[[groups]]
id = "<genesis commit id>"        # the group ID (04 §4.1)
path = "/home/martine/Famille"    # the folder on this device

[groups.priority]                 # per top-level folder: high | normal | low
"Photos" = "high"
"Archives" = "low"
```

- Keys not listed take their defaults; unknown keys are kept and reported by
  `pigeon doctor`, so that an older `pigeond` does not drop a newer setting.
- `pigeond` watches the file and applies a hand edit at once; an invalid edit
  is refused as a whole with a Problem naming the line, and the previous
  settings stay in force.
- When `pigeond` changes the file (Settings, `pigeon settings`, `pigeon
  pause`), it edits it in place with `toml_edit`, keeping comments and order,
  and writes it atomically **[no ADR: toml_edit]**.
- Which folders are kept on this computer is **not** a setting here: git's
  sparse-checkout file and the device's `wanted` expression record it, and
  `pigeond` restores both at start (ADR 0031, [08 §8.5](08-custody-loops.md)).

## 11.3 Per-repository files: `.git/pigeon/`

Per-repository state lives in `.git/pigeon/` of the repository (on a bare
custodian repository, in its `pigeon/` directory), so it moves with the
repository and git never shares it (ADR 0036).

| File | Written when | Content | Rebuildable? |
|---|---|---|---|
| `install.ndjson` | each install step ([07 §7.5](07-capture-and-install.md)) | the install journal | no: it is the crash record |
| `conflicts.json` | a divergence starts, changes or ends ([07 §7.6](07-capture-and-install.md)) | the divergence table | no: "what pigeon last installed" is known only here |
| `purges.toml` | a custodian first merges a purge decision, or seals it ([08 §8.6](08-custody-loops.md)) | when it received each active decision, and its seals | yes, conservatively: a lost file restarts each waiting period (ADR 0042) |
| `clock.json` | at most every second, and at exit | the Lamport clock of trace events (ADR 0008) | no, but losing it only weakens trace ordering |
| `invitations.json` | an invitation is created, used, cancelled or expires ([09 §9.6](09-network-and-joining.md)) | the hash of each secret, purpose, expiry, used flag | no |
| `aside/` | install step 4 | files moved aside, kept until the first full scan 10 minutes after `done` | no ([07 §7.5](07-capture-and-install.md), ADR 0040) |
| `stage/` | install step 2; keeping a folder (ADR 0039) | new files before they enter the folder | yes: unreferenced files are deleted at start |
| `recovered/` | install steps 6 and 9 | moved-aside content that was not pigeon's, until captured as a recovered version | no ([07 §7.6](07-capture-and-install.md)) |
| `RECOVERY.txt` (custodians, in the repository directory) | the roster changes | the group's name, the other custodians, and the git and git-annex commands that read the files without pigeon ([09 §9.9](09-network-and-joining.md), ADR 0047) | yes: written again from the roster |

### `install.ndjson`

One JSON object per line, appended and fsynced before the step it announces:

```json
{"format":1,"seq":41,"op":"begin","path":"Photos/a.jpg","mode":"100644","old_key":"SHA256E-s…","old":[66311,4224493,2097152,1791922902880098214],"new_key":"SHA256E-s…","tmp":".git/pigeon/stage/41","tmp_ino":4224517,"aside":".git/pigeon/aside/41"}
{"format":1,"seq":41,"op":"proved"}
{"format":1,"seq":41,"op":"done"}
{"format":1,"seq":41,"op":"released"}
```

- `old` is the index entry's `(dev, ino, size, mtime in ns)` that step 6 checks.
  `proved` records that the writer scan and the check passed, before step 7
  (ADR 0055). An entry ends with one of `done`, `skip`, `raced` or
  `recovered`; `released` records the removal of its aside (ADR 0040). A
  staged file is removed only once its entry has ended.
- At start, `pigeond` replays every `begin` without an end with the recovery
  rule of [07 §7.5](07-capture-and-install.md): forward if it has `proved`,
  back otherwise; when every entry has ended and
  its aside is released, it truncates the file (rename of an empty file over it).
- A torn last line (a crash during the append) is ignored: its step had not
  started. Every other malformed line stops install with a Problem.

### `conflicts.json`

```json
{
  "format": 1,
  "paths": {
    "Recettes/tarte.docx": {
      "kind": "suggestion",
      "since": "2026-10-13T14:02:11+02:00",
      "state": "waiting",
      "mine": "SHA256E-s…",
      "mine_stat": [66311, 4224610, 18432, 1791922931000000000],
      "proposal": "<commit id of the latest proposal of the chain>",
      "group": "SHA256E-s…",
      "installed": "SHA256E-s…",
      "told": false
    }
  }
}
```

`installed` is the key pigeon last wrote at the path; `mine_stat` is the
`(dev, ino, size, mtime)` of the person's file when `mine` was captured, which
*Take the group's version* checks (ADR 0040); `told` records that the
person was told once (ADR 0034). The table is small (one entry per diverging
path) and rewritten whole.

### `invitations.json`

```json
{"format":1,"invitations":[{"id":"inv-7Q2M","secret_sha256":"…","purpose":{"new-person":{"name":"Martine","admin":false}},"expires":"2026-10-14T14:10:00+02:00","used_by":null}]}
```

Expired entries are kept 30 days for the "already used / expired" messages,
then removed **[no ADR: retention]**.

## 11.4 What is kept only in memory

| Data | Rebuilt from |
|---|---|
| authority verdicts of commits | the history, by the rule of [04 §4.4](04-roster-and-authority.md) |
| version states, reasons, the headline | `main`, the git-annex location log, status reports ([06](06-status.md)) |
| the work queue | the catalogue, the location log and `wanted` ([15](15-resource-use.md)) |
| held changes | the working tree compared with `main` at each scan ([07 §7.4](07-capture-and-install.md)) |
| web sessions | none: a restart of `pigeond` asks people to open pigeon again ([10 §10.3](10-interfaces.md)) |
| peers' addresses | iroh's lookup and mDNS ([09 §9.5](09-network-and-joining.md)) |

A restart therefore costs one pass over the history and a `git status`, as
large repositories already need at start (ADR 0013).

## 11.5 Laws (ADR 0012)

Each is a property test over generated states and crash points:

- **Atomicity.** For any crash point during a write, a later read returns the
  old value or the new one.
- **Replay.** Replaying the install journal twice gives the same disk as
  replaying it once, and every path ends at its old content, its new content
  or the person's, with the index agreeing (INV-Journal; the `CaptureInstall`
  model checks a crash during replay).
- **Round trip.** `parse(render(x)) = x` for every file type and format, and a
  file written by format N is read by every later format.
- **Refusal.** A file with a newer `format` is never modified.
- **Rebuild.** Deleting every in-memory structure and rebuilding it from git,
  git-annex and the files of this chapter gives the same states and queue.
