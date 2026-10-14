# 0049. Copies count once per failure domain: shared disks, machines and declared domains

- **Status:** Withdrawn, 14 October 2026, by the agent, at Guillaume's request
  (chat): merged into [0046](0046-protected-means-content-and-history-on-disk.md);
  its rule was then dropped, at his request too.

This record was one part of the decision that ADR 0046 held. Its rule, that
copies count once per failure domain, was dropped the same day (Guillaume,
chat: "we won't be able to keep track of failure domains in practice so we
shouldn't try to model them"): each custodian now counts once, by its device.
Its full text, with its context, evidence and alternatives, is at
[commit 1a6067e](https://github.com/mariogeiger/pigeon/blob/1a6067e/docs/adr/0049-copies-count-once-per-failure-domain.md).
