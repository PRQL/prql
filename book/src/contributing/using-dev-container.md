# Development in Dev Containers

```admonish note
This document is a heavy work in progress and may not contain sufficient information.
```

[Dev Containers](https://containers.dev/) are a way to package a number of "developer tools" (compilers,
bundlers, package managers, loaders, etc.) into a single object. This is helpful
when many people want to contribute to a project: each person only has to
install the (single) Dev Container on their own machine to start working. By
definition, the Dev Container has a consistent set of tools that are known to
work together. This avoids a fuss with finding the proper versions of each of
the build tools.

To use a Dev Container on your local computer with VS Code, you must install the
[VS Code Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
extension and its system requirements.

When you first start the Dev Container, the build process may take a long time
(as much as 20-40 minutes) as the container collects all the packages of the
full PRQL toolchain. Once it's running, though, startup is fairly fast.

## How you use it

1. Clone the git repo onto your hard drive as usual. Then you start the Dev
  Container (say, using VS Code) that bundles all the developer tools.
2. Edit files locally (say, using VS Code) to work on PRQL. The tools in the Dev
  Container watch for changed files and rebuild the project as needed.
3. When you are satisfied with the changes, you can commit them to the repo, and
  push the changes as usual.

## Starting and stopping the Dev Container

_Instructions needed - both from VS Code and maybe from CLI?._

## Using PRQL in a Dev Container

After the Dev Container starts up, the VS Code Terminal pane shows a Dev
Container command line. Enter one of the following commands to work on a
component of PRQL.

VS Code offers a link to "Open in Browser" to the proper port to see the results
of your changes. When you are done working on that component, hit ^C to abort,
and return to the Dev Container command line.

- `task -l` lists all the available tasks.
- `task run-book` starts an `mdbook` server. As you edit the files of the
  Language Book (in the `book` directory), `mdbook` rebuilds those pages.
  (Port 3000)
- `task run-website` starts a `hugo` server. As you edit the files (in the
  `website` directory), `hugo` rebuilds those pages. (Port 1313)
- `task run-playground` starts a Node server to build the Playground. As you
  edit the files (in the `playground` directory), the server rebuilds those
  pages. (Port 3000)
- `task WHAT ELSE?` _Provide explanation of other useful commands._
