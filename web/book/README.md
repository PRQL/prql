# PRQL language book

These docs serve as a language book, for users of the language. They should be
friendly & accessible, at a minimum to those who understand basic SQL.

## Running

Install all required PRQL dev tools with:

```sh
task setup-dev
```

...or for the precise cargo command, run `cargo install --locked mdbook`. For
the complete build, add any `mdbook` crates listed in the `Taskfile.yaml`.

And then to build & serve locally[^1]:

```sh
task web:run-book
```

[^1]: ...which is equivalent to:

    ```sh
    cd book
    mdbook serve
    ```
