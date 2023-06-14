# Summary

<!-- markdownlint-disable MD042 — some pages aren't finished yet (though the graying out of top level pages is not ideal — it's either that, or links to pages that are blank. Or maybe we try and write a useful page for each heading?) -->

- [Introduction](./introduction.md)

- [Queries](./queries/README.md)

  - [Pipelines](./queries/pipelines.md)
  - [Functions](./queries/functions.md)
  - [Variables](./queries/variables.md)

- [Transforms](./transforms/README.md)

  - [Aggregate](./transforms/aggregate.md)
  - [Append](./transforms/append.md)
  - [Derive](./transforms/derive.md)
  - [Filter](./transforms/filter.md)
  - [From](./transforms/from.md)
  - [Group](./transforms/group.md)
  - [Join](./transforms/join.md)
  - [Select](./transforms/select.md)
  - [Sort](./transforms/sort.md)
  - [Take](./transforms/take.md)
  - [Window](./transforms/window.md)

- [Syntax](./syntax/README.md)

  - [Pipes](./syntax/pipes.md)
  - [Tuples](./syntax/tuples.md)
  - [Expressions & Operators](./syntax/expressions-and-operators.md)
  - [Inner Transforms](./syntax/inner-transforms.md)
  - [Comments](./syntax/comments.md)
  - [Quoted Identifiers](./syntax/quoted-identifiers.md)
  - [Parameters](./syntax/parameters.md)
  - [Numbers](./syntax/numbers.md)
  - [Keywords](./syntax/keywords.md)

- [Language features](./language-features/README.md)

  - [Case](./language-features/case.md)
  - [Coalesce](./language-features/coalesce.md)
    <!-- `DATE_TRUNC(foo_date, YEAR)` -> `foo_date.year`? Or -> `foo_date | as year`? Or `foo_date | to year`? -->
  - [Dates & times](./language-features/dates-and-times.md)
  - [Distinct](./language-features/distinct.md)
  - [Relation literals](./language-features/relation-literals.md)
  - [Null handling](./language-features/null.md)
  - [Ranges](./language-features/ranges.md)
  - [Regex](./language-features/regex.md)
  - [Standard library](./language-features/standard-library/README.md)
    - [Loop](./language-features/standard-library/loop.md)
    - [Reading files](./language-features/standard-library/reading-files.md)
  - [Strings](./language-features/strings.md)
  - [S-strings](./language-features/s-strings.md)
  - [F-strings](./language-features/f-strings.md)
  - [Target & Version](./language-features/target.md)

- [Bindings](./bindings/README.md)

  - [.NET](./bindings/dotnet.md)
  - [Elixir](./bindings/elixir.md)
  - [Java](./bindings/java.md)
  - [JavaScript](./bindings/javascript.md)
  - [Python](./bindings/python.md)
  - [R](./bindings/r.md)
  - [Rust](./bindings/rust.md)

- [Integrations](./integrations/README.md)

  - [`prqlc CLI`](./integrations/prqlc-cli.md)
  - [Jupyter](./integrations/jupyter.md)
  - [DuckDB](./integrations/duckdb.md)
  - [Prefect](./integrations/prefect.md)
  - [VS Code](./integrations/vscode.md)
  - [Rill](./integrations/rill.md)

- [Examples](./examples/README.md)

  - [Variables](./examples/variables.md)
  - [List equivalence](./examples/list-equivalence.md)
  - [CTE (intermediate tables)](./examples/cte.md)
  - [Employees](./examples/employees.md)

- [Compiler](./compiler/README.md)

  - [`prql-compiler`](./compiler/prql-compiler.md)
  - [Internals](./compiler/internals/README.md)
    - [Compiler architecture](./compiler/internals/compiler-architecture.md)
    - [Name resolving](./compiler/internals/name-resolving.md)
    - [Functions](./compiler/internals/functional-lang.md)
    - [Syntax highlighting](./compiler/internals/syntax-highlighting.md)

- [Contributing to PRQL](./contributing/README.md)

  - [Development](./contributing/development.md)

- [Changelog](./changelog.md)
