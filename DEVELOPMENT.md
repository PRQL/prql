# Development environment

Setting up a local dev environment for PRQL is simple, thanks to the rust ecosystem:

- Install [`rustup` & `cargo`](https://doc.rust-lang.org/cargo/getting-started/installation.html)[^5].
- That's it! Running `cargo test` should complete successfully.
- For more advanced development; e.g. adjusting `insta` outputs or compiling for
  web, run `task setup-dev` by installing
  [Task](https://taskfile.dev/#/installation), or by copying & pasting them from
  [Taskfile.yml](Taskfile.yml).
- For quick contributions, hit `.` in GitHub to launch a [github.dev
  instance](https://github.dev/prql/prql).
- Any problems: post an issue and we'll help.

[^5]:
    We'll need a clang compiler to compile the DuckDB integration tests,
    since we use [`duckdb-rs'](https://github.com/wangfenjin/duckdb-rs). To install a compiler:

    - On Mac, install xcode `xcode-select --install`
    - On Linux, `apt-get install libclang-dev`
    - On Windows, `duckdb-rs` doesn't work anyway, so these tests are excluded

    It's also possible to develop `prql-compiler` without using the integration tests. Running
    `cargo test --lib` should complete successfully by running only the unit tests.

## Components of PRQL

The PRQL project has several components.
Instructions for working with them are in the **README.md** file in their directory.
Here's an overview:

**playground:** A web GUI for the PRQL compiler.
It shows the PRQL source beside the resulting SQL output.
[README.md](./playground/README.md)

**book:** Tools to build the PRQL language book that documents the language.
[README.md](./book/README.md)

**website:** Tools to build the `hugo` website.
[README.md](./website/README.md)

**prql-compiler:** Installation and usage instructions for building and running the `prql-compiler`.
[README.md](./prql-compiler/README.md)

**prql-java:** Rust bindings to the `prql-compiler` rust library.
[README.md](./prql-java/README.md)

**prql-js:** Javascript bindings to the `prql-compiler` rust library.
[README.md](./prql-js/README.md)

**prql-lib:** Generates `.a` and `.so` libraries from the `prql-compiler` rust library for bindings to other languages
[README.md](./prql-lib/README.md)

**prql-macros:** rust macros for PRQL [README.md](./prql-macros/README.md)

**prql-python:** Python bindings to the `prql-compiler` rust library. [README.md](./prql-python/README.md)

## `task`

We have a couple of tasks which incorporate all building & testing. While they
don't need to be run as part of a standard dev loop — generally we'll want to
run a more specific test — they can be useful as a backstop to ensure everything
works, and as a reference for how each part of the repo is built & tested. They
should be broadly consistent with the GitHub Actions workflows; please report
any inconsistencies.

To build everything:

```sh
task build-all
```

To run all tests; (which includes building everything):

```sh
task test-all
```

These require installing Task, either `brew install go-task/tap/go-task` or
as described on [Task](https://taskfile.dev/#/installation).

## Tests

We use a pyramid of tests — we have fast, focused tests at the bottom of the
pyramid, which give us low latency feedback when developing, and then slower,
broader tests which ensure that we don't miss anything as PRQL develops[^1].

<!-- markdownlint-disable MD053 -->

[^1]:
    Our approach is very consistent with
    **[@matklad](https://github.com/matklad)**'s advice, in his excellent blog
    post [How to Test](https://matklad.github.io//2021/05/31/how-to-test.html).

> If you're making your first contribution, you don't need to engage with all this
> — it's fine to just make a change and push the results; the tests that run in
> GitHub will point you towards any errors, which can be then be run locally if
> needed. We're always around to help out.

Our tests:

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

- **Unit tests & inline insta snapshots** — like most projects, we rely on
  unit tests to test that our code basically works. We extensively use
  [Insta](https://insta.rs/), a snapshot testing tool which writes out the
  results of an expression in our code, making it faster to write and modify
  tests[^3].

  These are the fastest tests which run our code; they're designed to run on
  every save while you're developing. (While they're covered by `task test-all`,
  you'll generally want to have lower-latency tests running in a tight
  loop.)[^2]

[^2]: For example, this is a command I frequently run:

    ```sh
    RUST_BACKTRACE=1 watchexec -e rs,toml,pest,md -cr -- cargo insta test --accept -- -p prql-compiler --lib
    ```

    Breaking this down:

    - `RUST_BACKTRACE=1` will print a full backtrace, including where an error
      value was created, for rust tests which return `Result`s.
    - `watchexec -e rs,toml,pest,md -cr --` will run the subsequent command on any
      change to files with extensions which we are generally editing.
    - `cargo insta test --accept --` runs tests with `insta`, a snapshot library, and
      writes any results immediately. I rely on git to track changes, so I run
      with `--accept`, but YMMV.
    - `-p prql-compiler --lib` is passed to cargo by `insta`; `-p prql-compiler`
      tells it to only run the tests for `prql-compiler` rather than the other
      crates, and `--lib` to only run the unit tests rather than the integration
      tests, which are much slower.
    - Note that we don't want to re-run on _any_ file changing, because we can get into a
      loop of writing snapshot files, triggering a change, writing a snapshot
      file, etc.

[^3]:
    [Here's an example of an insta
    test](https://github.com/prql/prql/blob/0.2.2/prql-compiler/src/parser.rs#L580-L605)
    — note that only the initial line of each test is written by us; the remainder
    is filled in by insta.

- **[Integration
  tests](https://github.com/prql/prql/blob/main/prql-compiler/tests/integration/README.md)**
  — these run tests against real databases, to ensure we're producing correct
  SQL.

- **[Examples](https://github.com/prql/prql/blob/main/book/tests/snapshot.rs)**
  — we compile all examples in the PRQL Book, to test that they produce the SQL
  we expect, and that changes to our code don't cause any unexpected
  regressions.

- **[GitHub Actions on every
  commit](https://github.com/prql/prql/blob/main/.github/workflows/pull-request.yaml)**
  — we run tests on `prql-compiler` for standard & wasm targets, and the
  examples in the book on every pull request every time a commit is pushed.
  These are designed to run in under two minutes, and we should be reassessing
  their scope if they grow beyond that. Once these pass, a pull request can be
  merged.

  All tests up to this point can be run with `task test-all` locally.

- **[GitHub Actions on specific
  changes](https://github.com/prql/prql/blob/main/.github/workflows/test-all.yaml)**
  — we run additional tests on pull requests when we identify changes to some
  paths, such as bindings to other languages.

- **[GitHub Actions on
  merge](https://github.com/prql/prql/tree/main/.github/workflows)** — we run
  many more tests on every merge to main. This includes testing across OSs, all
  our language bindings, our `task` tasks, a measure of test code coverage, and
  some performance benchmarks.

  We can run these tests before a merge by adding a label `pr-test-all` to the
  PR.

  If these tests fail after merging, we revert the merged commit before fixing the test and
  then re-reverting.

The goal of our tests is to allow us to make changes quickly. If you find
they're making it more difficult for you to make changes, or there are missing
tests that would give you the confidence to make changes faster, then please
raise an issue.

## Releases

Currently we release in a semi-automated way:

- PR & merge an updated [Changelog](CHANGELOG.md).
- Run `cargo release --no-push --no-tag --no-publish -x patch` locally to bump
  the versions, then PR the resulting commit.
- After merging, go to [Draft a new
  release](https://github.com/prql/prql/releases/new), copy the changelog entry
  into the release notes, select a new tag to be created, and hit the "Publish"
  button.
- From there, all packages are published automatically based on our [release
  workflow](.github/workflows/release.yaml).
- Add in the sections for a new Changelog:

  ```md
  ## 0.2.X — [unreleased]

  Features:

  Fixes:

  Documentation:

  Web:

  Integrations:

  Internal changes:
  ```

We may make this more automated in future; e.g. automatic changelog creation.
