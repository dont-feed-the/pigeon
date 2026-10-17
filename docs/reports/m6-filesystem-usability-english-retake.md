# English filesystem-refusal wording retake

2026-10-17. Fresh isolated home `/tmp/pigeon-fs-refusal-en-retake.8yI8ZW/home`; supplied rebuilt CLI printed `pigeon 0.6.4`. English language config and `LANG=en_US.UTF-8`; loopback bind, relays and mDNS off. Daemon ran through `/workspace/pigeon-debug-tools/reap-run`, using normal system git and supplied patched git-annex.

With `PIGEON_SOCKET=/tmp/pigeon-fs-refusal-en-retake.8yI8ZW/home/run/pigeond.sock`, ran:

```sh
/workspace/pigeon-target-files/debug/pigeon create Family /proc/pigeon-usability-retake-8yI8ZW/folder --k 1 --name laptop
```

Exact stderr, exit 1 (stdout empty):

```text
This folder is on a file system pigeon cannot safely use. This check does not change your files. Choose a local ext4, btrfs or xfs folder on Linux, or an APFS folder on macOS. Linux backup machines can also use ZFS.
Code: P-FILE-SYSTEM-UNSUPPORTED
```

The added sentence clearly reassures me that this check leaves my files unchanged. It directly addresses the preservation uncertainty from the first session. The refusal and next step remain understandable.

Read the changed user-guide paragraph. It now gives a concrete way to identify the filesystem: `df -T /path/to/folder` and its Type column on Linux, using an existing parent for a new folder; Disk Utility's Format on Mac. This addresses my earlier question about how a user finds the named filesystem types. I did not test those platform instructions or accepted filesystem behavior.

The requested /proc destination and parent remained absent afterward. No existing-file preservation, filesystem compatibility, hardware behavior, or accepted-create/annex behavior was tested. No source edits or engineering review were performed.

Cleanup: terminated only daemon PID 1708503; reap-run PID 1708498 exited 0. Process checks then showed neither PID nor any remaining command referencing this isolated home. Raw command/output evidence retained in `/tmp/pigeon-fs-refusal-en-retake.8yI8ZW/evidence.json`. The first report remains separate and unchanged.
