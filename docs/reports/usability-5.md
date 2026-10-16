# Report: first-use and recovery usability

M4 evidence for the [roadmap](../roadmap.md). On 15 October 2026 the user
explicitly accepted independent agent usability sessions as equivalent to
human sessions. Three testers used the actual CLI and Chromium web pages;
they read the README and guide before testing, without reading the
implementation. Their daemons and group folders were isolated. This is
task-based evidence, not a long-term field trial or the M7 soak.

## Tasks and retests

The first pass used the existing 0.3.1 source build. Retests used a frozen
normal 0.3.2 candidate with git-annex 10.20261006-pigeon1 and system git
2.47.3. Source-test setup must put system git before the stock standalone
archive: that archive's git wrapper chooses its own git-annex. An initial
conflict failure in a restarted stock fixture did not reproduce in two
clean patched fixtures; it is not counted as a passing product test.

| Participant and task | First finding | Retest evidence |
|---|---|---|
| First use: start, create, invite and join | A stopped daemon gave no start command | `pigeon open` started fresh personal daemons in 56 and 65 ms; browser creation took 0.7 s and browser joining worked |
| First use: see a root-level file | Files said the group had no folder and omitted the file | The root file and its version link appeared; opening history took 0.559 s |
| First use: no backup machine | Sending suggested progress toward nonexistent backups | Home and CLI asked to add a backup machine and gave the invitation command |
| Recovery: ask about a new save | An uncaptured file was reported safe | Immediate requests at 0.417 s and 0.335 s after saves reported local changes; a modified protected file said it was being checked |
| Recovery: restore an old version | Task completed in 22–34 s | The old bytes were restored and the newer bytes remained recoverable |
| Recovery: preserve both edits | Initial stock fixture was inconclusive | Two clean patched groups produced Bob's suggestion and preserved exact group and Bob bytes with `resolve --keep-copy`; stable-candidate task took 10.3 s |
| Recovery: free, keep, delete and restore | Task completed in 31 s; keep's wording preceded content arrival | Another peer kept the bytes, keeping fetched them back, and deleted-file restoration recovered exact content |
| French browser: configure and join | New machines had no join page; saved settings leaked raw keys | Actual create, invitation and fresh-machine join worked; rejected forms neither created folders nor consumed invitations |
| French browser: versions | Root files could not be reached | V1 downloaded in 0.098 s with exact UTF-8 bytes while both computers kept exact V2 bytes |
| French browser: leaving | Confirmation was understandable | The second confirmation left the local file unchanged |

## Freshness thresholds

Two testers answered scenario questions about the proposed defaults:
one hour with no recent backup contact means *Not checked recently*;
a machine silent for 24 hours stops counting; ten minutes of *Sending*
is a grace period, never evidence of safety. Both recommended retaining
these defaults provisionally. Six first-use comprehension scenarios passed,
including the distinction between stale evidence and lost content.

The guide and Settings now explain those times. The French review found
that “no news from a backup machine” could mean one silent machine rather
than no recent machine; the final wording explicitly says no backup has
been heard from. This evidence resolves M4's comprehension question for
these provisional defaults; it does not measure real warning frequency,
offline habits or transfer duration.

## Changes and remaining work

- Status checks local pending changes even when a round owns the group
  lock. A writer kept open through a status request has a regression test.
- Root files link to their versions. Browser setup can create and join,
  including a second confirmation for a refounded group. Router tests
  cover missing/forged form tokens and confirmation before any join.
- The launcher starts its adjacent daemon when needed. Explicit socket
  overrides remain controlled by their caller.
- Settings use translated labels; the guide explains starting, joining
  one's other computer, the web interface and safety freshness.

M4 is not declared complete: the optional group telemetry collector and
the two dashboards still need their full acceptance evidence, and the
remaining recovery and command gaps must be checked against the spec.
The refounded confirmation was checked by router regression tests, not by
a real refounding browser session. Keep's completion wording remains a
minor usability finding: files can still be downloading after it returns.
