# 0059. A custodian's drop counts only copies hashed for it, and repair counts only members that can take a key

- **Status:** Withdrawn, 14 October 2026, by the agent, at Guillaume's request
  (chat): merged into [0054](0054-a-copy-counts-only-if-it-can-be-locked.md)
  (hashed locks) and [0049](0049-copies-count-once-per-failure-domain.md)
  (domain repair span, repair epochs on intact copies).

This record refined ADRs 0049 and 0054 on the evidence of the `Repair` model.
They now hold it; see the [policy](README.md#when-to-write-an-adr).

ADR 0054 was withdrawn in turn, the same day, and ADR 0049 merged into
ADR 0046: the hashed locks are in [09 §9.4](../spec/09-network-and-joining.md), the
repair span in ADR 0046 and [08 §8.3](../spec/08-custody-loops.md).
