# 3. Repositories and roles

> Mission: say what git and git-annex hold on each device, how each role is
> configured, and which git and git-annex commands pigeon may and may not run.
> git-annex is the engine (ADR 0001); pigeon reaches it through its CLI
> (ADR 0004, Proposed) and reaches git through the git CLI only (ADR 0010).

## 3.1 One repository per group and device

A device that belongs to a group holds one git repository with git-annex for it.

| Role | Repository | Where (default) |
|---|---|---|
| laptop | non-bare, the working tree *is* the shared folder | chosen at join, for example `~/Famille Geiger` |
| custodian | bare: objects only, no working tree, nothing exposed to applications **[no ADR]** | `/var/lib/pigeon/<group-id>.git` |

ADR 0002 asks custodians for "locked, read-only objects, never exposed to
applications"; a bare repository is the simplest way to meet it. E9 and E10 ran
custodians as non-bare repositories with locked files; the bare layout must
pass the same experiments before release ([16](16-open-questions.md)).

## 3.2 Refs

| Ref | Holds | Written by |
|---|---|---|
| `refs/heads/main` | the accepted catalogue: a signed commit history | this device's accept and capture loops only |
| `refs/heads/git-annex` | git-annex's branch: location logs, trust, groups, preferred content, metadata | git-annex |
| `refs/pigeon/proposals/<device-id>/<commit>` | proposals, one ref per proposal commit (ADR 0009) | the proposing device; replicated by fetch |
| `refs/pigeon/peers/<device-id>/main` | the last `main` fetched from each peer, before validation **[no ADR]** | `git fetch` |
| `refs/remotes/<remote>/git-annex` | the peer's git-annex branch, merged by `git annex merge` | `git fetch` |
| `refs/pigeon/status/<device-id>` | the latest signed status report of each device ([06 §6.3](06-status.md)) **[no ADR]** | its device; replicated by fetch |

`main` only moves forward: every update is an `update-ref` with the expected old
value, to a descendant of it.

## 3.3 What is annexed

`.gitattributes`, written in the genesis commit:

```gitattributes
* annex.largefiles=anything
.gitattributes annex.largefiles=nothing
.pigeon/** annex.largefiles=nothing
```

Every file of the shared folder is annexed (key backend `SHA256E`), dotfiles
included (`annex.dotfiles=true`); pigeon's own files are plain git blobs, so the
commit signature covers them (ADR 0009). Empty folders, symbolic links,
permissions other than the executable bit, and extended attributes are not
versioned in v1 **[no ADR]** ([16](16-open-questions.md)).

## 3.4 Configuration by role

### Laptop (ADR 0002)

| Setting | Value | Why |
|---|---|---|
| `annex.addunlocked` | `true` | files are normal, editable files |
| `annex.thin` | `true` | about one copy of disk space (E8) |
| `annex.autocommit` | `false` | pigeon alone commits (E9 pitfall) |
| `annex.synccontent` | `false` | pigeon alone moves content |
| `annex.resolvemerge` | `false` | pigeon merges, with its own conflict names ([05](05-catalogue-and-names.md)) |
| `annex.dotfiles` | `true` | every file is annexed |
| `annex.jobs` | `1` | background work stays light (ADR 0037) |
| trust | `untrust` | its copies never count toward K |
| groups | `laptop` | for diagnostics |
| `wanted` | `include=<folder>/* or …` over the top-level folders kept on this device | selective sync ([08 §8.5](08-custody-loops.md)) |
| sparse checkout | the kept top-level folders and `/.pigeon/` (E15) | folders not kept are absent from disk (ADR 0031) |
| `core.fsmonitor` | `true` on macOS | faster scans (ADR 0013) |
| `core.precomposeunicode` | `true` on macOS | NFC names ([05 §5.4](05-catalogue-and-names.md)) |
| `gpg.format`, `user.signingkey` | `ssh`, the device key file | signed commits (ADR 0007) |

### Custodian (ADRs 0002, 0003, 0014, 0018)

