# Report: optional Tokio task inspection

M4 progress, version 0.4.12, 16 October 2026. ADR 0025 calls for a debug
console feature. `pigeond/tokio-console` now forwards to the shared
telemetry crate's optional pinned console-subscriber 0.5.0 dependency and
Tokio tracing. Release packaging enables neither.

A console layer requires all three conditions: debug assertions,
`tokio_unstable`, and an explicit `PIGEON_TOKIO_CONSOLE=1` switch. Its
address is fixed to 127.0.0.1:6669; it does not read the console crate's
default environment configuration. The event buffer is 1024 and task
retention is 60 seconds. Per-client buffering is bounded by the upstream
default. The console's server thread is the crate's dedicated runtime.

Daily log filtering is now per layer, so its ordinary info threshold
cannot veto runtime task spans for the console. Runtime/tokio spans are
excluded from the OpenTelemetry bridge. Local logs and group telemetry
keep their existing independent destinations and bounds.

## Verification

The complete warning-free workspace Clippy check passes with the optional
feature, including a build without Tokio's unstable flag. A real debug
daemon built with the flag runs in two isolated no-group fixtures. Without
the switch it opens no console listener. With the switch, Linux's actual
TCP table reports only 127.0.0.1:6669 despite a hostile console bind setting.
A real HTTP/2 gRPC WatchUpdates request receives five runtime tasks, proving
that the normal log level does not hide the task events. Both daemons stop
normally and no console listener remains. The complete probe lifecycle
runs under a subreaper.

A second real build without `tokio_unstable` starts normally with the
switch both absent and present, and opens no console listener in either
case. The required default-feature full-workspace Clippy and repository
checks also pass. The CI Clippy step includes the optional feature so its
API is compiled on the supported runners. Sync/protection lag and connection/queue dashboard
instrumentation remain their own M4 follow-up. This report is not a release
or long-duration gate claim.
