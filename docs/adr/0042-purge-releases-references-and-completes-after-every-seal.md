# 0042. A purge releases the references its deciders saw; content goes only when no other reference holds it and every custodian has sealed

- **Status:** Withdrawn, 14 October 2026, by the agent, at Guillaume's request
  (chat): moved to the specification.

This record held a rule that the specification now carries: [08 §8.6](../spec/08-custody-loops.md) and [13 §13.4](../spec/13-verification.md).
Under the [policy](README.md#when-to-write-an-adr) it is not an ADR: it is
a refinement of ADR 0018, whose alternatives lose content another reference still needs. Its full text, with its context, evidence and
alternatives, is at [commit 1a6067e](https://github.com/mariogeiger/pigeon/blob/1a6067e/docs/adr/0042-purge-releases-references-and-completes-after-every-seal.md).
