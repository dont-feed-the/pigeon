# Independent English agent CLI capture retake — corrected build

Date: 2026-10-17, approximately 15:23–15:25 UTC. The parent supplied rebuilt PR #72 head `7951c520c9a8e84fc364e686cdea8f6507e207ea` at `/workspace/pigeon-target-files/debug`. `pigeon --version` still printed `pigeon 0.6.3`; the source-head identification comes from the parent, not source inspection.

This is a new isolated usability session. It does not treat the earlier `f1c147f...` session as evidence for the corrected status behavior. I did not read implementation source, perform engineering review, build, invoke git mutations, edit repository files, or affect other daemons. I used normal CLI/status/history/view and ordinary small-file saves, without throughput, hardware, HDD, or timing-budget claims. Earlier user-guide/version and history-granularity wording issues are being handled separately; I did not retest their documentation fixes.

## Setup and commands

Created `/tmp/pigeon-m6-capture-en-retake.LTGI0A` with `mktemp -d /tmp/pigeon-m6-capture-en-retake.XXXXXX`. Created `home/config` and `Family` beneath it. Isolated config:

```toml
format = 1
language = "en"
[network]
bind = "127.0.0.1:0"
mdns = false
relays = "off"
```

Launched under the required supervisor:

```sh
PATH=/usr/bin:/workspace/pigeon-patched/bin:/bin /workspace/pigeon-debug-tools/reap-run /workspace/pigeon-target-files/debug/pigeond --home /tmp/pigeon-m6-capture-en-retake.LTGI0A/home
```

All CLI commands below supplied `PIGEON_SOCKET=/tmp/pigeon-m6-capture-en-retake.LTGI0A/home/run/pigeond.sock` in the environment. The CLI path was `/workspace/pigeon-target-files/debug/pigeon` throughout. Exact timestamped invocations and output are retained in `/tmp/pigeon-m6-capture-en-retake.LTGI0A/transcript.txt`, with the helper that logs ordinary saves and invokes the CLI at `session.py` in that directory.

Created a one-computer group:

```sh
/workspace/pigeon-target-files/debug/pigeon create Family /tmp/pigeon-m6-capture-en-retake.LTGI0A/Family --k 1 --name en-laptop --person Alex
/workspace/pigeon-target-files/debug/pigeon status
```

Create succeeded and printed the invitation/install line; I did not share it. Initial status said “No files yet. Add files to your shared folder to get started.” No backup machine was added.

## Ordinary save: pending became visible

Saved `notes.txt` with exact bytes `Shopping list\nMilk\n` using Python `Path.write_text`, standing in for an editor save. Immediately ran:

```sh
/workspace/pigeon-target-files/debug/pigeon status
/workspace/pigeon-target-files/debug/pigeon status /tmp/pigeon-m6-capture-en-retake.LTGI0A/Family/notes.txt
/workspace/pigeon-target-files/debug/pigeon history /tmp/pigeon-m6-capture-en-retake.LTGI0A/Family/notes.txt
```

Both status commands returned this distinct pending explanation:

```text
Checking 1 changed file; it is not safe yet.
  notes.txt: Only on this computer
    This change is waiting to be saved and copied to the backup machines.
```

History returned “This change is waiting to be saved and copied to the backup machines.” It did not yet list a saved version.

After an ordinary short wait, ran status and history again. Status changed to:

```text
Not safe yet: add a backup machine.
  notes.txt: Only on this computer
    0 of 1 backup machines are set up. To add one, run on it the line pigeon invite prints, with --backup-machine at its end.
```

History now listed `1. just now: Only on this computer`. I could distinguish a detected change still being checked from a retained local version that lacks backup protection. All observed commands exited 0; the human-readable text communicates safety state.

## Repeated saves with status between them

Saved `notes.txt` four more times, successively adding Bread, Eggs, Tea, then Coffee. After every save ran:

```sh
/workspace/pigeon-target-files/debug/pigeon status /tmp/pigeon-m6-capture-en-retake.LTGI0A/Family/notes.txt
```

Every intervening status continued to say “Checking 1 changed file; it is not safe yet” and “This change is waiting to be saved and copied to the backup machines.” An immediate history call listed the original version 1 and appended the pending explanation. This is materially different from the earlier session, in which status between small saves was followed by immediately listed saved versions.

After writes stopped and an ordinary short wait, status showed the add-backup guidance, and history listed versions 2 and 1. Ran the normal view command:

```sh
/workspace/pigeon-target-files/debug/pigeon --json view /tmp/pigeon-m6-capture-en-retake.LTGI0A/Family/notes.txt
```

It returned a read-only view path ending in `notes (version 2).txt`. Read that view artifact. Its exact bytes were:

```text
Shopping list
Milk
Bread
Eggs
Tea
Coffee
```

The final newline was present. These bytes matched the final save. I do not infer that every intermediate edit must have its own version.

## Rapid-save burst

Saved `draft.txt` 12 times about 0.1 seconds apart with content `Draft revision N\n`, without status calls between those saves. The spacing modeled rapid repeated saves, not a required capture deadline. Immediately after the burst ran:

```sh
/workspace/pigeon-target-files/debug/pigeon status
/workspace/pigeon-target-files/debug/pigeon history /tmp/pigeon-m6-capture-en-retake.LTGI0A/Family/draft.txt
```

Status said “Checking 1 changed file; it is not safe yet.” It listed the already captured note with add-backup guidance, and the new draft separately with “This change is waiting to be saved and copied to the backup machines.” History gave the pending explanation and no draft version yet. The two per-file explanations helped distinguish the outstanding work.

After stopping writes and allowing ordinary completion, status switched to add-backup guidance for both files. History listed draft version 1. Ran:

```sh
/workspace/pigeon-target-files/debug/pigeon --json view /tmp/pigeon-m6-capture-en-retake.LTGI0A/Family/draft.txt
```

The returned read-only artifact `draft (version 1).txt` contained exactly `Draft revision 12\n`. Thus the final burst content was retained in the version visible to a user.

## Understanding and remaining ambiguity

The pending versus retained-local distinction is now directly observable in this corrected build. “Checking …; it is not safe yet” made the temporary work clear. Once capture completed, the explicit missing-backup count and invitation instructions gave a concrete next step. No observed state claimed safety or backup completion with zero backup machines.

Minor wording ambiguity: I had already saved in my editor, so “waiting to be saved” could momentarily suggest that my local Save failed. In context I understood it as pigeon still saving the version for sharing, because the file was listed as local and the headline said Checking. No manual-capture action was needed; waiting after editing stopped led to accessible history and correct final bytes. This did not block the session.

This retake does not establish correctness for protected-old-version transitions, remote copying, large/hot-file runtime behavior, or capture scheduling internals. It establishes only the visible pending-to-local behavior and final readable bytes observed here. Ordinary completion waits were observations, not performance guarantees.

## Cleanup

Only this isolated daemon was stopped. Its daemon PID was 3026413, supervised by reap-run PID 3026408; an isolated annex helper had PID 3027418. Ran `kill -TERM 3026413` and waited for the reap-run command to finish; it exited 0. Then ran:

```sh
ps -p 3026408,3026413,3027418 -o pid,ppid,args
pgrep -af '/tmp/pigeon-m6-capture-en-retake.LTGI0A/home'
```

No process rows or matching processes remained. Left the temporary files, readable views, and transcript for review. No other daemon or shared binary was changed.
