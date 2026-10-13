# 0029. A status icon in the menu bar or tray, and calm notifications

- **Status:** Proposed, 13 October 2026, by the agent in the UX study.

## Context

ADR 0021 gives two interfaces, the CLI and a local web page. People who are
not technical open neither unless something tells them to. They need a
glance-level answer to "are my files safe?", and to be told when they must
act, without being disturbed when all is well.

## Decision

- A small `pigeon-tray` process shows the headline level (ADR 0027) as the
  colour of an icon in the macOS menu bar or the Linux tray, with a menu:
  the headline, *Open pigeon*, *Open my folder*, *Pause sharing*. It is a
  client of `pigeond` over the local socket, like the CLI (ADR 0011), and
  holds no logic. Library: `tray-icon` 0.26.
- Notifications through `notify-rust` 4.18 only when a person must act, when
  files stay not safe longer than a threshold (24 h by default), or when a
  suggestion waits for them. Never for success; grouped, never per file.
- Headless machines (backup machines, servers) run no tray.

## Consequences

- One more binary to package and start at login (a LaunchAgent on macOS, an
  XDG autostart entry on Linux).
- GNOME without the AppIndicator extension shows no tray icon; there the
  notifications and the web page remain, and `pigeon doctor` says so.
