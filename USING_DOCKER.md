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
Use the `docker run...` command above, then:

**Playground** Use the command above, then enter:<br\>
`cd playground; npm start`

**Language Book** Use the command above, then enter:<br\>
`cd book; mdbook serve -n 0.0.0.0 -p 3000`

**Website** Use the command above, then enter:<br\>
`cd website; hugo server --bind 0.0.0.0 -p 3000`

(_[I don't know how to run/test the following components]_)

**prql-compiler** Use the command above, then enter:<br\>
`cd prql-compiler; ***what do people need to do?***`

**prql-java** Use the command above, then enter:<br\>
`cd prql-java; *** what do people need to do?***`

**prql-js** Use the command above, then enter:<br\>
`cd prql-js; ***what do people need to do?***`

**prql-lib** Use the command above, then enter:<br\>
`cd prql-lib; ***what do people need to do?***`

**prql-macros** Use the command above, then enter:<br\>
`cd prql-macros; ***what do people need to do?***`

**prql-python** Use the command above, then enter:<br\>
`cd prql-python; ***what do people need to do?***`
