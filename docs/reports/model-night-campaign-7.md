# Scheduled model qualification on stable main

Seven actual Linux nights remain required for M7. Routine `models-full`
skips changed-main work once the source is 25 hours old, which prevents
qualification while a release candidate stays stable.

Repository Actions variable `PIGEON_MODEL_NIGHTS_UNTIL` can enable full
scheduled model work on unchanged main until an explicit `YYYY-MM-DD`
expiry, exclusive at UTC midnight. An empty or expired value restores the
usual 25-hour policy. A malformed date fails selection visibly. Tag and
manual tiers and Monday's independent audits keep their existing behavior.
No source push cancels a scheduled main run.

This selects work; it does not create a scheduled event, count a manual run
as a night, replace a failed night, or establish a release's conformance.
The read-only nightly report still requires complete actual Linux jobs and
seven distinct consecutive UTC dates. Mac results remain background
observations. The campaign has not been enabled at this local checkpoint.

Four policy contracts cover expiry boundaries, unchanged/recent sources,
invalid dates and existing event/audit tiers. The existing shell-workflow
contract uses a coherent UTC clock and continues to cover all six tier
combinations.
