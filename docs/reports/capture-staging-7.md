# M6: bounded capture staging batches

> Mission: reduce repeated native setup without relaxing capture evidence.

A controlled staging experiment uses 4096 distinct 1024-byte files in fresh
unlocked annex repositories, including the same per-batch executable-mode
update as capture. Two alternating-order trials compare 64 and 1024 file
batches. File creation and independent verification are outside timing.
Every resulting pointer key matches its independently computed SHA-256, and
all working bytes remain identical.

| Trial | 64 files | 1024 files | Native Git calls |
|---|---|---|---|
| 1 | 31.231 s | 24.953 s | 128 versus 8 |
| 2, reversed order | 31.974 s | 25.136 s | 128 versus 8 |

The mean staging duration falls about 21%. This is an isolated small-file
measurement on the local filesystem, not the 100k-file workload, physical HDD
or a passing whole-process resource budget. The earlier full reference job
continues on its original source and configuration.

Capture now partitions input into at most 1024 files and 64 KiB of NUL input.
The byte bound also keeps executable-mode native arguments small. Oversized
single inputs stand alone, so the pure partition never loses a name or loops.
Progress tracking uses the same shared bounds and still rejects malformed,
duplicate or unterminated input; final answers must still name every expected
file exactly once with successful keys. Index proof, immutable bytes,
permissions, capture intent and commit authorization are unchanged.

Four partition laws use 1000 cases each. The native capture fixture crosses
both bounds with 1700 portable files, long UTF-8 names and executable files,
plus a nonportable newline name that must remain held locally. It checks every accepted key, working byte and immutable
annex object, then confirms a second capture adds nothing.

The initial new native fixture wrongly expected a newline-containing name to
be captured; the established portability policy correctly holds it, producing
1699 captured files out of 1700. The corrected fixture checks 1700 portable
files and separately asserts that the newline name stays held with its bytes.
No product portability rule is relaxed.

Local validation passes four thousand generated partition cases, 19 Git-driver
library cases and 123 script contracts. The corrected 1700-file native capture
passes in 20.519 seconds; the other 54 native install cases pass in the prior
combined run. Workspace/all-target Clippy passes with zero warnings.

The follow-up integrates current main
`cefa9cfc0702cbf088fae90ad6e3beceb7e7f6f3` (the v0.6.31 candidate)
before its own validation. That earlier candidate retains its independent
publication gates; no unpublished tag is treated as a completed release.

Integrated v0.6.31-base validation passes 202 daemon/Git library cases in
18.064 seconds, all 55 native install cases in 29.884 seconds, four thousand
partition-law cases and 123 script contracts. Cargo-deny checks advisories,
bans, licences and sources successfully; all-target workspace Clippy has
zero warnings. Frozen-binary preparation is exercised on the clean committed
source before the pull request is submitted.
