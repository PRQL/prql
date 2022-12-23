# Contributing

If you're interested in joining the community to build a better SQL, here is how
you start:

- Star this repo.
- Send a link to PRQL to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/prql/prql/issues/1) for updates.
- Join the [Discord](https://discord.gg/eQcfaCmsNc).

PRQL is evolving into a medium-sized project and we are looking for help in a
few different areas.

### Compiler

Compiler is written in Rust, and there's enough to do such that any level of
experience with rust is sufficient.

We try to keep a few onboarding issues on hand under the
["good first issue" label](https://github.com/prql/prql/labels/good%20first%20issue).
They have better descriptions of what to do than other issues, so they are a
good place to start.

To get started, you should read [DEVELOPMENT.md](./DEVELOPMENT.md) and
[ARCHITECTURE.md](./prql-compiler/ARCHITECTURE.md)

And if you have questions, there are lots of friendly people on the Discord who
will patiently help you.

### IDE

For non-technical savvy people, best way to explain a language and make it
useful is to build a UI. We currently do have the playground, but we are
dreaming bigger:

- Most approachable and portable way, would be a web application, just like
  playground.
- Unlike playground, it needs support for importing arbitrary CSV and parquet
  input files and then exporting the results.
- Fastest (and cheapest) way to execute the queries is probably DuckDB WASM
  (like in playground),
- Dreaming bigger, we would want support for a LSP client connected to a LSP
  server (upcoming) running as web worker.

This could all be achieved by extending the playground, or by starting anew,
with appropriate framework and without a bunch of mis-patterns that may or may
not be hidden in the playground code.

The project is in the brainstorming phase, so I you are interested, post a
message in the #web Discord channel.

### Integrations

PRQL will become usable for everyday tasks when it becomes easy to use from
other languages and tools.

We currently have bindings to the PRQL compiler in a few different languages,
but they may be lacking in ergonomics, documentation or even functionality.

If you have experience with packaging or are maintaining a tool for data
analysis, we'd need your help!

Try looking over
["integrations" label](https://github.com/prql/prql/labels/integrations).

### Language design

We decide on new language features in GitHub issues, usually under
["language design" label](https://github.com/prql/prql/issues?q=is%3Aopen+label%3Alanguage-design+sort%3Aupdated-desc).

You can also contribute by:

- Finding instances where the compiler produces incorrect results, and post a
  bug report — feel free to use the
  [playground](https://prql-lang.org/playground).
- Opening an issue / append to an existing issue with examples of queries that
  are difficult to express in PRQL — especially if more difficult than SQL.

With sufficient examples, suggest a change to the language! (Though suggestions
_without_ examples are difficult to engage with, so please do anchor suggestions
in examples.)

### Marketing

- Improve our website. We have
  [a few issues open](https://github.com/prql/prql/labels/web) on this front and
  are looking for anyone with at least some design skills.
- Contribute towards the docs. Anything from shaping a whole section of the
  docs, to simply improving a confusing paragraph or fixing a typo.
- Tell people about PRQL.
- Find a group of users who would be interested in PRQL, help them get up to
  speed, help the project understand what they need.

## Commits

- If a change is user-facing, it would be helpful to add a line in
  [**`CHANGELOG.md`**](CHANGELOG.md), with `{message}, ({@contributor, #X})`
  where `X` is the PR number.
- We're experimenting with using the
  [Conventional Commits](https://www.conventionalcommits.org) message format,
  enforced through
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
