# Authority trace discrepancy before M7

The 17 October [nightly models-full run](https://github.com/dont-feed-the/pigeon/actions/runs/39007114840)
failed the good Authority trace at seed 104, event 14. This is outside the
first 50 seeds exercised by the successful v0.6.0 candidate run. It remains
an open M7 conformance item; seven green nights are not claimed.

The generated trace is reproducible with `PIGEON_TRACE_SEEDS=104` and
`PIGEON_TRACE_DIR` when running `pigeon-core`'s `authority_trace` target with
nextest and the `tla-trace` feature. Seed 104 contains an accepted adoption
of a root-file proposal after the original root path was deleted. Its next
ordinary commit by device 4 is accepted by Rust; the two-slot Authority
model cannot consume that event. The trace adapter groups a concrete path
and all its conflict-copy names into one set of versions, while the native
judge evaluates additions and modifications at concrete names.

This appears to expose a concrete-path abstraction gap, but the cause still
needs a dedicated refinement and regression. No production permission rule,
model verdict, seed count or failed observation has been changed to hide it.
The finite Linux release gates and the broader nightly evidence remain
separate as specified by the release policy. M7 cannot close on the current
nightly result.
