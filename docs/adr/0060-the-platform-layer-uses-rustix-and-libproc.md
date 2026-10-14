# 0060. The platform layer uses rustix, with libproc on macOS; the lab seams are a self-kill and a file

- **Status:** Proposed, 14 October 2026, by the agent, for issue #17
  (Guillaume, in chat: "work on a first version of the implementation even if
  not all details are figured out yet"). Settles the `rustix` choice of
  14 §14.7 and the directories of 11 §11.2, both marked **[no ADR]**.

## Context

`pigeon-sys` is the only crate that may hold `unsafe` (ADR 0058). It must
rename without replacing (ADR 0040), make journal lines durable (11 §11.3),
find the writers of a file (ADR 0040's writer scan), pause children
(ADR 0037), read I/O pressure, name failure domains (ADR 0049) and carry the
two lab seams of ADR 0057. Several details were left open: the system-call
crate, how a scan reports what it could not see, the byte encoding of a
fingerprint, the runtime directory on macOS, and the exact form of the seams.

## Decision

1. **`rustix` 1.1 for system calls**: `renameat2` (`RENAME_NOREPLACE`,
   `RENAME_EXCHANGE`) and `renamex_np` (`RENAME_EXCL`, `RENAME_SWAP`),
   `fsync` and `F_FULLFSYNC`, `posix_fadvise` and `F_NOCACHE`, `kill` and
   `killpg`. Its safe API keeps `unsafe` to the macOS writer scan. **`libc`
   0.2, on macOS only**, for `libproc` (`proc_listallpids`,
   `proc_pidinfo`, `proc_pidfdinfo`). The three structures `libc` lacks
   (`proc_fileinfo`, `vnode_fdinfowithpath`, `proc_regionwithpathinfo`) are
   declared in `pigeon-sys` from `<sys/proc_info.h>`, and each `unsafe`
   block states its `SAFETY:` reason.
2. **The writer scan** returns the pids that may write the file and the
   number of processes it could not inspect. On Linux, a descriptor whose
   target has the file's `(dev, ino)` is a writer unless its `fdinfo`
   access mode is read-only; a line of `maps` that maps it `rw-s` is a
   writer. On macOS, a vnode descriptor with `FWRITE`, or a region with
   write protection that `MAP_SHARED` made, which XNU marks
   `VM_INHERIT_SHARE` (`bsd/kern/kern_mman.c`). The region's share mode
   cannot tell: a lone shared mapping reports `SM_PRIVATE`. Install treats a scan with
   uninspected processes as clear only if the file gives no write
   permission to group or others; otherwise the path waits (`in-use`).
3. **Directories.** As 11 §11.2, through `dirs`. The runtime directory is
   `$XDG_RUNTIME_DIR/pigeon` on Linux; macOS has none, so it is
   `<cache>/run`. A backup machine uses `/var/lib/pigeon`,
   `/var/log/pigeon`, `/var/cache/pigeon` and `/run/pigeon`. The socket is
   `pigeond.sock` and the lock `pigeond.lock`.
4. **Fingerprints.** A fingerprint is the first 16 bytes of SHA-256 over
   `pigeon-domain`, the group ID and the identifier, each followed by a zero
   byte, so no two triples share an input. Identifiers carry their kind
   (`wwn:`, `serial:`, `share:`, `fs-uuid:`), so a serial never equals a WWN
   by accident. A repository on `tmpfs` or `ramfs` is an error. In the slice,
   macOS reports its host (`IOPlatformUUID`) and no storage, since
   custodians run on Linux.
5. **The lab seams**, behind the `lab` feature of `pigeon-sys` only:
   - `PIGEON_LAB_CRASH=point:n`, with a point name of letters, digits, `.`,
     `-` and `_`, and `n` ≥ 1. `pigeond` checks it at start and refuses a
     malformed plan. The n-th crossing of the point sends `SIGKILL` to its
     own process, so no destructor runs and no buffer is flushed, as with
     `kill -9`. One point per run.
   - `PIGEON_LAB_FINGERPRINTS` names a JSON file
     `{"storage": [...], "host": "...", "virtual": ""}`, with `null` for
     unknown, which replaces probing.
   - A release build ignores both variables.
6. **`cargo deny`** checks licences, advisories and sources on Linux in
   the `rust` job. The licence list is permissive. MPL-2.0 is allowed for
   `option-ext` alone, which `dirs` needs through `dirs-sys`: its copyleft
   covers its own files only.

## Consequences

- The macOS scan rests on structure layouts copied from a system header,
  and a wrong layout would make writers invisible. Its CI test therefore
  opens and maps a file and expects to see its own writer.
- A person's file that others may write waits whenever the scan cannot see
  every process, which may delay an install on shared machines.
- Crash schedules (S9) kill exactly as a power cut kills a process.
  Unflushed buffers of `pigeond` are lost, as they would be in the field.
- A changed fingerprint encoding would split domains, so it is fixed here
  before any report is written.
