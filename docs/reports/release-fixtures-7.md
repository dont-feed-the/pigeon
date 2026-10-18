# Named release-fixture observations

> Mission: preserve failed release observations and correct asynchronous
> test preconditions without weakening safety checks or extending budgets.

v0.6.21, source `5239dafbe27044d00347e95983a9d280c27c2e7d`, is
immutable and unpublished. Required CI 39090818887 fails native shards 0,
2 and 3; lab 39090819012 passes. Both platform archives build, but the
publication gate refuses the failed CI. v0.6.20 remains the verified release.
No unchanged-source retry of v0.6.21 is used.

## Erasure preparation

Job 117567028097 fails the refounding fixture's immediate Preparing assertion
in 4.656 seconds. Its aggregate Protected precondition also accepts an empty
catalogue. The retained source contains both exact files and two hash-valid
objects; the backup still has only roster metadata and no objects. Its first
capture log is at 02:44:57.496 UTC. The exact refused response was not printed,
so this snapshot does not prove which prepare response occurred.

The fixture now waits for both explicitly named versions to be Protected
within the original 120-second protection budget before preparing erasure.
All preparation, confirmation, refounding and retained-byte assertions remain.
Artifact 12122877791: 4,863,892 bytes, SHA-256
`b548ea1b0b41898829c17e17e030dcc1701dc8a089a7173fbd3e2a055da8f553`.

## Fresh join after declined suggestion

Job 117567028068 fails in 68.103 seconds: after waiting for a.txt, it reads
c.txt once and observes no bytes. The retained new laptop has both accepted
paths, two hash-valid objects and exact c.txt bytes, “cid's own edit” followed
by a newline. Files install independently; observing one does not establish
that another is already installed. The same original 120-second wait now
requires both exact byte sequences. The subsequent exact-byte and empty
suggestion-list assertions remain.
Artifact 12121974203: 17,559,918 bytes, SHA-256
`72af9f97f0527cff91dc4ce2e90d56606d5511139308ae977557ef2badc83d0b`.

## Initial protection before held deletion

Job 117567028050 fails in 126.226 seconds before any deletion. Source and
backup accept all 102 main paths. Source has 102 hash-valid objects; backup
has 59 hash-valid objects. Logs show early NotListening refusals, later
closed-output errors and 30-second metadata-fetch timeouts. No high-pressure
backoff is recorded. The exact cause of incomplete protection is unproved.
The held test and its deadline are unchanged; a later passing run alone
will not establish a source-level fix for this observation.
Artifact 12123195811: 7,787,653 bytes, SHA-256
`5e5f35e8dcebbec030cade8e88446a001ab9b988db009780762fe255318433ed`.

All three original job logs and complete failed native directories are retained
outside the checkout. Neither named-path correction changes production status,
erasure authority, per-file installation, native transport or held behavior.

## Local validation

The integrated six-case real-peer run passes in 68.304 seconds under nextest,
including all three erasure cases, both held cases and the suggestion/fresh-join
case. The held lifecycle itself passes in 43.810 seconds without a source or
deadline change; that does not establish its hosted failure's cause. All-target
workspace Clippy and repository structural checks pass. Native ZFS's separate
hosted verdict remains the source-specific evidence described in its report.
