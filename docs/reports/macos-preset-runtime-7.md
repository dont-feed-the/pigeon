# Native macOS preset observation

> Mission: check actual launchd mode changes and native CPU child inheritance,
> separately from rendering, Linux measurements and the foreground HDD budget.

The existing packaging checks parse the Mac agents and exercise helper behavior
against a launchctl stand-in. They cannot establish that a real path watch
replaces the native daemon or that the new process receives a different native
CPU priority. The existing Mac maintenance observation establishes explicit
idle-child scheduling and actual corruption scrubbing for v0.6.16; it predates
the native resource presets.

A disposable hosted Mac packaging check now installs the actual service helper
around an observer process, then requests background, normal and background
mode. It exercises atomic hint replacement through the real launchd WatchPaths,
requires a replacement PID for each native-mode change, and requires equal
requests to preserve the PID. An observer built with the installed SDK reads
THREAD_EXTENDED_INFO through the public Mach API. It samples the daemon thread
via a ctypes library and an independently executed native child. Both must
have lower base priority in background mode and regain that priority afterward.
The test does not assume a fixed numeric kernel priority or claim I/O throughput.

The XNU public `mach/thread_info.h` defines `pth_priority` in
`thread_extended_info_data_t`; `task_policy.c` makes background daemon application
types DARWIN_BG. Private task-policy-state inspection requires root and is not
used. The test changes priority only through the provided supervisor; its native
observer only reads its own thread and process scheduling information.

The fixture refuses pre-existing pigeon supervisor files and loaded jobs and runs only when
GitHub Actions and Darwin are both detected. Ordinary local tests skip it,
so they cannot replace a person's installed service. The automatic updater is
disabled in the fixture. Cleanup removes only its supervision; failed synthetic
observers and values are retained under runner.temp/pigeon-macos-priority and
uploaded for 14 days by the failure-only packaging step. No identity, group or
invitation is created or uploaded. There is no new user-facing behavior.

## Hosted native observation

The one-runner [hosted Mac run](https://github.com/dont-feed-the/pigeon/actions/runs/39086664756)
passes on its first attempt at source
`5e9c2c7f082006e2b0a0ed4963a0b3dc97ac6243`. Job 117556064797 runs the
native observation in 28.783 seconds. Background, normal and background
processes have PIDs 3423, 3857 and 4115; their main-thread base priorities
are 4, 20 and 4. Independently executed children match each priority. Nice
remains zero in both modes; nice alone would miss this launchd policy.
Equal atomic requests preserve each PID. No fixed priority value is asserted.

Linux packaging discovery runs 33 checks: 32 pass and this native Mac check
explicitly skips. This environment has no Apple SDK or launchd, so the
hosted observation supplies the native result. Mac results remain
background evidence under the release policy; all required Linux publication
gates and both release archives stay required.

The manual macos-background workflow also accepts native_priority_only=true.
That profile uses one Mac job for this observer and skips annex, Rust, lab
and slice stages. It remains serialized with other Mac background runs and
leaves room for both release builds, so an active release does not defer the
one-runner profile. The ordinary periodic profile retains its release guard.
This is targeted runtime evidence, not a complete Mac suite.
