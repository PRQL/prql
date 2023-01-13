# PRQL Language Book

These docs serve as a language book, for users of the language. They should be
friendly & accessible, at a minimum to those who understand basic SQL.

## Running

Install all required PRQL dev tools with:

```sh
task setup-dev
```

...or if an individual install is preferred:

```sh
cargo install --locked mdbook
```

And then to build & serve locally[^1]:

```sh
task run-book
```

[^1]: ...which is equivalent to:

    ```sh
    cd book
    mdbook serve
    ```

## Preprocessors

As described in [**book.toml**](book.toml), we have a few preprocessors which
convert the markdown into the code displayed on the site. Some of these are
quite hacky, and will likely not work on Windows. If this is a problem, please
post an issue and we'll try and find a workaround.
