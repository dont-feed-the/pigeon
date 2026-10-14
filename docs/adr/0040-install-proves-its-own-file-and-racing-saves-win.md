# 0040. Install proves a file is its own before replacing it, and a racing save always wins

- **Status:** Withdrawn, 14 October 2026, by the agent, at Guillaume's request
  (chat): moved to the specification.

This record held a rule that the specification now carries: [07 §7.5](../spec/07-capture-and-install.md) and [11 §11.3](../spec/11-local-state.md).
Under the [policy](README.md#when-to-write-an-adr) it is not an ADR: it is
a safety invariant (never overwrite a save, P2) whose alternative loses a person's write, and a refinement of ADR 0034. Its full text, with its context, evidence and
alternatives, is at [commit 1a6067e](https://github.com/mariogeiger/pigeon/blob/1a6067e/docs/adr/0040-install-proves-its-own-file-and-racing-saves-win.md).
