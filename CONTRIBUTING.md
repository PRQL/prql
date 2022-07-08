# Contributing

If you're interested in joining the community to build a better SQL, there are
lots of ways of contributing; big and small:

- Star this repo.
- Send a link to PRQL to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/prql/prql/issues/1) for
  updates.
- Join the [Discord](https://discord.gg/eQcfaCmsNc).
- Contribute towards the code. Most of the code is written in rust, and there's
  enough to do such that any level of experience with rust is sufficient. And if
  you have rust questions, there are lots of friendly people on the Discord who
  will patiently help you.
  - Find an issue labeled [help
    wanted](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)
    or [good first
    issue](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
    and try to fix it. Feel free to PR partial solutions, or ask any questions on
    the Issue or Discord.
  - Start with something tiny! Write a test / write a docstring / make some rust
    nicer — it's a great way to get started by making an lasting impact in 30 minutes.
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

## Development environment

Setting up a local dev environment is simple, thanks to the rust ecosystem:

- Install [`rustup` & `cargo`](https://doc.rust-lang.org/cargo/getting-started/installation.html).
- That's it! Running `cargo test` should complete successfully.
- For more advanced development; e.g. adjusting `insta` outputs or compiling for
  web, run the commands in [Taskfile.yml](Taskfile.yml), either by copying &
  pasting or by installing [Task](https://taskfile.dev/#/installation) and
  running `task setup-dev`.
- For quick contributions, hit `.` in GitHub to launch a [github.dev
  instance](https://github.dev/prql/prql).
- Any problems: post an issue and we'll help.

## Merging

- **We merge any code that makes PRQL better**
- A pull request doesn't need to be perfect to be merged; it doesn't need to
  solve a big problem. It needs to:
  - be in the right direction
  - make incremental progress
  - be explicit on its current state, so it's easy for others to make more
    progress
- If you have merge permissions, and are reasonably confident that the expected
  value of the code in a pull request is positive, feel free to merge.
  - If you don't have merge permissions and have made a few pull requests, ask
    and ye shall receive.
- The primary way we ratchet the quality our code is through automated tests.
  - This means we generally need some sort of test for code changes to
    demonstrate incremental progress.
  - If a code change breaks functionality without breaking tests, our tests were
    insufficient.
- We use PR reviews to offer assistance and discuss broad issues with the code.
  - If you'd like a pull request to be reviewed before it merges, that's great —
    ask / assign a reviewer.
  - If a pull request hasn't received attention, please feel free to ping the pull
    request after a day.
  - Reviews around 'nits' like code formatting / idioms / etc are very welcome.
    But these are best handled through automated tests & lints rather than
    manual intervention. There isn't an expectation that authors must respond to
    refine-y feedback with specific changes.
- People may review a pull request after it's been merged. As part of the
  understanding that we can merge quickly, there's an expectation that
  contributors respond to feedback after merges.
- We should revert quite quickly if a pull request isn't consistent with our
  expectations, or there isn't as much consensus as we had hoped. It's very easy
  to revert code and then re-revert when we've resolved the issue. It's not a
  sign of bad engineering to have code reverted!
