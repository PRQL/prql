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
