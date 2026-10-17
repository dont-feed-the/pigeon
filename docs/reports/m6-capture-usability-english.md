# Independent English CLI file-capture agent usability session

Date: 2026-10-17, approximately 15:13–15:15 UTC. Supplied binary: `/workspace/pigeon-target-files/debug/pigeon`; `--version` printed `pigeon 0.6.3`. The parent identified this build as PR #72, source head `f1c147f90a58c941d3b356638241f56be8bfe6c5`; I did not inspect source or independently verify its commit. I used the supplied patched git-annex, normal system git, a fresh isolated temporary home, and a daemon supervised by `reap-run`.

This session assessed normal CLI understanding and visible behavior. It did not measure capture deadlines, throughput, CPU/memory use, hardware behavior, or remote backup correctness. I did not read implementation source, build anything, invoke direct git mutations, alter repository files, replace binaries, or affect existing daemons. The product created its own group repository inside the isolated temporary directory through its normal create command.

## User documentation and discovery

Read README and `docs/guide.md` from `/workspace/pigeon-m6-files`, the PR #72 checkout indicated by the parent. I initially read the corresponding user docs in `/workspace/pigeon` before the checkout location was clarified. I also read `/workspace/pigeon/AGENTS.md` for applicable workspace instructions. No implementation files were read.

Ran `/workspace/pigeon-target-files/debug/pigeon --help`. It lists create, status, history, view, settings, and other everyday commands, but no capture/save-now command. The guide says “Drop a file in the shared folder and it reaches the others.” I therefore expected normal file saves to be picked up automatically; I did not try undisclosed/internal capture actions. The guide's explicit distinction between local/published/protected made “Only on this computer” understandable.

Minor documentation confusion: the PR #72 guide says “Status: alpha (v0.5)” while the CLI reports 0.6.3. This did not block the task. The guide also lacks a user-facing explanation of whether every rapid save becomes a history version; this became a question during rapid-save tasks below, not evidence of lost final content.

## Setup and exact command conventions

Fresh working root: `/tmp/pigeon-m6-capture-en.toZcgN` (created with `mktemp -d /tmp/pigeon-m6-capture-en.XXXXXX`). All saved user files, group state, read-only views, and session helper scripts were under that root. The final report is this file under `/tmp`.

Used the following commands:

```sh
PATH=/usr/bin:/workspace/pigeon-patched/bin:/bin command -v git-annex
# /workspace/pigeon-patched/bin/git-annex
mkdir -p /tmp/pigeon-m6-capture-en.toZcgN/home/config /tmp/pigeon-m6-capture-en.toZcgN/Family
PATH=/usr/bin:/workspace/pigeon-patched/bin:/bin /workspace/pigeon-debug-tools/reap-run /workspace/pigeon-target-files/debug/pigeond --home /tmp/pigeon-m6-capture-en.toZcgN/home
```

Before launch, created this isolated configuration using an ordinary text file write:

```toml
format = 1
language = "en"
[network]
bind = "127.0.0.1:0"
mdns = false
relays = "off"
```

Every subsequent pigeon command used this environment variable:

```sh
PIGEON_SOCKET=/tmp/pigeon-m6-capture-en.toZcgN/home/run/pigeond.sock
```

Commands below use the full CLI path; that environment variable was supplied to every invocation. Ran `pigeond --help` only for launch discovery; it printed `pigeond: usage: pigeond [--home DIR | --service]`. A setup attempt to run `reap-run --help` produced a Python FileNotFoundError for `--help`; reap-run is an external session cleanup helper, so I did not classify that as a pigeon product failure.

## Tasks and observations

1. Create a small group:

```sh
/workspace/pigeon-target-files/debug/pigeon create Family /tmp/pigeon-m6-capture-en.toZcgN/Family --k 1 --name en-laptop --person Alex
/workspace/pigeon-target-files/debug/pigeon status
```

Create succeeded and printed “The group ‘Family’ is created” with an installation/join line and a clear warning about who can use it. I did not share the invitation. Initial status said exactly: “No files yet. Add files to your shared folder to get started.” The next step was clear.

2. Ordinary save:

Wrote `notes.txt` using Python `Path.write_text('Shopping list\nMilk\n')` as a simple stand-in for an editor save. Ran these commands immediately and again after an ordinary short wait:

```sh
/workspace/pigeon-target-files/debug/pigeon status
/workspace/pigeon-target-files/debug/pigeon status /tmp/pigeon-m6-capture-en.toZcgN/Family/notes.txt
/workspace/pigeon-target-files/debug/pigeon history /tmp/pigeon-m6-capture-en.toZcgN/Family/notes.txt
```

Status listed the file. Its exact visible guidance was:

```text
Not safe yet: add a backup machine.
  notes.txt: Only on this computer
    0 of 1 backup machines are set up. To add one, run on it the line pigeon invite prints, with --backup-machine at its end.
```

