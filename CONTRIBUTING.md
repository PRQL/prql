# Contributing

If you're interested in joining the community to build a better SQL, there are
lots of ways of contributing - big and small:

- Star this repo.
- Send a link to PRQL to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/PRQL/prql/issues/1) for
  updates.
- Join the [Discord](https://discord.gg/eQcfaCmsNc).
- Contribute towards the code. Most of the code is written in rust, and there's
  enough to do such that any level of experience with rust is sufficient.
  Read the [DEVELOPMENT.md](./DEVELOPMENT.md) file to get started.
  - Find an issue labeled [help
    wanted](https://github.com/PRQL/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)
    or [good first
    issue](https://github.com/PRQL/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
    and try to fix it. Feel free to PR partial solutions, or ask any questions on
    the Issue or Discord.
  - Start with something tiny! Write a test / write a docstring / make some rust
    nicer — it's a great way to get started by making an lasting impact in 30 minutes.
  - And if you have rust questions, there are lots of friendly people on the
    Discord who will patiently help you.
- Contribute towards the docs. Anything from shaping a whole section of the
  docs, to simply improving a confusing paragraph or fixing a typo.
- Contribute towards the language.
  - Find instances where the compiler produces incorrect results, and post a bug
    report — feel free to use the [playground](https://prql-lang.org/playground).
  - Open an issue / append to an existing issue with examples of queries that
    are difficult to express in PRQL — especially if more difficult than SQL.
  - With sufficient examples, suggest a change to the language! (Though
    suggestions _without_ examples are difficult to engage with, so please do
    anchor suggestions in examples.)
- Contribute towards the project.
  - Improve our website / book.
  - Tell people about PRQL.
  - Find a group of users who would be interested in PRQL, help them get up to
    speed, help the project understand what they need.

Any of these will inspire others to invest their time and energy into the
project; thank you in advance.

## Commits

- If a change is user-facing, it would be helpful to add a line in
  [**`CHANGELOG.md`**](CHANGELOG.md), with `{message}, ({@contributor, #X})`
  where `X` is the PR number.
- We're experimenting with using the [Conventional
  Commits](https://www.conventionalcommits.org) message format, enforced through
  [action-semantic-pull-request](https://github.com/amannn/action-semantic-pull-request).
  This would let us generate Changelogs automatically. The check is not required
  to pass at the moment.

## Merges

- **We merge any code that makes PRQL better**
- A PR doesn't need to be perfect to be merged; it doesn't need to solve a big
  problem. It needs to:
  - be in the right direction
  - make incremental progress
  - be explicit on its current state, so others can continue the progress
- If you have merge permissions, and are reasonably confident that a PR is
  suitable to merge (whether or not you're the author), feel free to merge.
  - If you don't have merge permissions and have authored a few PRs, ask and ye
    shall receive.
- The primary way we ratchet the code quality is through automated tests.
  - This means PRs almost always need a test to demonstrate incremental
    progress.
  - If a change breaks functionality without breaking tests, our tests were
    insufficient.
- We use PR reviews to give general context, offer specific assistance, and
  collaborate on larger decisions.
  - Reviews around 'nits' like code formatting / idioms / etc are very welcome.
    But the norm is for them to be received as helpful advice, rather than as
    mandatory tasks to complete. Adding automated tests & lints to automate
    these suggestions is welcome.
  - If you have merge permissions and would like a PR to be reviewed before it
    merges, that's great — ask or assign a reviewer.
  - If a PR hasn't received attention after a day, please feel free to ping the
    pull request.
- People may review a PR after it's merged. As part of the understanding that we
  can merge quickly, contributors are expected to incorporate substantive
  feedback into a future PR.
- We should revert quickly if the impact of a PR turns out not to be consistent
  with our expectations, or there isn't as much consensus on a decision as we
  had hoped. It's very easy to revert code and then re-revert when we've
  resolved the issue; it's a sign of moving quickly.

## Core team

If you have any questions or feedback and don't receive a response on one of the
general channels such as GitHub or Discord, feel free to reach out to:

- [**@aljazerzen**](https://github.com/aljazerzen) — Aljaž Mur Eržen
- [**@max-sixty**](https://github.com/max-sixty) — Maximilian Roos
- [**@snth**](https://github.com/snth) — Tobias Brandt

### Core team Emeritus

Thank you to those who have previously served on the core team:

- [**@charlie-sanders**](https://github.com/charlie-sanders) — Charlie Sanders
