# PRQL Language Reference

These docs serve as a language reference, for users of the language. They should
be friendly & accessible, at a minimum to those who understand basic SQL. They
should not contain rust code!

## Running

Install all required PRQL dev tools with:

```sh
task install-dev-tools
```

...or if an individual install is preferred:

```sh
cargo install --locked mdbook
```

And then to build & serve locally:

```sh
cd reference
mdbook serve
```

## Preprocessors

As described in [**book.toml**](book.toml), we have a few preprocessors which
convert the markdown into the code displayed on the site. Some of these are
quite hacky, and will likely not work on Windows. If this is a problem, please
post an issue and we'll try and find a workaround.