History listed `1. just now: Only on this computer`. This is an honest and actionable message for a one-computer group with K=1. I could tell the file had been noticed, and that having it locally was not a safe backup. All commands exited 0; their prose, rather than exit status, conveys that protection is incomplete.

3. Repeated saves with status after each:

Saved the same note four times, appending Bread, Eggs, Tea, then Coffee, with a path status call after each save. Afterward ran history and status again. History listed versions 5 through 1, all “Only on this computer”. No command failed. The capture behavior felt automatic; no manual capture step was needed.

4. Rapid repeated saves without asking status in between:

Wrote `draft.txt` 12 times about 0.1 seconds apart with content `Draft revision N\n`, then stopped writing. This modeled rapid repeated saves; the spacing was not treated as a performance requirement. Ran:

```sh
/workspace/pigeon-target-files/debug/pigeon status
/workspace/pigeon-target-files/debug/pigeon history /tmp/pigeon-m6-capture-en.toZcgN/Family/draft.txt
# repeated path status and history after a short wait
/workspace/pigeon-target-files/debug/pigeon status /tmp/pigeon-m6-capture-en.toZcgN/Family/draft.txt
/workspace/pigeon-target-files/debug/pigeon history /tmp/pigeon-m6-capture-en.toZcgN/Family/draft.txt
/workspace/pigeon-target-files/debug/pigeon --json view /tmp/pigeon-m6-capture-en.toZcgN/Family/draft.txt
```

History showed one retained version, both immediately after the series and later. The normal view command returned a read-only cache path ending in `draft (version 1).txt`; read that user-facing view artifact with `cat`. It contained exactly `Draft revision 12` followed by a newline. Thus the latest content was retained in the visible version. The observations do not establish that all intermediate saves must be retained. A user expecting undo for every press of Save could be surprised by one version, but the user documentation I read did not promise that granularity. I do not call this data loss.

5. Status while edits were still being made, and after settling:

Saved `working.txt` six times about 0.3 seconds apart with content `Work in progress N\n`. During that series, ran three path status calls about 0.4 seconds apart, then status/history after the writes had stopped and a short wait:

```sh
/workspace/pigeon-target-files/debug/pigeon status /tmp/pigeon-m6-capture-en.toZcgN/Family/working.txt
/workspace/pigeon-target-files/debug/pigeon history /tmp/pigeon-m6-capture-en.toZcgN/Family/working.txt
/workspace/pigeon-target-files/debug/pigeon --json view /tmp/pigeon-m6-capture-en.toZcgN/Family/working.txt
```

All three in-progress status calls and the later one said “Only on this computer” and gave the same add-backup next step. History eventually listed four versions. The latest read-only view artifact contained exactly `Work in progress 6` with a newline, matching the final saved content.

I did not encounter a distinct “settling”, “capturing”, or “still changing” message in these small-file interactions. That is an observational limit, not evidence that a pending-state message is broken: the files may have been captured before each status answer. These checks cannot establish how a large, continuously written file or an actually pending capture is explained.

6. Checked the discoverable settings:

```sh
/workspace/pigeon-target-files/debug/pigeon settings
/workspace/pigeon-target-files/debug/pigeon --version
```

Settings listed Gentle, automatic upload/download, Wait on battery/metered, threshold 20, no full-speed windows, and integrity-check window 01:00–06:00. There was no capture action in these normal settings. I made no resource-setting changes in this session.

## User understanding and visible-claim assessment

Automatic pickup worked for the observed small ordinary files. The file names appeared in status, history was accessible, and the latest readable saved versions matched the final rapid-save bytes. Every observed file status clearly said it was not safe because a backup machine was missing. No observed command falsely claimed remote protection. The next step—add a backup using the invitation line and `--backup-machine`—was concrete.

No blocking capture usability failure was observed in this scope. Remaining questions are history granularity during rapid saves and how pending/settling capture would be explained, because those were not directly answered by documentation or encountered as an explicit status in this session. A backup machine was not added, so this session does not test sharing completion or a transition from protected old content to an unprotected current save.

## Cleanup

The isolated daemon was launched under `reap-run`. Its observed PID was 3017446 and the supervisor PID was 3017441. Sent `kill -TERM 3017446` and waited for the reap-run command to exit; it exited 0. Afterwards ran:

```sh
ps -p 3017441,3017446,3017644 -o pid,ppid,args
pgrep -af '/tmp/pigeon-m6-capture-en.toZcgN/home'
```

The first command listed no remaining process rows; the second found no matches (exit 1). PID 3017644 had been the isolated annex helper visible under the same temp home, and it was also gone. Left the isolated files and transcript for review, without deleting any group state or touching other daemons. Detailed timestamped command/output transcript: `/tmp/pigeon-m6-capture-en.toZcgN/transcript.txt`. Save helper scripts and their exact saved contents are retained under the same root.
