---
####################### General #########################
layout: home
title: PRQL

####################### Hero section #########################
hero_section:
  enable: true
  heading: "PRQL is a modern language for transforming data"
  bottom_text: "— a simple, powerful, pipelined SQL replacement"
  button:
    enable: true
    link: https://prql-lang.org/book/
    label: "Reference"
  prql_example: |
    from employees
    derive [
      gross_salary = salary + payroll_tax,
      gross_cost = gross_salary + benefits_cost
    ]
    filter gross_cost > 0
    group [title, country] (
      aggregate [
        average salary,
        sum     salary,
        average gross_salary,
        sum     gross_salary,
        average gross_cost,
        sum_gross_cost = sum gross_cost,
        ct = count,
      ]
    )
    sort [sum_gross_cost, -country]
    filter ct > 200
    take 20

####################### Principles section #########################
principle_section:
  enable: true
  title: "Principles"
  items:
    - title: "Pipelined"
      main_text: "A PRQL query is a linear pipeline of transformations"
      content:
        Each line of the query is a transformation of the previous line’s result.
        This makes it easy to read, and simple to write.

    - title: "Simple"
      main_text: "PRQL serves both sophisticated engineers and analysts without coding experience."
      content:
        By providing a small number of powerful & orthogonal primitives, queries are simpler —
        there's only one way of expressing each operation. We can eschew the debt that SQL has built up.

    - title: "Open"
      main_text: "PRQL will always be open-source"
      content:
        PRQL is free-as-in-free, will never have a commercial product, and doesn’t prioritize one database over others.
        By compiling to SQL, PRQL is instantly compatible with most databases, and existing tools or programming languages that manage SQL.
        Where possible, PRQL unifies syntax across databases.

    - title: "Extensible"
      main_text: "PRQL can be extended through functions"
      content: PRQL has abstractions which make it a great platform to build on.
        Its explicit versioning allows changes without breaking backward-compatibility.
        And in the cases where PRQL doesn't yet have an implementation, it allows embedding SQL with S-Strings.

    - title: "Analytical"
      main_text: "PRQL's focus is analytical queries"
      content: We de-emphasize other SQL features such as inserting data or transactions.

showcase_section:
  enable: true
  title: "Showcase"
  content:
    - PRQL consists of a curated set of orthogonal transformations, which are combined together to form a pipeline.
      That makes it easy to compose and extend queries. The language also benefits from modern features, such syntax for dates, ranges and f-strings as well as functions, type checking and better null handling.
  buttons:
    - link: "/examples/"
      label: "More examples"
    - link: "/playground/"
      label: "Playground"
    - link: "/book/"
      label: "Book"
  examples:
    - id: basics
      label: Basic example
      prql: |
        from employees
        select [id, first_name, age]
        sort age
        take 10
      sql: |
        SELECT id, first_name, age
        FROM employees
        ORDER BY age
        LIMIT 10

    - id: friendly-syntax
      label: Friendly syntax
      prql: |
        from order  # this is a comment
        filter created_at > @2022-06-13  # dates
        filter status == "done"
        sort [-amount]  # sort order
      sql: |
        SELECT
          order.*,
          amount * COALESCE(promo, 0) AS promo_amount
        FROM order
        WHERE created_at > DATE '2022-06-13'
          AND status = 'done'
        ORDER BY amount DESC

    - id: null-handling
      label: Null handling
      prql: |
        from users
        filter last_login != null
        filter deleted_at == null
        derive channel = channel ?? "unknown"
      sql: |
        SELECT
          users.*,
          COALESCE(channel, 'unknown') AS channel
        FROM
          users
        WHERE
          last_login IS NOT NULL
          AND deleted_at IS NULL
    # markdown-link-check-disable
    - id: f-strings
      label: F-strings
      prql: |
        from web
        select url = f"http://www.{domain}.{tld}/{page}"
      sql: |
        SELECT CONCAT('http://www.', domain, '.', tld,
          '/', page) AS url
        FROM web
    # markdown-link-check-enable
    - id: functions
      label: Functions
      prql: |
        func fahrenheit_from_celsius temp -> temp * 9/5 + 32

        from weather
        select temp_f = (fahrenheit_from_celsius temp_c)
      sql: |
        SELECT
          temp_c * 9/5 + 32 AS temp_f
        FROM
          weather

tools_section:
  enable: true
  title: "Tools"
  sections:
    - link: https://prql-lang.org/playground/
      label: "Playground"
      text: "Online in-browser playground that compiles PRQL to SQL as you type."

    - link: "https://github.com/prql/prql"
      label: "prql-compiler"
      text: |
        Reference compiler implementation. Has a CLI utility that can transpile, format and annotate PRQL queries.

        `cargo install prql`

        `brew install prql/prq/prql`

    - link: https://github.com/prql/PyPrql
      label: "PyPrql"
      text: |
        Python TUI for connecting to databases.
        Provides a native interactive console with auto-complete for column names and Jupyter/IPython cell magic.

        `pip install pyprql`

integrations_section:
  enable: true
  title: "Integrations"
  sections:
    - label: dbt
      link: https://github.com/prql/dbt-prql
      text: |
        Allows writing PRQL in dbt models.
        This combines the benefits of PRQL's power & simplicity within queries; with dbt's version control, lineage & testing across queries.

    - label: "Jupyter/IPython"
      link: https://pyprql.readthedocs.io/en/latest/magic_readme.html
      text: |
        PyPrql has a magic extension, which executes a PRQL cell against a database.
        It can also set up an in-memory DuckDB instance, populated with a pandas DataFrame.

    - label: Visual Studio Code
      link: https://marketplace.visualstudio.com/items?itemName=prql.prql
      text: Extension with syntax highlighting and an upcoming language server.

    - label: "Prefect"
      text: Upcoming.

bindings_section:
  enable: true
  title: "Bindings"
  sections:
    - link: https://crates.io/crates/prql-compiler
      label: "prql-compiler"
      text: "PRQL's compiler library, written in Rust."

    - link: https://pypi.org/project/prql-python
      label: "prql-python"
      text: "Python bindings for prql-compiler."

    - link: https://www.npmjs.com/package/prql-js
      label: "prql-js"
      text: "JavaScript bindings for prql-compiler."

comments_section:
  enable: true
  title: "What people are saying"
  tweets:
    # NB: These uses a custom shortcode
    - "{{< tweet 1485965394880131081 >}}"
    - "{{< tweet 1514280454890872833 >}}"
    - "{{< tweet 1485958835844100098 >}}"
    - "{{< tweet 1485795181198983170 >}}"
    - "{{< tweet 1522562664467107840 >}}"
  quotes:
    - quote:
        {
          text: "It starts with FROM, it fixes trailing commas, and it's called PRQL?? If this is a dream, don't wake me up.",
          author: "Jeremiah Lowin, Founder & CEO, Prefect.",
        }
---
