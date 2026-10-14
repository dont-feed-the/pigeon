# 11. Local state

> Mission: every file `pigeond` keeps on a device besides git and git-annex:
> where, its format, who writes it, how a crash leaves it usable, and what
> is not stored (ADR 0036).

## 11.1 Principles (ADR 0036)

- **git and git-annex hold the truth** (catalogue, roster, proposals,
  decisions, reports, copies and checks); `git status` is the truth for the
  working tree (ADR 0013). What can be rebuilt is not stored (§11.4).
- **Plain files:** settings, the device key, and per repository a journal
  and a few tables; TOML if people write it, JSON if `pigeond` does.
- **`format = N`** in every file: a newer format is refused
  (`state-format-too-new`), an older one upgraded by an atomic rewrite after
  a successful read.
- **One writer:** `pigeond` (ADR 0011); people may also edit `config.toml`.
- **Atomic whole-file writes:** temporary file in the same directory, fsync,
  rename over, fsync the directory.

## 11.2 Per-user files

| Path | Content |
|---|---|
| `<config>/config.toml` | the person's settings for this device (below) |
| `<config>/device-key` · `device-key.pub` | the device key, OpenSSH format, mode 0600 ([09 §9.1](09-network-and-joining.md)) |
| `<state>/logs/` | JSON log lines, daily files, seven kept (ADR 0022, [12](12-observability.md)) |
| `<state>/telemetry/` | the bounded local OTLP-JSON files (ADR 0025) |
| `<state>/traces/` | trace events, only with the `tla-trace` feature (ADR 0008) |
| `<cache>/view/` | read-only copies opened by `pigeon view` (ADR 0035) |
| `<cache>/bin/` | links first in the children's `PATH`: the transport helper, and `git-remote-p2p-annex` where git-annex installs none ([14 §14.3](14-platforms-and-packaging.md)) |
| `<runtime>/pigeon/` | the socket, its lock file and the web port ([09 §9.3](09-network-and-joining.md)) |

`<config>`, `<state>`, `<cache>` are `$XDG_CONFIG_HOME/pigeon`,
`$XDG_STATE_HOME/pigeon`, `$XDG_CACHE_HOME/pigeon` on Linux and
`~/Library/Application Support/pigeon`, `~/Library/Logs/pigeon`,
`~/Library/Caches/pigeon` on macOS (ADR 0036). `<runtime>` is
`$XDG_RUNTIME_DIR/pigeon`, or `<cache>/run` on macOS. On a backup machine,
a system service, they are `/var/lib/pigeon`, `/var/log/pigeon`,
`/var/cache/pigeon` and `/run/pigeon`.

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

[network]                         # 09 §9.5
bind = "0.0.0.0:0"                # the endpoint's address; the lab binds 127.0.0.1:0
mdns = true                       # find peers on the local network

[loops]
interval_seconds = 30             # a round of the loops besides change notices (08 §8.1)

[[groups]]
id = "<genesis commit id>"        # the group ID (04 §4.1)
path = "/home/martine/Famille"    # the folder on this device

