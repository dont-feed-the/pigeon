# 0029. Calm notifications and a launcher in v1; the status icon waits for v1.1

- **Status:** Accepted, 13 October 2026, by Guillaume (UX question 3: the
  status icon later, in v1.1). Proposed by the agent in the UX study with
  the icon in v1.

## Context

ADR 0021 gives two interfaces, the CLI and a local web page. People who are
not technical open neither unless something tells them to. They need to be
told when they must act, without being disturbed when all is well, and a
glance-level answer to "are my files safe?". A menu-bar or tray icon gives
the glance; it is one more process to package, and GNOME hides tray icons
without an extension. Guillaume judged notifications and the web page
enough at first.

## Decision

- v1: notifications through `notify-rust` 4.18 only when a person must act,
  when files stay not safe longer than a threshold (24 h by default), or when
  a suggestion or a conflict waits for them. Never for success; grouped,
  never per file. Clicking one opens the web page.
- v1: a launcher entry named "pigeon" (an XDG `.desktop` file on Linux, a
  small app bundle on macOS) runs `pigeon open`, so the web page is one
  click away in the dock or the application menu.
- v1.1: a `pigeon-tray` process showing the headline level (ADR 0027) as
  the colour of an icon, with a menu: the headline, *Open pigeon*, *Open my
  folder*, *Pause sharing*. A client of `pigeond` over the local socket, like
  the CLI (ADR 0011). Library then: `tray-icon`.
- Headless machines (backup machines, servers) have neither.

## Consequences

- Between notifications, people see the status only by opening pigeon; the
  usability test checks whether that is enough.
- The headline and its levels are designed now, so the icon in v1.1 only
  renders them.