| Setting | Value | Why |
|---|---|---|
| repository | bare ([§3.1](#31-one-repository-per-group-and-device)) | never exposed to applications |
| trust | `semitrust` (git-annex's default), never `trust` | every drop verifies it |
| groups | `custodian` | `sizebalanced` and status count this group |
| `wanted` | `sizebalanced=custodian:K and not metadata=pigeon-released=*` | partial custodians balanced by capacity; whole history (with `--all`, E9); released keys excluded after retention (E14) |
| `maxsize` | from the roster entry of the device | capacity for `sizebalanced` (ADR 0003) |
| `annex.jobs` | `cpus` | a dedicated machine |
| serving | append-only: no remote `REMOVE` (ADR 0014) | [09 §9.4](09-network-and-joining.md) |

### Group-wide (git-annex branch)

`numcopies = mincopies = K` (ADR 0002). Every device removed from the roster is
marked `dead`, so its copies stop counting and repair starts ([08 §8.4](08-custody-loops.md)).

## 3.5 Settings follow the roster, or drops stop

git-annex's shared settings live in its branch, which any device can change.
They are a *function of the roster*: for each device ID of the roster, its
trust, groups, `wanted` and `maxsize`; for the group, `numcopies` and
`mincopies`. The roster maps each device ID to its git-annex repository UUID
([04 §4.2](04-roster-and-authority.md)).

After every merge of the git-annex branch and every change of `main`, `pigeond`:

1. computes the expected settings from the roster of its `main`;
2. reads the actual ones (`git annex info --json`, `wanted`, `group`, `trust`);
3. if they differ, **suspends every drop on this device** (evict, purge,
   rebalancing) and re-applies the expected settings **[no ADR: re-apply]**;
4. resumes drops only when a fresh read matches.

Drops stay suspended while two devices with different rosters disagree; this
ends when their `main` branches converge. The status shows "checking the
group's settings" while drops are suspended (P6).

## 3.6 Remotes

Every other device of the roster is a git remote of this repository:

```ini
[remote "pigeon-3f9a0c1d2e4b"]          ; "pigeon-" + first 12 hex digits of the device ID [no ADR]
    url = p2p-annex::iroh+<device-id>   ; git-annex's generic P2P transport (ADR 0006)
    annex-uuid = <the device's git-annex UUID, from the roster>
    fetch = +refs/heads/main:refs/pigeon/peers/<device-id>/main
    fetch = +refs/heads/git-annex:refs/remotes/pigeon-3f9a0c1d2e4b/git-annex
    fetch = refs/pigeon/proposals/*:refs/pigeon/proposals/*
    fetch = +refs/pigeon/status/*:refs/pigeon/peers/<device-id>/status/*
```

- **Pull only.** A device never pushes `main` or proposals into a peer; each
  device fetches and validates for itself (P4). Content moves both ways
  through git-annex's P2P protocol (`get` downloads, `copy --to` stores).
- Remotes are created and removed by `pigeond` when the roster changes.
- Proposal refs are fetched without `+`: their names are unique and their
  targets never change.
- `git annex remotedaemon --foreground` runs per repository (ADR 0011): it
  tells peers when this device changed, so they fetch at once. E12 must show
  that it only fetches and never merges `main`.

## 3.7 Driving git and git-annex

`pigeond` keeps, per repository, one `git cat-file --batch` process and
long-lived `git annex … --batch --json` processes for the commands it issues
often (`add`, `whereis`, `metadata`, `get`, `copy`, `drop`); it parses their
JSON with serde (ADR 0004). Each call has a timeout; a failure keeps the
command, exit status and end of stderr (ADR 0023).

| Allowed | Used for |
|---|---|
| `git cat-file --batch`, `rev-list`, `for-each-ref`, `status --porcelain=v2 -z` | reading commits, trees, refs and changes |
| `commit-tree -S`, `update-ref`, `merge-tree --write-tree`, `read-tree`, `update-index`, `write-tree` | writing commits and merges without touching the working tree |
| `git fetch <remote>`, `sparse-checkout` | exchanging history; folders kept |
| `git annex add`, `calckey`, `reinject` | capture |
| `git annex get`, `copy --to` (`--key`, `--auto`, `--all`) | transfers, in pigeon's priority order (ADR 0037) |
| `git annex drop` (local only, verified), `drop --key --force` (purge and Drop only) | freeing space, rebalancing, purge |
| `git annex whereis`, `find`, `info`, `metadata`, `fsck --incremental --time-limit` | observations, scrubs |
| `git annex numcopies`, `mincopies`, `trust`, `untrust`, `semitrust`, `group`, `wanted`, `maxsize`, `dead` | settings ([§3.5](#35-settings-follow-the-roster-or-drops-stop)) |
| `git annex merge`, `smudge --update`, `restage`, `remotedaemon`, `version` | branch merge, sparse folders (E15), notifications, checks |

**Never run:** `git annex sync`, `assist` and `assistant` (they commit and
merge on their own, E9); `git annex move`, `drop --from`, `--rebalance` run
against a remote, `dropunused` (they drop from peers or outside a purge,
ADR 0014); `git merge`, `checkout`, `pull`, `reset --hard` on the working tree
(install is pigeon's, [07](07-capture-and-install.md)); `git push`.

The output of every command pigeon parses is covered by **JSON contract
tests** run against the minimum git-annex version (ADRs 0004, 0020); a new
git-annex version is supported only once they pass ([13](13-verification.md)).
