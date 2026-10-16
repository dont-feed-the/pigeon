# Report: portable release validation fixtures

16 October 2026. Version 0.5.5 is the next M4 publication candidate, including
the green integrated purge-proof scheduling change and the mounted peer UI.
The earlier v0.5.0, v0.5.1 and v0.5.2 tags remain immutable and unpublished.

## What stopped v0.5.2

macOS CI job 116948328578 in run 38893485561 failed the capacity and folder
rule tests at joining Bob. Their daemon home was `bob`, and their proposed
file folder was `Bob`. On the runner's case-insensitive disk these are the
same nonempty directory; the production join correctly refused it. The
folder-rule fixture had the same later `cara`/`Cara` collision. The file
folders now use `bob-files` and `cara-files` on every runner.

The same job's restore test read local telemetry immediately after reading
the catalogue, before a completed SDK batch was necessarily exported. It
now waits up to twenty seconds for the same required capture/round parent
relationship, without forcing an export or weakening that assertion.

macOS workspace job 116948328626 found seven bridge unit tests stopped by
`state_dir is too long for portable Unix socket paths`. The default macOS
temporary directory made the fixture exceed that limit. The shared bridge
fixtures now use short private temporary directories under `/tmp`. The
production path validation still rejects excessive paths.

## Evidence and pending gates

The three real-daemon capacity, folder-rule and restore tests pass locally
with matching lab binaries: 27.02, 11.85 and 8.24 seconds. All-feature
workspace Clippy and repository checks pass. Bridge unit fixture validation
and the next current-main PR/tag checks are recorded as they complete.

The previous tag passed lab and cross-runner reach, but its failed and
cancelled workflows do not establish release readiness. Successful fresh
macOS validation and the actual service-user/install/uninstall proof are
still required before publication. No elapsed soak or nightly streak is
claimed by this report.
