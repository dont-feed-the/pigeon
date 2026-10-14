# 0045. git and git-annex read only the configuration pigeon writes

- **Status:** Withdrawn, 14 October 2026, by the agent, at Guillaume's request
  (chat): moved to the specification.

This record held a rule that the specification now carries: [03 §3.7](../spec/03-repository-and-roles.md) and [07 §7.9](../spec/07-capture-and-install.md).
Under the [policy](README.md#when-to-write-an-adr) it is not an ADR: it is
a safety invariant (git reads nothing pigeon did not write), a refinement of ADR 0010. Its full text, with its context, evidence and
alternatives, is at [commit 1a6067e](https://github.com/mariogeiger/pigeon/blob/1a6067e/docs/adr/0045-git-reads-only-the-configuration-pigeon-writes.md).
