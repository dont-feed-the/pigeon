# 0056. Every asserting experiment runs on Linux and macOS; a platform difference is shown, never hidden

- **Status:** Withdrawn, 14 October 2026, by the agent, at Guillaume's request
  (chat): moved to the specification.

This record held the lab and CI policy, and the fixes for what the experiments
found. Under the [policy](README.md#when-to-write-an-adr) that is not an
architecture decision, so it lives in the specification:
[13 §13.7–13.8](../spec/13-verification.md) (every asserting experiment on
Linux and macOS, `SKIP (platform)`, git isolated),
[07 §7.5](../spec/07-capture-and-install.md) (the macOS writer scan),
[08 §8.2](../spec/08-custody-loops.md) and
[16 §16.4](../spec/16-open-questions.md) (the deadline on P2P fetches),
[08 §8.5](../spec/08-custody-loops.md) (the stale `sparse-checkout.lock`) and
[14 §14.3](../spec/14-platforms-and-packaging.md) (`git-remote-p2p-annex` on
macOS). Its text stays in the git history.
