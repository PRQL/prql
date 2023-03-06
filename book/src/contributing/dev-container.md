# Develop in Dev Containers

```admonish note
Currently the Dev Container included in this repository only supports the `amd64` platform.
```

[Dev Containers](https://containers.dev/) are a way to package a number of
"developer tools" (compilers, bundlers, package managers, loaders, etc.) into a
single object. This is helpful when many people want to contribute to a project:
each person only has to install the (single) Dev Container on their own machine
to start working. By definition, the Dev Container has a consistent set of tools
that are known to work together. This avoids a fuss with finding the proper
versions of each of the build tools.

To use a Dev Container on your local computer with VS Code, you must install the
[VS Code Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
and its system requirements.

## How you use it

While there are a variety of tools that support Dev Containers, the focus here
is on developing with VS Code in a container by
[GitHub Codespaces](https://docs.github.com/en/codespaces/overview) or
[VS Code Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers).

Please refer to the documents for general instructions on how to use these
tools.

## Using PRQL in a Dev Container

[Task](https://taskfile.dev/) is installed in the container for quick access to
tasks defined on the `Taskfiles.yml`. Autocompression works when using the `zsh`
shell installed in the container.

Here are some useful commands available in the container.

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