[groups.priority]                 # per top-level folder: high | normal | low
"Photos" = "high"
"Archives" = "low"
```

- `[network]` and `[loops]` are **[no ADR]**: the lab and the slice set
  `bind = "127.0.0.1:0"`, `mdns = false`, `interval_seconds = 1`. An
  unspecified bind address is told to peers as loopback.
- Missing keys take defaults; unknown keys are kept and reported by `pigeon
  doctor`, so an older `pigeond` drops no newer setting.
- A hand edit applies at once; an invalid one is refused whole with a
  Problem naming the line, and the old settings stay.
- `pigeond` edits the file in place, keeping comments and order, and writes
  it atomically **[no ADR: toml_edit]**.
- Kept folders are not a setting: the sparse-checkout file and `wanted`
  record them ([08 §8.5](08-custody-loops.md)).

## 11.3 Per-repository files: `.git/pigeon/`

Per-repository state lives in `.git/pigeon/` (`pigeon/` of a bare
custodian repository), so it moves with the repository and git never
shares it (ADR 0036).

| File | Written when | Content | Rebuildable? |
|---|---|---|---|
| `install.ndjson` | each install step ([07 §7.5](07-capture-and-install.md)) | the install journal | no: it is the crash record |
| `conflicts.json` | a divergence starts, changes or ends ([07 §7.6](07-capture-and-install.md)) | the divergence table | no: "what pigeon last installed" is known only here |
| `purges.toml` | a custodian first merges a purge decision, or seals it ([08 §8.6](08-custody-loops.md)) | when it received each active decision, and its seals | yes, conservatively: a lost file restarts each waiting period ([08 §8.6](08-custody-loops.md)) |
| `clock.json` | at most every second, and at exit | the Lamport clock of trace events (ADR 0008) | no, but losing it only weakens trace ordering |
| `invitations.json` | an invitation is created, used, cancelled or expires ([09 §9.6](09-network-and-joining.md)) | each secret and its hash, purpose, expiry, used flag; mode 0600 | no |
| `aside/` | install step 4 | files moved aside, kept until the first full scan 10 minutes after `done` | no ([07 §7.5](07-capture-and-install.md), [07 §7.5](07-capture-and-install.md)) |
| `stage/` | install step 2; keeping a folder ([08 §8.5](08-custody-loops.md)) | new files before they enter the folder | yes: unreferenced files are deleted at start |
| `recovered/` | install steps 6 and 9 | moved-aside content that was not pigeon's, until captured as a recovered version | no ([07 §7.6](07-capture-and-install.md)) |
| `RECOVERY.txt` (custodians, in the repository directory) | the roster changes | the group's name, the other custodians, and the git and git-annex commands that read the files without pigeon ([09 §9.9](09-network-and-joining.md), ADR 0047) | yes: written again from the roster |

The first build (13 §13.12) also keeps there `peers.json` (the addresses
peers gave at their join; the lab runs without mDNS), `p2p-token` (the token
the filter supplies, 09 §9.3), `freed.json` (folders freed here) and
`dead.json` (UUIDs already marked `dead`): rebuildable or stand-ins
**[no ADR: first build]**.

### `install.ndjson`

One JSON object per line, appended and fsynced before the step it announces:

```json
{"format":1,"seq":41,"op":"begin","path":"Photos/a.jpg","mode":"100644","old_key":"SHA256E-s…","old":[66311,4224493,2097152,1791922902880098214],"new_key":"SHA256E-s…","tmp":".git/pigeon/stage/41","tmp_ino":4224517,"aside":".git/pigeon/aside/41"}
{"format":1,"seq":41,"op":"proved"}
{"format":1,"seq":41,"op":"done"}
{"format":1,"seq":41,"op":"released"}
```

`old` is the index entry's `(dev, ino, size, mtime in ns)` that step 6
checks; the ops are those of [07 §7.5](07-capture-and-install.md) (`journal`).
Recovery at start replays every `begin` without an end, then truncates the
file once every entry has ended and its aside is released (rename of an
empty file over it). A torn last line is ignored (its step had not
started); any other malformed line stops install with a Problem.

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
stat of the person's file when `mine` was captured, which *Take the group's
version* checks ([07 §7.5](07-capture-and-install.md)); `told` records that
the person was told once (ADR 0034). The table is rewritten whole.

### `invitations.json`

```json
{"format":1,"invitations":[{"id":"inv-7Q2M","secret":"…","secret_sha256":"…","purpose":{"new-person":{"name":"Martine","admin":false}},"expires":"2026-10-14T14:10:00+02:00","used_by":null}]}
```

Expired entries are kept 30 days for the "already used / expired" messages
**[no ADR: retention]**.

## 11.4 What is kept only in memory

| Data | Rebuilt from |
|---|---|
| authority verdicts of commits | the history, by the rule of [04 §4.4](04-roster-and-authority.md) |
| version states, reasons, the headline | `main`, the git-annex location log, status reports ([06](06-status.md)) |
| the work queue | the catalogue, the location log and `wanted` ([15](15-resource-use.md)) |
| held changes | the working tree compared with `main` at each scan ([07 §7.4](07-capture-and-install.md)) |
| web sessions | none: a restart of `pigeond` asks people to open pigeon again ([10 §10.3](10-interfaces.md)) |
| peers' addresses | iroh's lookup and mDNS ([09 §9.5](09-network-and-joining.md)) |

A restart costs one pass over the history and a `git status` (ADR 0013).

## 11.5 Laws (ADR 0012)

- **Replay:** the `journal` laws (`recovery_ends_every_entry_and_a_second_does_nothing`,
  `a_crash_during_recovery_ends_as_one_recovery`,
  `nothing_the_person_wrote_is_lost`) and `CaptureInstall.tla` `Journal`.
- **Round trip:** `parse(render(x)) = x` for every file and format, and
  format N is read by every later format. Coded so far for journal records,
  roster files and invitations (`pigeond/tests/codecs.rs`:
  `journal_records_round_trip`, `roster_files_round_trip`,
  `invitations_round_trip`) and the socket protocol
  (`pigeon-proto/tests/proto_laws.rs`).
- Not yet coded as laws: **atomicity** (a crash during a write leaves the
  old value or the new one); **refusal** (a file of a newer format is never
  modified); **rebuild** (rebuilding the in-memory state from git, git-annex
  and these files gives the same states and queue).
