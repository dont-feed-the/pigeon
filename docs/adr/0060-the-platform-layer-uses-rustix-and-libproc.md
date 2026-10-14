# 0060. The platform layer uses rustix, with libproc on macOS; the lab seams are a self-kill and a file

- **Status:** Withdrawn, 14 October 2026, by the agent, at Guillaume's request
  (chat): moved to the specification.

This record held library picks and platform details. Under the
[policy](README.md#when-to-write-an-adr) that is not an architecture decision,
so it lives in the specification:
[14 §14.7–14.8](../spec/14-platforms-and-packaging.md) (`rustix`, `libc` for
`libproc`, `cargo deny`), [07 §7.5](../spec/07-capture-and-install.md) (the
writer scan), [06 §6.3](../spec/06-status.md) (fingerprints),
[11 §11.2](../spec/11-local-state.md) (directories) and
[13 §13.12](../spec/13-verification.md) (the lab seams). Its text stays in the
git history.
