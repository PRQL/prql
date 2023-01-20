# Using the Dockerfile

The `Dockerfile` in this repo builds a Docker image that has current versions of
our rust development tools. This can be the lowest-effort way of setting up a
rust environment for those that don't have one already.

## Development cycle

The developer loop when using Docker is substantially the same as if the tools
had been installed directly.

All the source files live in the `prql` directory on the host. As the source
changes, the tools (running in the Docker container) can watch those directories
and re-run so results are instantly visible.

When the Docker container exits (say, at the end of the development session),
the `prql` directory on the local machine contains the latest files. Use `git`
to pull or to push the `prql` repo from the host as normal.

To do all this, build the Docker image and start a container as described in the
**Installation** section.

## Installation

Once Docker is installed, build the Docker image with the following commands.

> Note: It will take some time while Docker pulls in all the necessary developer
> tools.

```bash
cd <top-level-PRQL-directory>
docker build -t prql .
```

_Optional:_ Install `pre-commit` on the machine that hosts Docker. It runs
several [Static Checks](./DEVELOPMENT.md#tests) to ensure code consistency. You
can also configure `git` to run `pre-commit` automatically for each commit with
the second (one-time) command below.

```bash
pre-commit run -a   # Run checks manually
pre-commit install  # (one time) install the git hooks
```

Finally, start up the Docker container with:

```bash
cd <top-level-PRQL-directory>
docker run -it -v $(pwd)/:/src -p 3000:3000 prql
```

- There'll be a `root@xxxxxxxxx:/src/#` prompt
- Enter a command to run or test code; for example `cargo test`
- Enter `exit` to stop the container

## Running code with Docker

Currently our Docker image only supports running rust dependencies. (adding
`hugo` & `nodejs` so that the playground can run would be a welcome
contribution.)

Use the `docker run...` command above, then enter the relevant commands; for
example `cargo insta test --accept` or `task run book` — more details of the
commands are in each component's `README.md` file or
[**`DEVELOPMENT.md`**](DEVELOPMENT.md).

> Note: The first time you run a component, it may take some time to install
> additional files. Once they're built, start up is quick.

<!-- Currently these aren't supported in docker — see notes in Dockerfile -->

<!-- **Playground:** Use the command above, then enter:

```bash
cd playground
npm install # first time only
npm start
```

**Language Book:** Use the command above, then enter these commands.
(The first time you run this, the container will compile many files.)

```bash
cd book
mdbook serve -n 0.0.0.0 -p 3000
```

**Website:** Use the command above, then enter:

```bash
cd website
hugo server --bind 0.0.0.0 -p 3000
``` -->

## Developing the Dockerfile

When making updates to the Dockerfile, we have automated testing that the
Dockerfile builds on each merge in
[**`test-all.yaml`**](.github/workflows/test-all.yaml), and automated testing
that the confirms all rust tests pass, in
[**`nightly.yaml`**](.github/workflows/nightly.yaml).

Add a label to the PR `pr-test-all` or `pr-cron` to run these tests on a PR.
