# Using the Dockerfile

The `Dockerfile` in this repo builds a Docker container
that has current versions of all the development tools.
Using Docker means you do not have to concern
yourself that these tools will conflict with
other software on your computer.

First install Docker on your computer,
using one of the many guides on the internet.

Next build the Docker container with the following commands.
_(It will take some time while Docker pulls in all the
necessary developer tools.)_

```bash
cd <top-level-PRQL-directory>
docker build -t prql .
```

Finally, start up the Docker container with:

```bash
cd <top-level-PRQL-directory>
docker run -it -v $(pwd)/:/app -p 3000:3000 prql
```

- You'll see a `root@xxxxxxxxx:/app/#` prompt
- Enter the commands below for the component you're working on
- Ctl-C to exit that component
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

```
cd book
mdbook serve -n 0.0.0.0 -p 3000
```

**Website:** Use the command above, then enter:

```
cd website
hugo server --bind 0.0.0.0 -p 3000
```

**prql-compiler:** Use the command above, 
`cd prql-compiler` then read the **Usage** section of the [README.md](./prql-compiler/README.md)

**prql-java:** Use the command above, 
`cd prql-java ` then read the **Usage** section of the [README.md](./prql-java/README.md)

**prql-js:** Use the command above, 
`cd prql-js ` then read the **Usage** section of the [README.md](./prql-js/README.md)

**prql-lib:** Use the command above, 
`cd prql-lib ` then read the **Usage** section of the [README.md](./prql-lib/README.md)

**prql-macros:** Use the command above, 
`cd prql-macros ` then read the **Usage** section of the [README.md](./prql-macros/README.md)

**prql-python:** Use the command above, 
`cd prql-python ` then read the **Usage** section of the [README.md](./prql-python/README.md)
