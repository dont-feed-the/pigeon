# M6: native hints for background capture

> Mission: record event admission, fallback evidence and remaining M6 gates.

## Behavior and boundary

Linux inotify and macOS FSEvents now wake background capture. Hints carry a
saturating revision, never a catalogue or an authorization decision. Startup,
new hints, overflow/rescan, settling deadlines and the ten-minute bound admit
the existing full metadata/capture observation. A successful observation
acknowledges only its starting revision, preserving events received during it;
a failed observation acknowledges nothing and retries after five seconds.
Explicit status observations remain fresh. Writer-busy files without a
settling deadline get a thirty-second revisit.

Linux registers user directories without following symlinks or watching the
root `.git` and `.pigeon` metadata trees. Directory creation, removal and
rename refresh registration before another full observation. Old kernel
registrations are released before replacements, avoiding doubled watch use.
Registration runs outside the group state lock and asynchronous worker.
Missing roots and native errors fall back to ordinary scans and sixty-second
registration retries. One backend failure wakes fallback once. Overflow wakes
a full observation; a Linux overflow also refreshes directory registration.
No event path or native error detail enters the public notice or diagnostics.

The command line and Health page show an optional translated scan-only notice
without changing the safety headline or any local-byte proof. It says that
native watching is unavailable, checks continue and retry is automatic.

## Laws and native evidence

Six `change_scan` laws run 1000 cases each: startup, periodic observation
despite lost/duplicate hints, concurrent revisions, settling/fallback,
saturation and backwards time. Native fault cases cover overflow on an
otherwise ignored access, repeated backend failure, read versus close-write,
and root loss first observed through a queued read-access event. That last
race initially failed the native root-removal case: it invalidated the watcher
without waking fallback. The corrected path wakes even when the triggering
access would normally be ignored.

A real-daemon regression sets ordinary rounds to 600 seconds, waits for an
initial empty scan, saves a file and polls Git directly without invoking
status. Published v0.6.30 fails in 61.652 seconds. With native hints it passes
in 3.910 seconds, including pointer and annex-content byte checks. A second
case creates a directory after startup, waits for Linux registration refresh,
then saves inside it; it passes in 3.148 seconds. A Linux `/proc/self/fdinfo`
fixture proves exactly three kernel watches for root/user/leaf, excluding a
deep annex metadata tree and an external directory symlink.

The final Linux validation passes 46 system/native cases (one manual test
ignored), 183 daemon library cases and 56 native install/watch cases. All
workspace/all-target Clippy checks pass with zero warnings. Earlier validation
also passes 135 protocol/i18n/web cases, six thousand scan-law cases and 120
script contracts. Translated wording is checked again before committing.

## Simulated usability

The user authorizes agent usability sessions as equivalent to people. Existing
English and French newcomer agents independently reviewed the notice as
wording-only sessions on 18 October. Both understood automatic continued
checks and inferred no immediate detection or protection promise. The English
review requested clearer explanation of fallback; the final bilingual text
adds that native watching is unavailable and retries automatically. These
sessions establish no timing, resource or elapsed-soak evidence.

## Remaining gates

This is partial ADR 0013 implementation. The existing authoritative
metadata-only candidate observation differs from the ADR's literal
`git status --porcelain=v2 -z` observer. macOS built-in Git fsmonitor remains
open. Accepted ADR 0013 is unchanged. Other callers, including telemetry,
can still request metadata observations between background capture scans;
this change does not establish a whole-process ten-minute scan cadence.

The full reference profile already running on the preceding shared-inventory
source remains a separate baseline. No passing 100k-file CPU/RSS window,
physical-HDD budget, 72-hour soak, seven actual scheduled model nights or
release-candidate dogfood is inferred from these finite tests. See
[resources](resources-7.md) and the [roadmap](../roadmap.md).

## First hosted validation: explicit held decisions

Original PR head `7fb385656da6e2c37d2f3136f028d84fea42d42d`, CI
39111806052, fails native shards 2 and 3 after a held deletion is explicitly
shared. Folder restore waits 60 seconds for accepted deletion; the mass-delete
case waits for the shared deletion to reach main. Both retained logs show a
successful `held-released` Share after the last filesystem deletion event,
with no later captured deletion. The native watcher cache had acknowledged
that event while deletion was still held; internal approval creates no user
filesystem event. This is a source regression, not a timing flake.

The correction invalidates capture observation after successful held choices
before waking the next round. The original assertions and deadlines remain.
Original artifacts 12130817884 (9,968,244 bytes, SHA-256
`dbec2f3fd546b4a548f515ede6d3537089c6bd4b1f98b3489e825198318799fa`)
and 12130996741 (8,219,362 bytes, SHA-256
`f7bb118a73975dfe52a49612030fa6fcfd88cb8dd7b52e265a1550fa149545dd`)
are retained. No unchanged-source CI retry is used.

Corrected local native validation passes all five held, folder-restore and
watch cases in 24.811 seconds; the formerly failing folder-restore case passes
in 15.234 seconds and the mass-delete case in 24.811 seconds. Workspace
all-target Clippy again passes with zero warnings.
