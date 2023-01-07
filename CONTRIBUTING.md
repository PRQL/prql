# Contributing

If you're interested in joining the community to build a better SQL, here is how
you start:

- Star this repo.
- Send a link to PRQL to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/PRQL/prql/issues/1) for updates.
- Join the [Discord](https://discord.gg/eQcfaCmsNc).

PRQL is evolving into a medium-sized project and we are looking for help in a
few different areas.

### Compiler

Compiler is written in Rust, and there's enough to do such that any level of
experience with rust is sufficient.

We try to keep a few onboarding issues on hand under the
["good first issue" label](https://github.com/PRQL/prql/labels/good%20first%20issue).
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
with appropriate framework and with higher quality code.

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
["integrations" label](https://github.com/PRQL/prql/labels/integrations).

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

- [**@aljazerzen**](https://github.com/aljazerzen) — Aljaž Mur Eržen
- [**@max-sixty**](https://github.com/max-sixty) — Maximilian Roos
- [**@snth**](https://github.com/snth) — Tobias Brandt

### Core team Emeritus

Thank you to those who have previously served on the core team:

- [**@charlie-sanders**](https://github.com/charlie-sanders) — Charlie Sanders
