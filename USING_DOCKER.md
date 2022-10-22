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
docker run -it -v $(pwd)/:/app -p 3000:3000 prql
```

- You'll see a `root@xxxxxxxxx:/app/#` prompt
- Enter the commands below for the component you're working on
- Ctrl-C to exit that component
- Enter 'exit' to close down the Docker machine

## Running components under Docker

The following commands come from the `README.md`
file in each of the component directories.
Use the `docker run...` command above, then enter
the command(s) below.

_(The first time you run a component, it may take some
time to install additional files.
Once they're built, start up is quick.)_

**Playground:** Use the command above, then enter:

```bash
cd playground
npm install # first time only
npm start
```

**Language Book:** Use the command above, then enter:

```bash
cd book
mdbook serve -n 0.0.0.0 -p 3000
```

**Website:** Use the command above, then enter:

```bash
cd website
hugo server --bind 0.0.0.0 -p 3000
```

**prql-compiler:** Use the command above,
`cd prql-compiler` then read the **Usage** section of the
[README.md](./prql-compiler/README.md)

**prql-java:** Use the command above,
`cd prql-java` then read the **Usage** section of the [README.md](./prql-java/README.md)

**prql-js:** Use the command above,
`cd prql-js` then read the **Usage** section of the [README.md](./prql-js/README.md)

**prql-lib:** Use the command above,
`cd prql-lib` then read the **Usage** section of the [README.md](./prql-lib/README.md)

**prql-macros:** Use the command above,
`cd prql-macros` then read the **Usage** section of the [README.md](./prql-macros/README.md)

**prql-python:** Use the command above,
`cd prql-python` then read the **Usage** section of the [README.md](./prql-python/README.md)

## Manual testing for Dockerfile

While the Dockerfile is under development, use these minimal tests
before committing new code.

1. **Check the Taskfile.yml** Run these commands to ensure that the
   `Taskfile.yml` still builds the "normal" environment:

   ```bash
   cd <directory-with-prql>
   cargo test
   task setup-dev
   ```

1. **Build the Docker container** as described above.

1. **Quick tests for the Docker container**
   Start the container (as described above),
   then check the various components (also, as described above)

1. **Examine the Github actions/workflows** for errors in
   your own repo before pushing to the main `prql/prql` repo.
