# PRQL

[PRQL](https://prql-lang.org/) bindings for Elixir.

## Installation

The package isn't published on Hex yet
([#1510](https://github.com/PRQL/prql/issues/1510)). Depend on a local checkout
of this repo instead; building it requires a Rust toolchain:

```elixir
def deps do
  [
    {:prql, path: "path/to/prql/prqlc/bindings/elixir"}
  ]
end
```

## Basic Usage

```elixir
  iex> PRQL.compile("from customers", signature_comment: false)
      {:ok, "SELECT\n  *\nFROM\n  customers\n"}


  iex> PRQL.compile("from customers\ntake 10", target: :mssql, signature_comment: false)
  {:ok, "SELECT\n  *\nFROM\n  customers\nORDER BY\n  (\n    SELECT\n      NULL) OFFSET 0 ROWS\nFETCH FIRST\n  10 ROWS ONLY\n"}
```

## Development

We are in the early stages of developing Elixir bindings.

We're using `Rustler` to provide Rust bindings for `prqlc`.

Currently using the bindings in an Elixir project requires compiling the Rust
crate from this repo:

- Install dependencies with `mix deps.get`
- Compile project `mix compile`
- Run tests `mix test`

Future work includes publishing pre-compiled artifacts, so Elixir projects can
run PRQL without needing a Rust toolchain.
