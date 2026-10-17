# English filesystem-refusal usability session

2026-10-17. Used supplied lab binaries at `/workspace/pigeon-target-files/debug`; `pigeon --version` printed `pigeon 0.6.4`. Fresh isolated home `/tmp/pigeon-fs-refusal-en.ZGT0rq/home`, English config and `LANG=en_US.UTF-8`, loopback networking with relays and mDNS off. Daemon ran under `/workspace/pigeon-debug-tools/reap-run`; PATH selected normal `/usr/bin/git` and supplied patched git-annex. I read only the permitted English Fluent message additions, not implementation source. No source edits, accepted creates, builds, git changes, or PR actions.

Command, with `PIGEON_SOCKET=/tmp/pigeon-fs-refusal-en.ZGT0rq/home/run/pigeond.sock`:

```sh
/workspace/pigeon-target-files/debug/pigeon create Family /proc/pigeon-usability-ZGT0rq/folder --k 1 --name laptop
```

Exact plain output on stderr (stdout was empty), exit 1:

```text
This folder is on a file system pigeon cannot safely use. Choose a local ext4, btrfs or xfs folder on Linux, or an APFS folder on macOS. Linux backup machines can also use ZFS.
Code: P-FILE-SYSTEM-UNSUPPORTED
```

The refusal is clear, and its next step is explicit for someone who knows the filesystem of their folders. A less technical person might not know how to find an ext4/btrfs/xfs folder; the message provides no way to check. “This folder is on” is understandable as referring to the destination even though the requested folder does not yet exist.

Preservation is not explicit: the message does not say that creating the group was refused before changes, or that existing files were preserved. In this particular session the target and its parent were absent before and after. A subsequent ordinary `pigeon status` returned “This computer is not a member of the group.” / `Code: P-NOT-IN-GROUP`, exit 1. Thus no successful group creation was claimed or observed. Because no existing destination files were present, I did not test preservation of existing files and do not infer it from this case.

The English Fluent source also contains an unknown-filesystem message telling the user to check access to the folder or parent and retry. I did not trigger that path, so this report does not validate its runtime presentation.

This is message/usability evidence only. It does not establish accepted filesystem support, hardware compatibility, or behavior on any of the filesystems named in the recommendation.

Cleanup: sent TERM only to isolated daemon PID 1707773, then waited for reap-run PID 1707766 to exit (exit 0). Process checks found neither PID nor any remaining command referencing this isolated home. Retained concise raw evidence at `/tmp/pigeon-fs-refusal-en.ZGT0rq/evidence.json` and follow-up status output at `/tmp/pigeon-fs-refusal-en.ZGT0rq/status.txt`. No other daemons were stopped.
