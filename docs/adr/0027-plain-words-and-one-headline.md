# 0027. People see plain words and one headline status

- **Status:** Proposed, 13 October 2026, by the agent in the UX study
  ([`docs/design/ux.md`](../design/ux.md)) that Guillaume asked for. To be
  confirmed by the group and by the usability test.

## Context

Most members of a group are not technical. The proposal names three version
states (local, published, protected) and promises an honest status (P6). In
everyday English and French, *published* suggests "made public" and
*protected* suggests a password. The engine underneath (git-annex) has its
own vocabulary (annex, remote, numcopies, trust, drop) that people should
never need.

## Decision

- The code, the spec and `--json` keep the names local, published,
  protected, uncertain. People see: **Only on this computer**, **Shared, not
  safe yet**, **Safe**, **Not checked since …** (French: *Seulement sur cet
  ordinateur*, *Partagé, pas encore en sécurité*, *En sécurité*, *Pas vérifié
  depuis …*). A state is always an icon, a word and a colour together.
- One vocabulary table (ux.md §2.2) is the only source of user-facing words;
  git and git-annex terms appear only in diagnostics and logs.
- Every surface (tray, home page, `pigeon status`, notifications) leads with
  one headline of five levels, worst first: *Needs you*, *Not safe yet*,
  *Working*, *Safe*, *Not checked recently*. A headline says what, then why
  and what happens next, in at most two sentences, with the time of the last
  check.
- Changes younger than a grace period (10 minutes) are *Working*, not *Not
  safe yet*.

## Consequences

- The status computation must produce, besides the per-version state, the
  reason and the next step ("waiting for Paul's NAS, offline since Monday"),
  so the daemon tracks why each version is not yet protected.
- The usability test compares these words with the original ones; the words
  are frozen only after it.
- Thresholds (10 minutes, 24 hours, 7 days) are settings with tested defaults.
