# Bindings

PRQL has bindings for many languages. These include:

We have three tiers of bindings:

- Supported
- Unsupported
- Nascent

## Supported

Supported bindings require:

- A maintainer.
- Implementations of the
  [core compile functions](https://docs.rs/prql-compiler/latest/prql_compiler/#functions).
- Test coverage for these functions.
- A published package to the language's standard package repository.
- A script in `Taskfile.yml` to bootstrap a development environment.
- Any dev tools, such as a linter & formatter, in pre-commit or MegaLinter.

The currently supported bindings are:

- [JavaScript](./javascript.md)
- [Python](./python.md)
- [R](./r.md)
- [Rust](./rust.md)

Most of these are in the main PRQL repo, and we gate any changes to the
compiler's API on compatible changes to the bindings.

## Unsupported

Unsupported bindings work, but don't fulfil all of the above criteria. We don't
gate changes to the compiler's API. If they stop working, we'll demote them to
nascent.

- [Java](./java.md)
- [Elixir](./elixir.md)
- `prqlc-clib`, the C bindings

## Nascent

Nascent bindings are in development, and may not yet fully work.

- [.NET](./dotnet.md)
- [PHP](./php.md)

## Naming

Over time, we're trying to move to a consistent naming scheme:

- Crates are named `prqlc-$lang`.
- Where possible, packages are published to each language's package repository
  as `prqlc`.
