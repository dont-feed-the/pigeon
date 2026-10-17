# Native macOS maintenance priority

The maintenance launcher supported Linux's idle I/O class only. On macOS it
returned Unsupported from the pre-exec hook, so scheduled integrity checks
could not start. The native corrupt-object scrub fixture was Linux-only.

A maintenance child now sets nice 19 and Darwin's process-scoped throttled
disk policy before exec. The daemon's scheduling is unchanged. Any failed
native scheduling call still prevents the child starting. Other unsupported
platforms still fail explicitly; this change adds no macOS backup role.
The existing child/parent fixture checks both priorities after exec on each
supported platform; the corrupt-object scrub fixture also runs on macOS.

Apple's `bsd/sys/resource.h` defines IOPOL_TYPE_DISK=0,
IOPOL_SCOPE_PROCESS=0 and IOPOL_THROTTLE=3. Its
[`getiopolicy_np(3)`](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man3/getiopolicy_np.3)
describes throttling for long-running background I/O and inheritance.
`libsyscall/wrappers/getiopolicy_np.c` forwards scalar process parameters to
`__iopolicysys`; `bsd/kern/kern_resource.c::iopolicysys_disk`'s process branch
does not enter the thread-QoS removal path. The pre-exec hook performs no
allocation, locking or runtime work.

Apple Silicon all-target Clippy for pigeon-sys passes. This is compile-time
evidence, not a macOS runtime result, a completed resource milestone or a
measurement of foreground interference. Linux native and broader checks,
current-main integration and background macOS results are recorded below.

## Initial validation

All 30 Linux platform-layer checks pass, including inherited child CPU/I/O
priority and unchanged parent policy. Whole-workspace all-target Clippy,
formatting and header/length/ADR/spec checks pass on this v0.6.15 base.
The Apple Silicon platform all-target Clippy check passes separately.
Logs: `/tmp/pigeon-macos-maintenance-local.log`. The actual corrupt-object
scrub and all affected checks are repeated with the transfer-progress rollup
on current main before its PR; no macOS runtime pass is inferred here.

The outer install-test module guard is also enabled for macOS, so the new
scrub fixture is actually discovered there. A broader daemon install-test
cross-check could not compile aws-lc because this Linux environment lacks
an Apple C compiler/SDK (`cc` rejects `-mmacosx-version-min`). Its retained
log is `/tmp/pigeon-macos-maintenance-install-cross.log`; it supplies no
daemon cross-compile or runtime verdict. The separate platform-layer Apple
all-target Clippy result above remains valid. Native Apple validation runs
in the background under the established release policy.

## Current-main rollup validation

The integrated rollup passes all 30 Linux platform checks, the real
corrupt-object scrub, all affected transfer/daemon selections and whole-
workspace all-target Clippy. The scrub preserves healthy objects and its
durable result, and refuses unknown/torn maintenance state.

A separate earlier background Mac workspace job
[117528514931](https://github.com/dont-feed-the/pigeon/actions/runs/39077215598/job/117528514931)
at source `323a2c8a82401f05291ad48ad282418b48981ee1` passed 735/735 checks
(184.846 seconds, one slow, one skipped), including native streaming and
Apple power/network probes. That source predates this maintenance fix;
it is not runtime proof of the new disk policy or a passing complete
background workflow. The remaining background stages are separate.
