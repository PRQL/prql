# Summary

<!-- markdownlint-disable MD042 — some pages aren't finished yet (though the graying out of top level pages is not ideal — it's either that, or links to pages that are blank. Or maybe we try and write a useful page for each heading?) -->

- [Introduction](./introduction.md)

- [Queries](./queries/README.md)

  - [Pipelines](./queries/pipelines.md)
  - [Functions](./queries/functions.md)
  - [Tables](./queries/tables.md)

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

- [Syntax](./syntax.md)
- [Language features](./language-features/README.md)

  - [Coalesce](./language-features/coalesce.md)
    <!-- `DATE_TRUNC(foo_date, YEAR)` -> `foo_date.year`? Or -> `foo_date | as year`? Or `foo_date | to year`? -->
  - [Dates & times](./language-features/dates-and-times.md)
  - [Distinct](./language-features/distinct.md)
  - [Null handling](./language-features/null.md)
  - [Ranges](./language-features/ranges.md)
    <!--   - Regex — `REGEX_MATCH(foo, "\\w{3}")` -> `foo ~ r"\w{3}"`? Or -> `regex foo r"\w{3}"`? -->
  - [Regex](./language-features/regex.md)
  - [Stdlib](./language-features/standard-library.md)
  - [Strings](./language-features/strings.md)
  - [S-Strings](./language-features/s-strings.md)
  - [F-Strings](./language-features/f-strings.md)
  - [Switch](./language-features/switch.md)
  - [Target & Version](./language-features/target.md)

- [Bindings](./bindings/README.md)

  - [Java](./bindings/java.md)
  - [JavaScript](./bindings/javascript.md)
  - [Python](./bindings/python.md)
  - [R](./bindings/r.md)
  - [Rust](./bindings/rust.md)
  - [Elixir](./bindings/elixir.md)

- [Integrations](./integrations/README.md)

  - [dbt](./integrations/dbt.md)
  - [Jupyter](./integrations/jupyter.md)
  - [Prefect](./integrations/prefect.md)
  - [VSCode](./integrations/vscode.md)
  - [Rill](./integrations/rill.md)

- [Examples](./examples/README.md)

  - [Variables](./examples/variables.md)
  - [List equivalence](./examples/list-equivalence.md)
  - [CTE (intermediate tables)](./examples/cte.md)
  - [Employees](./examples/employees.md)

- [Contributing to PRQL](./contributing/README.md)

  - [Development](./contributing/development.md)
  - [Using Docker](./contributing/using-docker.md)

- [Internals](./internals/README.md)

  - [Compiler architecture](./internals/compiler-architecture.md)
  - [Name resolving](./internals/name-resolving.md)
  - [Functions](./internals/functional-lang.md)
  - [Syntax highlighting](./internals/syntax-highlighting.md)
