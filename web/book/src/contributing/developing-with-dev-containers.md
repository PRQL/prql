# Developing with Dev Containers

```admonish note
Currently the Dev Container included in this repository only supports the `amd64` platform.
```

[Dev Containers](https://containers.dev/) are a way to package a number of
developer tools (compilers, bundlers, package managers, loaders, etc.) into a
single object. This is helpful when many people want to contribute to a project:
each person only has to install the Dev Container on their own machine to start
working. By definition, the Dev Container has a consistent set of tools that are
known to work together. This avoids a fuss with finding the proper version of
each of the build tools.

While there are a variety of tools that support Dev Containers, the focus here
is on developing with VS Code in a container by
[GitHub Codespaces](https://docs.github.com/en/codespaces/overview) or
[VS Code Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers).

To use a Dev Container on a local computer with VS Code, install the
[VS Code Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
and its system requirements. Then refer to the links above to get started.
