# PRQL

[PRQL](https://prql-lang.org/) bindings for Elixir.

## Installation

```elixir
def deps do
  [
    {:prql, "~> 0.1.0"}
  ]
end
```

## Basic Usage

```elixir
  iex> PRQL.compile("from customers", signature_comment: false)
      {:ok, "SELECT\n  *\nFROM\n  customers\n"}


  iex> PRQL.compile("from customers\ntake 10", target: :mssql, signature_comment: false)
  {:ok, "SELECT\n  *\nFROM\n  customers\nORDER BY\n  (\n    SELECT\n      NULL\n  ) OFFSET 0 ROWS\nFETCH FIRST\n  10 ROWS ONLY\n"}
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
