# Report: interface, recovery and M4 release candidate

16 October 2026. Version 0.5.0 combines the M4 work into one pull request
based on main at c4f0278, including main's shorter CI tiers. The earlier
0.4.1–0.4.17 branches were development candidates, not published patch
releases. Their changes are included here; their obsolete CI is cancelled.
This report records implementation and local evidence. Publication still
requires the combined candidate's hosted checks and release gates.

## What people can do

Home separates current files from retained versions and uses signed backup
reports for its headline, without requiring a diagnostic collector. Files
includes unfinished local saves. The Space page distinguishes configured
storage from observed free space and explains the extra independent copy
needed for headroom. See [group health](health-5.md),
[local folder choices](local-scope-5.md) and [capacity](capacity-5.md).

English and French interfaces include nested Free/Keep, whole-folder
restoration, read-only review of proposed content, folder ownership and
steward rules, and Doctor's actual installation checks. Notifications and
the launcher were delivered with the reach candidate. The remaining
command stub is Erase, assigned to M5. See [folder commands](folder-restore-5.md),
[folder rules](folder-rules-5.md) and [installation checks](doctor-5.md).

Recovery supports paper-key authority through a surviving backup, printable
localized recovery sheets, verified refounding and in-place rejoining of
backup repositories. An offline export follows verified archives, including
deleted nested files. Recovery preserves the signed suggestion proofs
needed to judge earlier decisions. See [recovery](recovery-5.md),
[paper keys](recovery-paper-5.md), [refounding](refound-5.md),
[archive export](archive-export-5.md) and [decision history](decision-history-5.md).

Optional diagnostics use the OpenTelemetry SDK with a bounded local spool,
an authenticated collector selected in the roster, and an explicitly
configured backend. The two dashboards cover group health and operator
work; real SDK exports were queried in the dashboard backend. Peer bundles
require consent. See [observability](../../telemetry/README.md),
[operator metrics](operations-5.md),
[debug console](console-5.md) and [peer diagnostics](recovery-5.md).

## Usability and status thresholds

Five independent source-blind agent participants completed real CLI and
Chromium sessions, including two newcomers. The user explicitly accepts
agents as equivalent to people for this gate. All five correctly explained
local, shared and protected states, restored exact older spreadsheet bytes
while retaining newer history, and restored an accepted-deleted nested
folder. No incorrect safety belief was recorded. These meet the protocol's
4/5 teach-back, 4/5 recovery and zero incorrect-safety-belief criteria.

The first candidate failed proposed-content review for all five and fresh
join after decline for two. Those failures remain recorded. Two independent
focused retakes passed after repairs, including exact proposed bytes with
accepted bytes unchanged and first-attempt joins. A separate French mobile
capacity review found confusing headroom copy and horizontal overflow; its
retake correctly explained an unreported backup's configured capacity and
the need for another independent copy, with no horizontal overflow.

The 10-minute work grace, one-hour stale check and 24-hour backup exclusion
remain provisional thresholds supported by these teach-backs. This is not
a measured longitudinal warning-frequency study. Interaction times include
agent orchestration and are not estimates of human reaction time.
[The full usability report](usability-m4-5.md) preserves initial failures,
assistance, focused retakes and the limits of this evidence.

## Verification and release status

Reports above record the actual commands and results for each component,
including 1000-case algebra and protocol laws, real multi-daemon recovery,
crash-boundary rejoining, exact-byte restoration and telemetry exports.
The latest capacity interface passed all 36 English/French axe pages and
its final real three-device regression. The combined candidate retains
main's four daemon-test shards and adds every new lab regression to them.

The M3 v0.4.0 tag remains immutable. Its relay and lab checks passed, but
the release stopped before publication because the workflow called the
private service socket as root. The corrected workflow tests refusal of
another UID, then calls Doctor as the service account. Its macOS name
fixture also assumed a case-sensitive disk: the fixture now tests the
second case-colliding entry only where the filesystem can create it, and
always verifies that both admitted files retain their exact bytes. The
core portable-name laws continue to check case collisions on every host.
The local workspace cannot perform the service account switch; the hosted
check remains required. See [reach evidence](reach-4.md).

The final combined local suite also caught an older placement fixture
changing K and a custodian UUID in one roster commit. The strengthened
roster writer correctly refused it. Splitting those changes still failed:
a device's UUID is immutable. The fixture now creates the actual backup
before genesis and names its real UUID in the initial roster; the authority
rule is retained.

The combined `cargo test --workspace --features pigeond/lab --no-fail-fast`
completed with 366 tests passing and that one fixture failure, with none
ignored. After the fixture correction, the complete installation test
binary passed 16/16. That was aggregate local evidence, not an uninterrupted
green workspace run. It includes both real telemetry regressions, paper
recovery, refound crash boundaries, held deletions and portable names. Final
all-target Clippy, formatting, repository checks, actionlint and the 15
packaging regressions are also required before the rollup commit.

