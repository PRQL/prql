---
####################### General #########################
layout: home
title: PRQL

hero_section:
  enable: true
  heading: "PRQL is a modern language for transforming data"
  bottom_text: "— a simple, powerful, pipelined SQL replacement"
  button:
    enable: false
    link: https://prql-lang.org/book/
    label: "Reference"
  # the PRQL example is defined in data/examples/hero.yaml

why_prql_section:
  enable: true
  title: "Why PRQL?"
  items:
    - title: For data engineers
      content:
        - PRQL is concise, with abstractions such as variables & functions
        - PRQL is database agnostic, compiling to many dialects of SQL
        - PRQL isn't limiting — it can contain embedded SQL where necessary
        - PRQL has bindings to most major languages _(and more are in progress)_
        - PRQL allows for column lineage and type inspection _(in progress)_
    - title: For analysts
      content:
        - PRQL is ergonomic for data exploration — for example, commenting out a
          filter, or a column in a list, maintains a valid query
        - PRQL is simple, and easy to understand, with a small number of
          powerful concepts
        - PRQL allows for powerful autocomplete, type-checking, and helpful
          error messages _(in progress)_
    - title: For tools
      content:
        - PRQL's vision is a foundation to build on; we're open-source and will
          never have a commercial product
        - PRQL is growing into a single secular standard which tools can target
        - PRQL is easy for machines to read & write
    - title: For HackerNews enthusiasts
      content:
        - The PRQL compiler is written in Rust
        - We talk about "orthogonal language features" a lot

showcase_section:
  enable: true
  title: "Showcase"
  content:
    - PRQL consists of a curated set of orthogonal transformations, which are
      combined together to form a pipeline. That makes it easy to compose and
      extend queries. The language also benefits from modern features, such
      syntax for dates, ranges and f-strings as well as functions, type checking
      and better null handling.
  buttons:
    - link: "/playground/"
      label: "Playground"
    - link: "/book/"
      label: "Book"
  examples:
    # The examples are defined in data/examples/, this list just defines their order.
    - basic
    - friendly-syntax
    - orthogonal
    - expressions
    - f-strings
    - windows
    - functions
    - top-n
    - s-strings
    - joins
    - null-handling
    - dialects

    # Currently excluded because it's lots of text
    # prql: |
    #   # Check out how much simpler this is relative to the SQL...

    #   let track_plays = (                     # Assign with `let`
    #     from plays
    #     group [track] (
    #       aggregate [
    #         total = count,
    #         unfinished = sum is_unfinished,
    #         started = sum is_started,
    #       ]
    #     )
    #   )

principles_section:
  enable: true
  title: "Principles"
  items:
    - title: "Pipelined"
      main_text: "A PRQL query is a linear pipeline of transformations"
      content:
        Each line of the query is a transformation of the previous line’s
        result. This makes it easy to read, and simple to write.

    - title: "Simple"
      main_text:
        "PRQL serves both sophisticated engineers and analysts without coding
        experience."
      content:
        By providing a small number of powerful & orthogonal primitives, queries
        are simple and composable — there's only one way of expressing each
        operation. We can eschew the debt that SQL has built up.

    - title: "Open"
      main_text: "PRQL is open-source, with an open community"
      content:
        PRQL will always be fully open-source and will never have a commercial
        product. By compiling to SQL, PRQL is compatible with most databases,
        existing tools, and programming languages that manage SQL. We're a
        welcoming community for users, contributors, and other projects.

    - title: "Extensible"
      main_text:
        "PRQL is designed to be extended, from functions to language bindings"
      content:
        PRQL has abstractions which make it a great platform to build on. Its
        explicit versioning allows changes without breaking
        backward-compatibility. And in the cases where PRQL doesn't yet have an
        implementation, it allows embedding SQL with s-strings.

    - title: "Analytical"
      main_text: "PRQL's focus is analytical queries"
      content:
        PRQL was originally designed to serve the growing need of writing
        analytical queries, emphasizing data transformations, development speed,
        and readability. We de-emphasize other SQL features such as inserting
        data or transactions.

videos_section:
  enable: true
  title: "Pipelines in action"
  items:
    - youtube_id: IQRRsfavEic

tools_section:
  enable: true
  title: "Tools"
  sections:
    - link: https://prql-lang.org/playground/
      label: "Playground"
      text:
        "Online in-browser playground that compiles PRQL to SQL as you type."

    - link: https://pyprql.readthedocs.io/
      label: "pyprql"
      text: |
        Provides Jupyter/IPython cell magic and Pandas accessor.

        `pip install pyprql`

    - link: https://crates.io/crates/prqlc
      label: "prqlc"
      text: |
        A CLI for PRQL compiler, written in Rust.

        `cargo install prqlc`

        `brew install prqlc`

        `winget install prqlc`

integrations_section:
  enable: true
  title: "Integrations"
  sections:
    - label: "Jupyter/IPython"
      link: https://pyprql.readthedocs.io/en/latest/magic_readme.html
      text:
        "pyprql contains a Jupyter extension, which executes a PRQL cell against
        a database. It can also set up an in-memory DuckDB instance, populated
        with a pandas DataFrame."

    - label: "ClickHouse"
      link: https://clickhouse.com/docs/en/guides/developer/alternative-query-languages
      text: |
        ClickHouse natively supports PRQL with

        `SET dialect = 'prql'`

    - label: Visual Studio Code
      link: https://marketplace.visualstudio.com/items?itemName=prql-lang.prql-vscode
      text: Extension with syntax highlighting and live SQL compilation.

    - label: "Prefect"
      link: https://prql-lang.org/book/project/integrations/prefect.html
      text: Add PRQL models to your Prefect workflows with a single function.

    - label: "DuckDB"
      link: https://github.com/ywelsch/duckdb-prql
      text: A DuckDB extension to execute PRQL

bindings_section:
  enable: true
  title: "Bindings"
  section_id: "bindings"
  sections:
    - link: https://pypi.org/project/prql-python
      label: "prql-python"
      text: Python bindings for prqlc.

    - link: https://www.npmjs.com/package/prql-js
      label: "prql-js"
      text: "JavaScript bindings for prqlc."

    - link: https://CRAN.R-project.org/package=prqlr
      label: "prqlr"
      text: "R bindings for prqlc."

    - link: "https://crates.io/crates/prqlc"
      label: "prqlc"
      text: |
        Compiler implementation, written in Rust. Compile, format & annotate PRQL queries.

    - link: https://prql-lang.org/book/project/bindings/index.html
      label: Others
      text: |
        Java, C, C++, Elixir, .NET, and PHP have unsupported or nascent bindings.

testimonials_section:
  enable: true
  title: "What people are saying"
  # The testimonials are in data/testimonials.yaml.
---
