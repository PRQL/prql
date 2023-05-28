# Compile target

```admonish note
See also the [Target & Version](../../../language-features/target.md) language features page.
```

## Priority of targets

The compile target of a query is defined in the top of the query or compiler's option.
If different targets are specified in these two places, the option specified in the compiler takes precedence.

For example, the following shell example specifies `sql.generic` in the query and `sql.duckdb` in the `--target` option of the `prqlc compile` command.
In this case, `sql.duckdb` takes precedence and the SQL output is based on the DuckDB dialect.

```sh
echo 'prql target:sql.generic
      from foo' | prqlc compile --target sql.duckdb
```

To use the target described in the query, a special target `sql.any` can be specified in the compiler option.

```sh
echo 'prql target:sql.generic
      from foo' | prqlc compile --target sql.any
```
