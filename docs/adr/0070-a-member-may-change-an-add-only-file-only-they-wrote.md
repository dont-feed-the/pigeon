# 0070. A member may change an add-only file that only they wrote

- **Status:** Accepted, 15 October 2026, by Mario (chat, answering the
  question of [16 §16.4](../spec/16-open-questions.md)).

## Context

In an add-only folder anyone adds a file, and only its stewards (the admins
when none is named) modify or delete one ([04 §4.3](../spec/04-roster-and-authority.md)).
A member who saves a file twice, such as a photo they cropped after adding
it, makes a suggestion of their own file that a steward must accept. The
question was whether the author of a path's only version may change it
until someone else does.

The alternative is to keep the rule as it is: simple, but it turns
everyday corrections into work for the stewards.

## Decision

- In an add-only folder, and at the root, a member may modify or delete a
  path whose every version in the base was written by them: the persons who
  wrote a slot are the authors of the contributions that wrote or deleted
  it in B(c), an adoption counting as its proposal's author and a conflict
  copy as its version's (05 §5.2). Once another person writes the path,
  only the stewards change it.
- An owned folder is unchanged: only its owner changes it.
- The deciders of a proposal follow (04 §4.5): the person who alone wrote a
  path decides on a suggestion to change it, with its stewards.
- `pigeon-core::catalogue` gives the writers of each slot
  (`View::writers`), and `authority::allowed` takes them.
  `Authority.tla` records the person each commit writes as and checks the
  rule (`Writers`, `Mine`), and the code's verdicts are checked against it
  (`AuthorityTrace`).

## Consequences

- Fewer suggestions in shared folders; a member corrects their own file at
  once.
- The verdict on a commit now depends on who wrote its paths, which the
  catalogue knows from accepted history alone, so it stays a pure function
  of the commit and its ancestors (locality, 04 §4.4).
- Writers are persons, not devices: a person's new laptop changes the files
  their lost one wrote.
