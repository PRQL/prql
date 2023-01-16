# Development

## Setting up an initial dev environment

We can set up a local development environment sufficient for navigating,
editing, and testing PRQL's compiler code in two minutes:

- Install
  [`rustup` & `cargo`](https://doc.rust-lang.org/cargo/getting-started/installation.html).
- [Optional but highly recommended] Install `cargo-insta`, our testing
  framework:

  ```sh
  cargo install cargo-insta
  ```

- That's it! Running the unit tests for the `prql-compiler` crate after cloning
  the repo should complete successfully:

  ```sh
  cargo test -p prql-compiler --lib
  ```

  ...or, to run tests and update the test snapshots:

  ```sh
  cargo insta test --accept -p prql-compiler --lib
  ```

  There's more context on our tests in [How we test](#how-we-test) below.

That's sufficient for making an initial contribution to the compiler.

---

## Setting up a full dev environment

> **Note**: We really care about this process being easy, both because the
> project benefits from more contributors like you, and to reciprocate your
> future contribution. If something isn't easy, please let us know in a GitHub
> Issue. We'll enthusiastically help you, and use your feedback to improve the
> scripts & instructions.

For more advanced development; for example compiling for wasm or previewing the
website, we have two options:

### Option 1: Use the project's `task`

> **Note**: This is tested on MacOS, should work on Linux, but won't work on
> Windows.

- Install Task; either `brew install go-task/tap/go-task` or as described on
  [Task](https://taskfile.dev/#/installation)
- Then run the `setup-dev` task. This runs commands from our
  [Taskfile.yml](Taskfile.yml), installing dependencies with `cargo`, `brew`,
  `npm` & `pip`, and suggests some VSCode extensions.

  ```sh
  task setup-dev
  ```

### Option 2: Install tools individually

- We'll need `cargo-insta`, to update snapshot tests:

  ```sh
  cargo install cargo-insta
  ```

- We'll need a couple of additional components, which most systems will have
  already. The easiest way to check whether they're installed is to try running
  the full tests:

  ```sh
  cargo test
  ```

  ...and if that doesn't complete successfully, check we have:

  - A clang compiler, to compile the DuckDB integration tests, since we use
    [`duckdb-rs'](https://github.com/wangfenjin/duckdb-rs). To install one:

    - On MacOS, install xcode with `xcode-select --install`
    - On Debian Linux, `apt-get update && apt-get install clang`
    - On Windows, `duckdb-rs` isn't supported, so these tests are excluded

  - Python >= 3.7, to compile `prql-python`.

- For more involved contributions, such as building the website, playground,
  book, or some release artifacts, we'll need some additional tools. But we
  won't need those immediately, and the error messages on what's missing should
  be clear when we attempt those things. When we hit them, the
  [Taskfile.yml](Taskfile.yml) will be a good source to copy & paste
  instructions from.

<!--

Until we set up a Codespaces, I don't think this is that helpful — it can't run any code,
including navigating rust code with rust-analyzer. We'd def take a contribution for a
codespaces template, though.

### github.dev

- Alternatively, for quick contributions (e.g. docs), hit `.` in GitHub to
  launch a [github.dev instance](https://github.dev/PRQL/prql). This has the
  disadvantage that code can't run. -->

### Building & testing the full project

We have a couple of tasks which incorporate all building & testing. While they
don't need to be run as part of a standard dev loop — generally we'll want to
run a more specific test — they can be useful as a backstop to ensure everything
works, and as a reference for how each part of the repo is built & tested. They
should be consistent with the GitHub Actions workflows; please report any
inconsistencies.

To build everything:

```sh
task build-all
```

To run all tests; (which includes building everything):

```sh
task test-all
```

These require installing Task, either `brew install go-task/tap/go-task` or as
described on [Task](https://taskfile.dev/#/installation).

## Contribution workflow

We're similar to most projects on GitHub — open a Pull Request with a suggested
change!

### Commits

- If a change is user-facing, it would be helpful to add a line in
  [**`CHANGELOG.md`**](CHANGELOG.md), with `{message}, ({@contributor, #X})`
  where `X` is the PR number.
- We're experimenting with using the
  [Conventional Commits](https://www.conventionalcommits.org) message format,
  enforced through
  [action-semantic-pull-request](https://github.com/amannn/action-semantic-pull-request).
  This would let us generate Changelogs automatically. The check is not required
  to pass at the moment.

### Merges

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

## Components of PRQL

The PRQL project has several components. Instructions for working with them are
in the **README.md** file in their respective paths. Here's an overview:

**[book](./book/README.md)**: The PRQL language book, which documents the
language.

**[playground](./playground/README.md)**: A web GUI for the PRQL compiler. It
shows the PRQL source beside the resulting SQL output.

**[prql-compiler](./prql-compiler/README.md)**: Installation and usage
instructions for building and running the `prql-compiler`.

**[prql-java](./prql-java/README.md)**: Rust bindings to the `prql-compiler`
rust library.

**[prql-js](./prql-js/README.md)**: Javascript bindings to the `prql-compiler`
rust library.

**[prql-lib](./prql-lib/README.md)**: Generates `.a` and `.so` libraries from
the `prql-compiler` rust library for bindings to other languages

**[prql-python](./prql-python/README.md)**: Python bindings to the
`prql-compiler` rust library.

**[website](./website/README.md)**: Our website, hosted at
<https://prql-lang.org>, built with `hugo`.

## How we test

We use a pyramid of tests — we have fast, focused tests at the bottom of the
pyramid, which give us low latency feedback when developing, and then slower,
broader tests which ensure that we don't miss anything as PRQL develops[^1].

<!-- markdownlint-disable MD053 -->

[^1]:
    Our approach is very consistent with
    **[@matklad](https://github.com/matklad)**'s advice, in his excellent blog
    post [How to Test](https://matklad.github.io//2021/05/31/how-to-test.html).

> **Note**
>
> If you're making your first contribution, you don't need to engage with all
> this — it's fine to just make a change and push the results; the tests that
> run in GitHub will point you towards any errors, which can be then be run
> locally if needed. We're always around to help out.

Our tests, from the bottom of the pyramid to the top:

- **[Static checks](.pre-commit-config.yaml)** — we run a few static checks to
  ensure the code stays healthy and consistent. They're defined in
  [**`.pre-commit-config.yaml`**](.pre-commit-config.yaml), using
  [pre-commit](https://pre-commit.com). They can be run locally with

  ```sh
  pre-commit run -a
  ```

  The tests fix most of the issues they find themselves. Most of them also run
  on GitHub on every commit; any changes they make are added onto the branch
  automatically in an additional commit.

- **Unit tests & inline insta snapshots** — like most projects, we rely on unit
  tests to rapidly check that our code basically works. We extensively use
  [Insta](https://insta.rs/), a snapshot testing tool which writes out the
  values generated by our code, making it fast & simple to write and modify
  tests[^3].

  These are the fastest tests which run our code; they're designed to run on
  every save while you're developing. We include a `task` which does this (thy
  full command run on every save is
  `cargo insta test --accept -p prql-compiler --lib`):

  ```sh
  task -w test-rust-fast
  ```

<!--
This is the previous doc. It has the advantage that it explains what it's doing, and is
easy to change (e.g. to run all packages). But because of
https://github.com/watchexec/watchexec/issues/371, the ignore behavior is unfortunately quite
inconsistent in watchexec. Let's revert back if it gets solved.

[^2]: For example, this is a command I frequently run:

    ```sh
    RUST_BACKTRACE=1 watchexec -e rs,toml,pest,md -cr --ignore='target/**' -- cargo insta test --accept -p prql-compiler --lib
    ```

    Breaking this down:

    - `RUST_BACKTRACE=1` will print a full backtrace, including where an error
      value was created, for rust tests which return `Result`s.
    - `watchexec -e rs,toml,pest,md -cr --ignore='target/**' --` will run the
      subsequent command on any change to files with extensions which we are
      generally editing.
    - `cargo insta test --accept --` runs tests with `insta`, a snapshot
      library, and writes any results immediately. I rely on git to track
      changes, so I run with `--accept`, but YMMV.
    - `-p prql-compiler --lib` is passed to cargo by `insta`; `-p prql-compiler`
      tells it to only run the tests for `prql-compiler` rather than the other
      crates, and `--lib` to only run the unit tests rather than the integration
      tests, which are slower.
    - Note that we don't want to re-run on _any_ file changing, because we can
      get into a loop of writing snapshot files, triggering a change, writing a
      snapshot file, etc. -->

[^3]:
    [Here's an example of an insta test](https://github.com/PRQL/prql/blob/0.2.2/prql-compiler/src/parser.rs#L580-L605)
    — note that only the initial line of each test is written by us; the
    remainder is filled in by insta.

- **[Integration tests](book/src/integrations/README.md)** — these run tests
  against real databases, to ensure we're producing correct SQL.

- **[Examples](book/tests/snapshot.rs)** — we compile all examples in the PRQL
  Book, to test that they produce the SQL we expect, and that changes to our
  code don't cause any unexpected regressions.

- **[GitHub Actions on every commit](.github/workflows/pull-request.yaml)** — we
  run tests on `prql-compiler` for standard & wasm targets, and the examples in
  the book on every pull request every time a commit is pushed. These are
  designed to run in under two minutes, and we should be reassessing their scope
  if they grow beyond that. Once these pass, a pull request can be merged.

  All tests up to this point can be run with:

  ```sh
  task test-all
  ```

- **[GitHub Actions on specific changes](.github/workflows/)** — we run
  additional tests on pull requests when we identify changes to some paths, such
  as bindings to other languages.

- **[GitHub Actions on merge](.github/workflows/test-all.yaml)** — we run many
  more tests on every merge to main. This includes testing across OSs, all our
  language bindings, our `task` tasks, a measure of test code coverage, and some
  performance benchmarks.[^6]

  We can run these tests before a merge by adding a label `pr-test-all` to the
  PR.

  If these tests fail after merging, we revert the merged commit before fixing
  the test and then re-reverting.

[^6]:
    We reference "actions", such as
    [`build-prql-python`](.github/actions/build-prql-python/action.yaml) from
    workflows. We need to use these actions since workflow calls can only have a
    depth of 2 (i.e. workflow can call workflows, but those workflows can't call
    other workflows). We occasionally copy & paste small amounts of yaml where
    we don't want to abstract something tiny into another action.

    An alternative approach would be to have all jobs in a single workflow which
    is called on every change, and then each job contains all its filtering
    logic. So `pull-request.yaml` and `test-all.yaml` would be a single file,
    and `test-python` would a job that has an `if` containing a) path changes,
    b) a branch condition for `main`, and c) a PR label filter. That would be a
    "flatter" approach — each job contains all its own criteria. The downside
    would less abstraction, more verbose jobs, and a long list of ~25/30 skipped
    jobs on every PR (since each job is skipped, rather than never started).

    Ideally we wouldn't have to make these tradeoffs — GHA would offer an
    arbitrary DAG of workflows, with filters at each level, and a UI that less
    prominently displays workflows which aren't designed to run.

- **[GitHub Actions nightly](.github/workflows/nightly.yaml)** — we run tests
  that take a long time or are unrelated to code changes, such as security
  checks, or expensive timing benchmarks, every night.

  We can run these tests before a merge by adding a label `pr-cron` to the PR.

The goal of our tests is to allow us to make changes quickly. If you find
they're making it more difficult for you to make changes, or there are missing
tests that would give you the confidence to make changes faster, then please
raise an issue.

---

## Releasing

Currently we release in a semi-automated way:

1. PR & merge an updated [Changelog](CHANGELOG.md). GitHub will produce a draft
   version at <https://github.com/PRQL/prql/releases/new>, including "New
   Contributors".
2. Run `cargo release version patch -x && cargo release replace -x` to bump the
   versions, then PR the resulting commit.
3. After merging, go to
   [Draft a new release](https://github.com/PRQL/prql/releases/new), copy the
   changelog entry into the release notes, enter the tag to be created, and hit
   "Publish".
4. From there, both the tag and release is created and all packages are
   published automatically based on our
   [release workflow](.github/workflows/release.yaml).
5. Add in the sections for a new Changelog:

   ```md
   ## 0.4.X — [unreleased]

   **Features**:

   **Fixes**:

   **Documentation**:

   **Web**:

   **Integrations**:

   **Internal changes**:

   **New Contributors**:
   ```

We may make this more automated in future; e.g. automatic changelog creation.
