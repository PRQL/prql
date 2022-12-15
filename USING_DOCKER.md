# Using the Dockerfile

The `Dockerfile` in this repo builds a Docker container
that has current versions of all the development tools.
Using Docker means you do not have to concern
yourself that these tools will conflict with
other software on your computer.

## Development cycle

The developer loop when using Docker is substantially the same as
if the tools had been installed directly.

All the source files live in the `prql` directory on your machine.
As you edit the source, the tools (wrapped in the Docker container)
watch those directories and re-run
so you can see your results instantly.

When you exit the Docker container (say, at the end of the development
session), the `prql` directory on the local machine contains the
latest files.
You can use `git` to pull or to push the `prql` repo as normal.

To do all this, build the Docker container and start it
as described in the **Installation** section.
Then read the separate steps in **Running components under Docker**
for each component you wish to work on.

## Installation

First install Docker on your computer,
using one of the many guides on the internet.

Next build the Docker container with the following commands.
_(It will take some time while Docker pulls in all the
necessary developer tools.)_

```bash
cd <top-level-PRQL-directory>
docker build -t prql .
```

_Optional:_ Install `pre-commit` on the machine that hosts Docker.
It runs several
[Static Checks](./DEVELOPMENT.md#tests) to ensure code consistency.
You can also configure `git` to run `pre-commit` automatically
for each commit with the second (one-time) command below.

```bash
pre-commit run -a                  # Run checks manually
pre-commit install --install-hooks # (one time) install the git hooks
```

Finally, start up the Docker container with:

```bash
cd <top-level-PRQL-directory>
docker run -it -v $(pwd)/:/src -p 3000:3000 prql
```

- You'll see a `root@xxxxxxxxx:/src/#` prompt
- Enter the commands below for the component you're working on
- Ctrl-C to exit that component
- Enter 'exit' to close down the Docker machine

## Running components under Docker

Currently Docker only supports running rust dependencies, though adding `hugo` &
`nodejs` such that the playground can run would be a welcome contribution.

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

**prql-compiler:** Use the command above,
`cd prql-compiler` then read the **Usage** section of the
[README.md](./prql-compiler/README.md)

## Developing the Dockerfile

When making updates to the Dockerfile, we have automated testing that the
Dockerfile builds on each merge, in
[**`test-all.yaml`**](.github/workflows/test-all.yaml), and automated testing
that the rust tests pass, in [**`cron.yaml`**](.github/workflows/cron.yaml).

Add a label to the PR `pr-test-all` or `pr-cron` to run these tests on a PR.
