# 0082. Use the maintained D-Bus client for Linux notifications

- **Status:** Proposed, 17 October 2026.
- Supersedes the Linux notification-backend choice in ADR 0029; its calm
  notification policy and the macOS backend remain applicable.

## Context

The dependency audit began rejecting `zvariant`, `zbus_names` and
`zvariant_derive` under RUSTSEC-2026-0370, RUSTSEC-2026-0371 and
RUSTSEC-2026-0372. Upstream consolidated these crates into zbus 6.
`notify-rust` 4.18.2 still requires zbus 5 for its Linux backend, so upgrading
only pigeon's power/network probes would retain the retired crates.

## Decision

Use zbus 6 for Linux power/network probes and the standard
`org.freedesktop.Notifications` interface. Keep `notify-rust` for macOS,
without enabling its Linux D-Bus dependency. Add no advisory exceptions.

Subscribe to notification signals before sending Notify. Only the returned
notification ID's default action opens pigeon's own local page. Closure or
another action does not open it. The signal queue is bounded. Reap the
`xdg-open` child after a click. Missing desktop services remain optional;
no content or capture operation waits for a notification.

## Consequences

The notification body and localized action stay the same. Private native
D-Bus fixtures verify the call, immediate actions, unrelated notification
IDs and closure without opening a browser or using the user's session bus.
Dependency auditing covers the replacement chain on all supported targets.
