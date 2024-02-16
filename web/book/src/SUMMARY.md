<!-- markdownlint-disable MD042 — some pages aren't finished yet (though the graying out of top level pages is not ideal — it's either that, or links to pages that are blank. Or maybe we try and write a useful page for each heading?) -->

[Overview](./overview.md)

# Tutorial

- [Relations](./tutorial/relations.md)
- [Filtering](./tutorial/filtering.md)
- [Aggregation](./tutorial/aggregation.md)

# How do I?

- [Read files?](./how-do-i/read-files.md)
- [Remove duplicates?](./how-do-i/distinct.md)
- [Create ad-hoc relations?](./how-do-i/relation-literals.md)

# Reference

- [Syntax](./reference/syntax/README.md)

  - [Literals](./reference/syntax/literals.md)
  - [Strings](./reference/syntax/strings.md)
    - [F-strings](./reference/syntax/f-strings.md)
    - [R-strings](./reference/syntax/r-strings.md)
    - [S-strings](./reference/syntax/s-strings.md)
  - [Tuples](./reference/syntax/tuples.md)
  - [Arrays](./reference/syntax/arrays.md)
  - [Identifiers & keywords](./reference/syntax/keywords.md)
  - [Function calls](./reference/syntax/function-calls.md)
  - [Pipes](./reference/syntax/pipes.md)
  - [Operators](./reference/syntax/operators.md)
  - [Case](./reference/syntax/case.md)
  - [Ranges](./reference/syntax/ranges.md)
  - [Comments](./reference/syntax/comments.md)
  - [Parameters](./reference/syntax/parameters.md)

- [Declarations]()
  <!-- I don't know what to call this section. -->

  - [Variables](./reference/declarations/variables.md)
  - [Functions](./reference/declarations/functions.md)

- [Standard library](./reference/stdlib/README.md)

  - [Transforms](./reference/stdlib/transforms/README.md)

    - [Aggregate](./reference/stdlib/transforms/aggregate.md)
    - [Append](./reference/stdlib/transforms/append.md)
    - [Derive](./reference/stdlib/transforms/derive.md)
    - [Filter](./reference/stdlib/transforms/filter.md)
    - [From](./reference/stdlib/transforms/from.md)
    - [Group](./reference/stdlib/transforms/group.md)
    - [Join](./reference/stdlib/transforms/join.md)
    - [Loop](./reference/stdlib/transforms/loop.md)
    - [Select](./reference/stdlib/transforms/select.md)
    - [Sort](./reference/stdlib/transforms/sort.md)
    - [Take](./reference/stdlib/transforms/take.md)
    - [Window](./reference/stdlib/transforms/window.md)

  - [Aggregation functions]()
  - [Date functions](./reference/stdlib/date.md)
  - [Mathematical functions](./reference/stdlib/math.md)
  - [Text functions](./reference/stdlib/text.md)

- [Specification](./reference/spec/README.md)

  - [Null handling](./reference/spec/null.md)
  - [Name resolution](./reference/spec/name-resolution.md)
  - [Modules](./reference/spec/modules.md)
  - [Type system](./reference/spec/type-system.md)

# Project

- [Changelog](./project/changelog.md)

- [Target & version](./project/target.md)

- [Bindings](./project/bindings/README.md)

  - [.NET](./project/bindings/dotnet.md)
  - [Elixir](./project/bindings/elixir.md)
  - [Java](./project/bindings/java.md)
  - [JavaScript](./project/bindings/javascript.md)
  - [PHP](./project/bindings/php.md)
  - [Python](./project/bindings/python.md)
  - [R](./project/bindings/r.md)
  - [Rust](./project/bindings/rust.md)

- [Integrations](./project/integrations/README.md)

  - [`prqlc CLI`](./project/integrations/prqlc-cli.md)
  - [ClickHouse](./project/integrations/clickhouse.md)
  - [Jupyter](./project/integrations/jupyter.md)
  - [DuckDB](./project/integrations/duckdb.md)
  - [Prefect](./project/integrations/prefect.md)
  - [VS Code](./project/integrations/vscode.md)
  - [Rill](./project/integrations/rill.md)
  - [Syntax highlighting](./project/integrations/syntax-highlighting.md)
  - [PostgreSQL](./project/integrations/postgresql.md)

- [Contributing to PRQL](./project/contributing/README.md)

  - [Development](./project/contributing/development.md)
  - [Language design](./project/contributing/language-design.md)