The first combined hosted run, 38869118021, passed the other completed
checks but found a timing assumption at the end of the two-refound test:
opening the old archived version does not mean the current version has
already downloaded. The fixture now waits, with a deadline, for the exact
current bytes too. It still requires both archive links, proposal proofs
and both versions on the fresh laptop. The superseded run is cancelled;
the updated candidate must pass on current main before merging.

The next run, 38870214779, found a real welcome race: the signed roster CAS
succeeded, then a transient `index.lock` made metadata staging fail, so the
inviter refused an already durable membership. The writer now reloads the
accepted roster immediately, acknowledges the committed membership and
retries only the roster index entry in subsequent rounds. Laptop startup
also requests this repair, covering a crash after CAS. A deterministic
locked-index regression checks the durable head and in-memory roster,
eventual exact metadata repair, and unchanged private staged/working bytes.

The user instructed us to assume the members' dogfood is done. This report
does not invent a field trial, seven nightly model passes, a 72-hour soak
or two weeks of release-candidate observation. Those M7 gates and native
Windows and other v1.1 features are outside this milestone. M5 purge and
scrub are next; ordinary deletion still retains recoverable custody copies.

Main moved again before the next run. The rollup now includes 5b03f35's
daemon conformance checks and symbolic proofs, preserving their install
observations alongside folder restore, executable modes and refound crash
replay. Main's short write-tree contention retry remains in place. The
post-CAS metadata repair handles the separate failure after membership
already became durable.

Development-only optimization of curve25519-dalek, ed25519-dalek and sha2
keeps the exact law counts. On the same local M5 foundation candidate, the
complete 14-test authority target took 620.48 seconds unoptimized and 4.23
seconds with those three dependencies optimized; its new purge law still
runs 1,000 cases. No safety check is removed, and release code already
optimizes these dependencies. This measurement concerns test execution,
not a promise about hosted runner time.

After integrating 5b03f35, the complete workspace suite passed uninterrupted:
376 tests across 100 target summaries, zero failures and zero ignored tests.
`scripts/check-driver-traces` then passed its real daemon scenarios, TLC
validation, recorded crash regressions and deliberately corrupted trace
checks. The Free/Keep trace fixture now requires both target writes to
succeed instead of discarding their results. All-target Clippy also included
the daemon's `tla-trace` feature; formatting, repository rules, actionlint
and all 15 packaging tests passed.

One returning participant then ran a focused source-blind retest on frozen,
normal 0.5.0 binaries built from staged tree
`b8498168e46a23191b8856dd143dedddbb3b8064`. This was not five new first-use
sessions. A new non-admin member invited their own backup in one attempt;
People retained the member role and listed both owned machines. The backup
joined in 0.615 seconds. The first exact file was already present at the
first content check 0.764 seconds later, while the originating laptop
continued to report not-safe until verified protection propagated, 25.766
seconds after join returned. An edit reached both machines; the backup
retained exact original and edited bytes and read-only history opened both.
A fresh browser join also succeeded once: the current file was absent at
the membership response, then exact edited bytes arrived on the next
0.502-second content-wait poll. Both historical versions opened correctly.
These timings include orchestration and are evidence of asynchronous
arrival, not promises about human task speed or network latency. The
participant also noticed the guide's stale v0.3 label, now corrected to v0.5.
The [complete focused retake](usability-current-5.md) records the remaining
wording friction and verifies that all owned processes were stopped.

Before the rollup merged, main advanced to c4f0278 with parameterized purge
proofs and verification reuse for unchanged formal inputs. The superseded
378ff5d CI jobs were cancelled and the rollup rebased again. The incoming
change affects proofs, documentation and version metadata, not application
code. Its five proof-cache identity and provenance regressions also pass.

Run 38875492801 passed the daemon shards and other completed checks but
failed the socket-alias privacy regression: with the runner's umask, the
temporary directory had mode 0755, not 0700. TempDir's default follows
umask. Both the socket alias and the private in-place join staging now
request mode 0700 at creation, avoiding a chmod window. The regression is
rerun under umask 022; the existing assertion remains unchanged.
The corrected alias check passed under umask 022, followed by all 40 daemon
unit tests and both complete refound crash/restart tests under that umask.
Final all-target Clippy and repository rules passed again. The hosted run
must validate this correction before publication.

## Publication correction after the macOS tag check

PR #45 merged with all 22 checks passing, then v0.5.0's macOS unit target
found asymmetric canonical-path comparison in configuration replacement.
That tag remains unchanged and unpublished. The remaining superseded runs
were cancelled. The [0.5.1 correction](release-051.md) reproduces the same
failure through a real directory alias on every runner, normalizes both
paths, and records the new publication candidate's validation.
