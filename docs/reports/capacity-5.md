# Storage people can account for

M4 follow-up, 0.4.17, 16 October 2026. `pigeon status --capacity` and the
Space page use the same structured response. Files and Settings link to it.

The accounting takes one immutable reading of admitted retention refs.
Each distinct key counts once, in current files before accepted history
before suggestions. Required storage multiplies each part by K using the
existing core capacity algebra and its laws. Impossible size sums return a
problem instead of overflowing. Configured capacity follows the core's
one-copy-per-custodian limit. Each device's used/free/limit/reserve numbers
keep their signed report's time and freshness; missing reports remain
explicit. These figures are observations, not extra evidence of safety.

Open, accepted, declined and withdrawn retained suggestions include their
signed id, age and additional retained bytes before copies. Shared bytes
can appear in several rows and count once in the totals. Declining alone
never releases content. Purge and erase remain the next milestone.

Extra custody copies use one bounded `whereis --batch-keys --json` process
per 256 keys, with a five-second total observation window. Missing rows,
unreadable output or a timeout leave the figure unknown. This is a read-only
observation, separate from retention, transfer admission and Drop. The batch
behavior is from `Command/Whereis.hs` and `CmdLine/Batch.hs` in the pinned
git-annex 10.20261006.

The real three-device regression passes in 22.35 seconds. It proves that
two paths sharing a key count once, a replaced/deleted version stays in
history, an ordinary member's declined suggestion still consumes space,
one K=1 custodian has zero observed overflow, and its disk report retains
its time. The actual Space pages show the declined suggestion in English
and French and preserve the exact accepted working bytes and deletion.
The structured response round-trips. Protocol laws (1000 cases), both
language bundles, web and CLI tests and all-target clippy pass. The final
regression passes again in 24.80 seconds, with the revised wording and
structured `configured` field. Axe checks all 36 mockup/real EN/FR pages,
including Space, with zero violations; final clippy and repository checks
pass.


## Independent usability

A returning French-speaking participant used frozen normal 0.4.17 binaries,
without source or implementation-report access. They independently found
Space through Settings in about 31 seconds, read all three storage parts,
found a real declined suggestion's age and 16,384 additional bytes, and
correctly observed that declining left the budget unchanged. An unreachable
second backup had no report and no invented zero disk-space value.

The first candidate failed two usability checks. Its warning asked for more
storage despite showing gigabytes free, and its configured aggregate grew
when an unreported backup joined without explaining why. A 40-character
suggestion id also made a 390px page 412px wide. These original findings and
screenshots were preserved, rather than counted as passes.

A separately frozen second candidate explicitly distinguishes configured
capacity from measured disk reports, explains that a computer counts for at
most one independent copy, and explains the 25% headroom request. The same
two-backup fixture remained unchanged and its unknown report remained
explicit. The participant independently explained the aggregate correctly.
A separate genuine one-backup fixture exercised the revised warning beside
6,536,470,528 reported free bytes, an unlimited allocation and 5 GB reserve:
they understood why another backup computer would add independent capacity
while free disk space on the existing one would not. Both pages measured
390px at a 390px viewport after wrapping long ids. These focused retests
passed; they do not relabel the original failures. All 38 original and 37
second-candidate owned process identities were independently verified gone.
Detailed private evidence is retained outside the repository.
