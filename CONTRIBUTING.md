# Contributing

If you're interested in joining the community to build a better SQL, here are
ways to start:

- Star this repo.
- Send a link to PRQL to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/PRQL/prql/issues/1) for updates.
- Join our [Discord](https://discord.gg/eQcfaCmsNc)
- Follow us on [Twitter](https://twitter.com/prql_lang)
- Find an issue labeled
  [Good First Issue](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)[^1]
  and start contributing to the code.

[^1]:
    These are better phrased as "Well explained issues"; the core team regularly
    do these issues too!

PRQL is evolving from a project with lots of excitement into a project that
folks are using in their work and integrating into their tools. We're actively
looking for collaborators to lead that growth with us.

## Areas for larger contributions

### Compiler

The compiler is written in Rust, and there's enough to do such that any level of
experience with rust is sufficient.

We try to keep a few onboarding issues on hand under the
["good first issue" label](https://github.com/PRQL/prql/labels/good%20first%20issue).
These have been screened to have sufficient context to get started (and we very
much welcome questions where there's some context missing).

To get started, read [DEVELOPMENT.md](./DEVELOPMENT.md) and
[ARCHITECTURE.md](./prql-compiler/ARCHITECTURE.md)

And if you have questions, there are lots of friendly people on the Discord who
will patiently help you.

### Bindings & integrations

For PRQL to be successful, it needs to be available for the languages & tools
that people already use.

- We currently have bindings to the PRQL compiler in a few different languages;
  many of these can be improved, documented, and packaged in a better way.
- If you have experience with packaging in an ecosystem that doesn't currently
  have bindings, then creating PRQL bindings for that language we don't
  currently support would be valuable to the project.
- If there's a tool that you use yourself to run data queries which you think
  would benefit from a PRQL integration, suggest one to us or the tool. If it's
  open-source, build & share a prototype.

Relevant issues are labeled
[Integrations](https://github.com/PRQL/prql/labels/integrations).

### Language design

We decide on new language features in GitHub issues, usually under
["language design" label](https://github.com/PRQL/prql/issues?q=is%3Aopen+label%3Alanguage-design+sort%3Aupdated-desc).

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
  [a few issues open](https://github.com/PRQL/prql/labels/web) on this front and
  are looking for anyone with at least some design skills.
- Contribute towards the docs. Anything from shaping a whole section of the
  docs, to simply improving a confusing paragraph or fixing a typo.
- Tell people about PRQL.
- Find a group of users who would be interested in PRQL, help them get up to
  speed, help the project understand what they need.

## Core team

If you have any questions or feedback and don't receive a response on one of the
general channels such as GitHub or Discord, feel free to reach out to:

- [**@asljazerzen**](https://github.com/aljazerzen) — Aljaž Mur Eržen
- [**@max-sixty**](https://github.com/max-sixty) — Maximilian Roos
- [**@snth**](https://github.com/snth) — Tobias Brandt

### Core team Emeritus

Thank you to those who have previously served on the core team:

- [**@charlie-sanders**](https://github.com/charlie-sanders) — Charlie Sanders
