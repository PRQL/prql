<!-- markdownlint-disable MD042 — some pages aren't finished yet (though the graying out of top level pages is not ideal — it's either that, or links to pages that are blank. Or maybe we try and write a useful page for each heading?) -->

# Introduction

[Introduction](./README.md)

# Tutorial

A friendly & accessible guide for learning PRQL. It has a gradual increase of
difficulty and requires only basic understanding of programming languages.
Knowledge of SQL is beneficial, because of many comparisons to SQL, but not
required.

- [Relations](./tutorial/relations.md)
- [Filtering](./tutorial/filtering.md)
- [Aggregation](./tutorial/aggregation.md)

<!-- We used to have a "How do I", which I think would be good, but we didn't build enough to maintain it. If we find the Reference or Tutorial has enough content that we could move here, we could start it again  -->
<!-- # How do I? -->

# Reference

In-depth information about the PRQL language. Includes justifications for
language design decisions and formal specifications for parts of the language.

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

- [Importing data](./reference/data/README.md)

  - [From](./reference/data/from.md)
  - [Reading files](./reference/data/read-files.md)
  - [Ad-hoc data](./reference/data/relation-literals.md)

- [Declarations]()
  <!-- I don't know what to call this section. -->

  - [Variables — `let` & `into`](./reference/declarations/variables.md)
  - [Functions](./reference/declarations/functions.md)

- [Standard library](./reference/stdlib/README.md)

  - [Transforms](./reference/stdlib/transforms/README.md)

    - [Aggregate](./reference/stdlib/transforms/aggregate.md)
    - [Append](./reference/stdlib/transforms/append.md)
    - [Derive](./reference/stdlib/transforms/derive.md)
    - [Filter](./reference/stdlib/transforms/filter.md)
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
  - [Removing duplicates](./reference/stdlib/distinct.md)

- [Specification](./reference/spec/README.md)

  - [Null handling](./reference/spec/null.md)
  - [Name resolution](./reference/spec/name-resolution.md)
  - [Modules](./reference/spec/modules.md)
  - [Type system](./reference/spec/type-system.md)

# Project

General information about the project, tooling and development.

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
