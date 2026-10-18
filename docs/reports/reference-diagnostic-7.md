# M6: separate diagnostic seeding from budget qualification

> Mission: reach a full reference dataset for diagnosis without relabelling a gate.

The ordinary reference profile still uses default settings throughout. Its
100000 files, 50GB, two real peers, every-object hashes, complete ten-minute
window, descendant-inclusive CPU, daemon RSS high-water and full-scan checks
are unchanged. The default workflow input remains false.

An explicit `--diagnostic-fast-prepare` option sets the software resource
preset to Fast during preparation. Linux service properties remain unchanged.
After both accepted trees and every object are independently verified, the
public CLI restores Gentle on both peers. Readback must contain exactly one
matching Gentle setting before sampling; a reload grace follows. Both PIDs
stay alive, so preparation memory peaks remain in their RSS high-water marks.
There is no CLI/status/Git polling during the measurement window.

These runs always report top-level and backup `budget_verdict: not-measured`,
including when every numeric check passes. Their raw results remain under
`diagnostic_budget_verdict`, alongside CPU/RSS/scan numbers. The job name and
qualification field explicitly say diagnostic. They cannot supply an M6 pass.
A failed preparation, failed byte proof or failed preset restoration still
fails the run. This option does not establish default seeding throughput or
prove that the original bottleneck is fixed.

End diagnostics retain process state (`T` stopped, `D` I/O wait, `R` running,
and the other bounded kernel states) beside fenced CPU ticks and RSS. A single
state snapshot does not establish a complete duty cycle or the exact cause of
a past stall. Private names, command lines and credentials remain excluded.

The optional workflow input is `reference_fast_preparation=true` with
`native_reference_only=true`. On an owned suitable host:

```sh
scripts/reference-budget --bin /absolute/programs --root /absolute/new-fixture \
  --report /absolute/report.json --diagnostic-fast-prepare
```

## Previous full-size observations

The ordinary shared-inventory source
`0e345d81ea6c114e7405a29f8f2a24e935b97ca0` expires its 5400-second
preparation limit with 65,600 unverified source objects and no accepted
capture. Its [retained report](writer-inventory-7.md) records the exact scope.

Newer source `9ad54f6a1be11d979ccac876865c18bb7bac6413`, run
39115625323/job 117640710858, expires its 600-second preparation limit at
09:48:38 UTC on 18 October. It has 16,386 index paths and 17,407 unverified
objects, with no complete source/backup catalogue. Source RSS high-water is
163,201,024 bytes during setup, not a qualified memory-window result. Source
cgroup CPU is 281.345090 seconds; the live daemon has 2090 ticks at 100 Hz.
Repeated declared I/O-pressure waits remain visible. Different runs and
pressure histories do not establish a causal speed ratio.

Artifact 12132846759 is 2027 bytes, SHA-256
`e1e5c8bceb6c3479c576b059ecc99857e7f1e577e076a48094136c539c9721c2`.
The original fixture/log evidence is retained without an unchanged-source
retry. No idle CPU, full reference RSS, scan or physical-HDD budget is passed.

## Local checks

All 126 script contracts pass, including refusal of ambiguous/incorrect
preset readback and preventing diagnostic results from certifying either
peer. The initial reduced native Fast→Gentle control-flow run completes with
both byte-valid peers, confirmed Gentle readback and an explicit unmeasured
verdict. It uses frozen lab programs from
`9ad54f6a1be11d979ccac876865c18bb7bac6413`; it establishes no resource budget.
The default reduced path is checked separately, and end-state diagnostics are
checked again before submission. Current main is integrated before PR checks.

The final reduced Fast→Gentle run completes with both end-state snapshots,
confirmed Gentle configuration and unmeasured top-level/backup verdicts. The
separate default reduced run also completes without enabling diagnostic mode.
Workspace/all-target Clippy and all 126 script contracts pass. Older released
changelog sections move to a linked archive when the file-length check reaches
1002 lines; their text is retained and relative report links are rebased.

## Integrated release validation

PR #103, head `89c6e8528be96c0dce8fe4cd899e7c0a845292f8`, integrates main `470f744f3ec62a958240ff184af06ae4300a4965` before
validation. CI 39118381050, lab 39118380801 and bridge
39118380890 pass. v0.6.33 is published and independently
verified before this candidate. Local validation passes 126 script contracts,
default and diagnostic lab smokes, zero-warning all-target Clippy and repository
checks. Fresh v0.6.34 tag gates and independent archive/install checks remain
required. Diagnostic preparation cannot qualify M6; elapsed gates stay open.
