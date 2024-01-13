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
  cargo test --package prql-compiler --lib
  ```

  ...or, to run tests and update the test snapshots:

  ```sh
  cargo insta test --accept --package prql-compiler --lib
  ```

  There's more context on our tests in [How we test](#how-we-test) below.

That's sufficient for making an initial contribution to the compiler.

---

## Setting up a full dev environment

```admonish info
We really care about this process being easy, both because the
project benefits from more contributors like you, and to reciprocate your
future contribution. If something isn't easy, please let us know in a GitHub
Issue. We'll enthusiastically help you, and use your feedback to improve the
scripts & instructions.
```

For more advanced development; for example compiling for wasm or previewing the
website, we have two options:

### Option 1: Use the project's `task`

```admonish note
This is tested on macOS, should work on amd64 Linux, but won't work on others (include Windows),
since it relies on `brew`.
```

- [Install Task](https://taskfile.dev/installation/).
- Then run the `setup-dev` task. This runs commands from our
  [Taskfile.yml](https://github.com/PRQL/prql/blob/main/Taskfile.yml),
  installing dependencies with `cargo`, `brew`, `npm` & `pip`, and suggests some
  VS Code extensions.

  ```sh
  task setup-dev
  ```

### Option 2: Install tools individually

- We'll need `cargo-insta`, to update snapshot tests:

  ```sh
  cargo install cargo-insta
  ```

- We'll need Python, which most systems will have already. The easiest way to
  check is to try running the full tests:

  ```sh
  cargo test
  ```

  ...and if that doesn't complete successfully, ensure we have Python >= 3.7, to
  compile `prql-python`.

- For more involved contributions, such as building the website, playground,
  book, or some release artifacts, we'll need some additional tools. But we
  won't need those immediately, and the error messages on what's missing should
  be clear when we attempt those things. When we hit them, the
  [Taskfile.yml](https://github.com/PRQL/prql/blob/main/Taskfile.yml) will be a
  good source to copy & paste instructions from.

### Option 3: Use a Dev Container

This project has a
[devcontainer.json file](https://github.com/PRQL/prql/blob/main/.devcontainer/devcontainer.json)
and a
[pre-built dev container base Docker image](https://github.com/PRQL/prql/pkgs/container/prql-devcontainer-base).
Learn more about Dev Containers at
[https://containers.dev/](https://containers.dev/)

Currently, the tools for Rust are already installed in the pre-built image, and,
Node.js, Python and others are configured to be installed when build the
container.

While there are a variety of tools that support Dev Containers, the focus here
is on developing with VS Code in a container by
[GitHub Codespaces](https://docs.github.com/en/codespaces/overview) or
[VS Code Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers).

To use a Dev Container on a local computer with VS Code, install the
[VS Code Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
and its system requirements. Then refer to the links above to get started.

### Option 4: Use nix development environment

A [nix](https://nixos.org/) flake `flake.nix` provides 3 development
environments:

- **default**, for building the compiler
- **web**, for the compiler and the website,
- **full**, for the compiler, the website and the compiler bindings.

To load the shell:

1. [Install nix (the package manager)](https://nixos.org/download). (only first
   time)

2. Enable flakes, which are a (pretty stable) experimental feature of nix. (only
   first time)

   For non-NixOS users:

   ```
   mkdir -p ~/.config/nix/
   tee 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf
   ```

   For NixOs users, follow instructions [here](https://nixos.wiki/wiki/Flakes).

3. Run:

   ```
   nix develop
   ```

   If you want "web" or "full" shell, run:

   ```
   nix develop .#web
   ```

Optionally, you can install [direnv](https://direnv.net/), to automatically load
the shell when you enter this repo. The easiest way is to also install
[direnv-nix](https://github.com/nix-community/nix-direnv) and configure your
`.envrc` with:

```
# .envrc
use flake .#full
```

---

## Contribution workflow

We're similar to most projects on GitHub — open a Pull Request with a suggested
change!

### Commits

- If a change is user-facing, please add a line in
  [**`CHANGELOG.md`**](https://github.com/PRQL/prql/blob/main/CHANGELOG.md),
  with `{message}, ({@contributor, #X})` where `X` is the PR number.
  - If there's a missing entry, a follow-up PR containing just the changelog
    entry is welcome.
- We're using [Conventional Commits](https://www.conventionalcommits.org)
  message format, enforced through
  [action-semantic-pull-request](https://github.com/amannn/action-semantic-pull-request).

### Merges

- **We merge any code that makes PRQL better**
- A PR doesn't need to be perfect to be merged; it doesn't need to solve a big
  problem. It needs to:
  - be in the right direction,
  - make incremental progress,
  - be explicit on its current state, so others can continue the progress.
- That said, there are a few instances when we need to ensure we have some
  consensus before merging code — for example non-trivial changes to the
  language, or large refactorings to the library.
- If you have merge permissions, and are reasonably confident that a PR is
  suitable to merge (whether or not you're the author), feel free to merge.
  - If you don't have merge permissions and have authored a few PRs, ask and ye
    shall receive.
- The primary way we ratchet the code quality is through automated tests.
  - This means PRs almost always need a test to demonstrate incremental
    progress.
  - If a change breaks functionality without breaking tests, our tests were
    probably insufficient.
  - If a change breaks existing tests (for example, changing an external API),
    that indicates we should be careful about merging a change, including
    soliciting others' views.
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
  resolved the issue; it's a sign of moving quickly. Other options which resolve
  issues immediately are also fine, such as commenting out an incorrect test or
  adding a quick fix for the underlying issue.

## Docs

We're very keen on contributions to improve our documentation.

This includes our docs in the book, on the website, in our code, or in a Readme.
We also appreciate issues pointing out that our documentation was confusing,
incorrect, or stale — if it's confusing for you, it's probably confusing for
others.

Some principles for ensuring our docs remain maintainable:

- Docs should be as close as possible to the code. Doctests are ideal on this
  dimension — they're literally very close to the code and they can't drift
  apart since they're tested on every commit. Or, for example, it's better to
  add text to a `--help` message, rather than write a paragraph in the Readme
  explaining the CLI.
- We should have some visualization of how to maintain docs when we add them.
  Docs have a habit of falling out of date — the folks reading them are often
  different from those writing them, they're sparse from the code, generally not
  possible to test, and are rarely the by-product of other contributions. Docs
  that are concise & specific are easier to maintain.
- Docs should be specifically relevant to PRQL; anything else we can instead
  link to.

If something doesn't fit into one of these categories, there are still lots of
ways of getting the word out there — a blog post / gist / etc. Let us know and
we're happy to link to it / tweet it.

## How we test

We use a pyramid of tests — we have fast, focused tests at the bottom of the
pyramid, which give us low latency feedback when developing, and then slower,
broader tests which ensure that we don't miss anything as PRQL
develops{{footnote: Our approach is very consistent with
**[@matklad](https://github.com/matklad)**'s advice, in his excellent blog
post [How to
Test](https://matklad.github.io//2021/05/31/how-to-test.html).}}.

<!-- markdownlint-disable MD053 -->

```admonish info
If you're making your first contribution, you don't need to engage with all
this — it's fine to just make a change and push the results; the tests that
run in GitHub will point you towards any errors, which can be then be run
locally if needed. We're always around to help out.
```

Our tests, from the bottom of the pyramid to the top:

- **[Static checks](https://github.com/PRQL/prql/blob/main/.pre-commit-config.yaml)**
  — we run a few static checks to ensure the code stays healthy and consistent.
  They're defined in
  [**`.pre-commit-config.yaml`**](https://github.com/PRQL/prql/blob/main/.pre-commit-config.yaml),
  using [pre-commit](https://pre-commit.com). They can be run locally with

  ```sh
  task test-lint
  # or
  pre-commit run -a
  ```

  The tests fix most of the issues they find themselves. Most of them also run
  on GitHub on every commit; any changes they make are added onto the branch
  automatically in an additional commit.

  - Checking by [MegaLinter](https://megalinter.io/latest/), which includes more
    Linters, is also done automatically on GitHub. (experimental)

- **Unit tests & inline insta snapshots** — we rely on unit tests to rapidly
  check that our code basically works. We extensively use
  [Insta](https://insta.rs/), a snapshot testing tool which writes out the
  values generated by our code, making it fast & simple to write and modify
  tests{{footnote:
  [Here's an example of an insta test](https://github.com/PRQL/prql/blob/0.2.2/prql-compiler/src/parser.rs#L580-L605)
  — note that only the initial line of each test is written by us; the
  remainder is filled in by insta.}}

  These are the fastest tests which run our code; they're designed to run on
  every save while you're developing. We include a `task` which does this:

  ```sh
  task test-rust-fast
  # or
  cargo insta test --accept --package prql-compiler --lib
  # or, to run on every change:
  task -w test-rust-fast
  ```

<!--
This is the previous doc. It has the advantage that it explains what it's doing, and is
easy to change (e.g. to run all packages). But because of
https://github.com/watchexec/watchexec/issues/371, the ignore behavior is unfortunately quite
inconsistent in watchexec. Let's revert back if it gets solved.

[^2]: For example, this is a command I frequently run:

    ```sh
    RUST_BACKTRACE=1 watchexec -e rs,toml,md -cr --ignore='target/**' -- cargo -q insta test --accept -p prql-compiler --lib
    ```

    Breaking this down:

    - `RUST_BACKTRACE=1` will print a full backtrace, including where an error
      value was created, for Rust tests which return `Result`s.
    - `watchexec -e rs,toml,md -cr --ignore='target/**' --` will run the
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

- **[Documentation](https://github.com/PRQL/prql/tree/main/web/book/tests/documentation)**
  — we compile all examples from our documentation in the Website, README, and
  PRQL Book, to test that they produce the SQL we expect, and that changes to
  our code don't cause any unexpected regressions. These are included in:

  ```sh
  cargo insta test --accept
  ```

- **[Database integration tests](https://github.com/PRQL/prql/tree/main/prqlc/prqlc/tests/integration/dbs)**
  — we run tests with example queries against databases with actual data to
  ensure we're producing correct SQL across our supported dialects. The
  in-process tests can be run locally with:

  ```sh
  task test-rust
  # or
  cargo insta test --accept --features=default,test-dbs
  ```

  More details on running with external databases are in the
  [Readme](https://github.com/PRQL/prql/tree/main/prqlc/prqlc/tests/integration/dbs).

```admonish note
Integration tests use DuckDB, and so require a clang compiler to compile
[`duckdb-rs`](https://github.com/wangfenjin/duckdb-rs). Most development
systems will have one, but if the test command fails, install a clang compiler with:

  - On macOS, install xcode with `xcode-select --install`
  - On Debian Linux, `apt-get update && apt-get install clang`
  - On Windows, `duckdb-rs` isn't supported, so these tests are excluded
```

- **[GitHub Actions on every commit](https://github.com/PRQL/prql/blob/main/.github/workflows/tests.yaml)**
  — we run tests relevant to a PR's changes in CI — for example changes to docs
  will attempt to build docs, changes to a binding will run that binding's
  tests. The vast majority of changes trigger tests which run in less than five
  minutes, and we should be reassessing their scope if they take longer than
  that. Once these pass, a pull request can be merged.

- **[GitHub Actions on merge](https://github.com/PRQL/prql/blob/c042eef48709e2c1af577161554fd09f14e67e0f/.github/workflows/pull-request.yaml#L124)**
  — we run a wider set tests on every merge to main. This includes testing
  across OSs, all our language bindings, a measure of test code coverage, and
  some performance benchmarks.

  If these tests fail after merging, we should revert the commit before fixing
  the test and then re-reverting.

  Most of these will run locally with:

  ```sh
  task test-all
  ```

- **[GitHub Actions nightly](https://github.com/PRQL/prql/blob/main/.github/workflows/nightly.yaml)**
  — every night, we run tests that take longer, are less likely to fail, or are
  unrelated to code changes — such as security checks, bindings' tests on
  multiple OSs, or expensive timing benchmarks.

  We can run these tests before a merge by adding a label `pr-nightly` to the
  PR.

The goal of our tests is to allow us to make changes quickly. If they're making
it more difficult to make changes, or there are missing tests that would offer
the confidence to make changes faster, please raise an issue.

---

## Website

The website is published together with the book and the playground, and is
automatically built and released on any push to the `web` branch.

The `web` branch points to the latest release plus any website-specific fixes.
That way, the compiler behavior in the playground matches the latest release
while allowing us to fix mistakes in the docs with a tighter loop than every
release.

Fixes to the playground, book, or website should have a `pr-backport-web` label
added to their PR — a bot will then open & merge another PR onto the `web`
branch once the initial branch merges.

The website components will run locally with:

```sh
# Run the main website
task run-website
# Run the PRQL online book
task run-book
# Run the PRQL playground
task run-playground
```

---

## Releasing

Currently we release in a semi-automated way:

1. PR & merge an updated
   [Changelog](https://github.com/PRQL/prql/blob/main/CHANGELOG.md). GitHub will
   produce a draft version at <https://github.com/PRQL/prql/releases/new>,
   including "New Contributors".

   Use this script to generate the first line:

   ```sh
   echo "This release has $(git rev-list --count $(git rev-list --tags --max-count=1)..) commits from $(git shortlog --summary $(git rev-list --tags --max-count=1).. | wc -l | tr -d '[:space:]') contributors. Selected changes:"
   ```

2. If the current version is correct, then skip ahead. But if the version needs
   to be changed — for example, we had planned on a patch release, but instead
   require a minor release — then run
   `cargo release version $version -x && cargo release replace -x` to bump the
   version and PR the resulting commit.

3. After merging, go to
   [Draft a new release](https://github.com/PRQL/prql/releases/new){{footnote: Only
       maintainers have access to this page.}}, copy the changelog entry into the
   release description{{footnote: Unfortunately GitHub's markdown parser
        interprets linebreaks as newlines. I haven't found a better way of
        editing the markdown to look reasonable than manually editing the text
        or asking LLM to help.}}, enter the tag to be created, and hit
   "Publish".

4. From there, both the tag and release is created and all packages are
   published automatically based on our
   [release workflow](https://github.com/PRQL/prql/blob/main/.github/workflows/release.yaml).

5. Run
   `cargo release version patch -x --no-confirm && cargo release replace -x --no-confirm`
   to bump the versions and add a new Changelog section; then PR the resulting
   commit.

6. Check whether there are [milestones](https://github.com/PRQL/prql/milestones)
   that need to be pushed out.

7. Review the **Current Status** on the README.md to ensure it reflects the
   project state.

We may make this more automated in future; e.g. automatic changelog creation.
