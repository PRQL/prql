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
  prql_example: |
    from invoices
    filter invoice_date >= @1970-01-16
    derive {
      transaction_fees = 0.8,
      income = total - transaction_fees
    }
    filter income > 1
    group customer_id (
      aggregate {
        average total,
        sum_income = sum income,
        ct = count,
      }
    )
    sort {-sum_income}
    take 10
    join c=customers {==customer_id}
    derive name = f"{c.last_name}, {c.first_name}"
    select {
      c.customer_id, name, sum_income
    }
    derive db_version = s"version()"

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
        - PRQL is a stable foundation to build on; we're open-source and will
          never have a commercial product
        - PRQL is a single secular standard which tools can target
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
    - id: basics
      label: Basic example
      prql: |
        from employees
        select {id, first_name, age}
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
        from track_plays
        filter plays > 10_000                # Readable numbers
        filter (length | in 60..240)         # Ranges with `..`
        filter recorded > @2008-01-01        # Simple date literals
        filter released - recorded < 180days # Nice interval literals
        sort {-length}                       # Concise order direction

      sql: |
        SELECT
          *
        FROM
          track_plays
        WHERE
          plays > 10000
          AND length BETWEEN 60 AND 240
          AND recorded > DATE '2008-01-01'
          AND released - recorded < INTERVAL 180 DAY
        ORDER BY
          length DESC

    - id: orthogonal
      label: Orthogonality
      prql: |
        from employees
        # `filter` before aggregations...
        filter start_date > @2021-01-01
        group country (
          aggregate {max_salary = max salary}
        )
        # ...and `filter` after aggregations!
        filter max_salary > 100_000
      sql: |
        SELECT
          country,
          MAX(salary) AS max_salary
        FROM
          employees
        WHERE
          start_date > DATE '2021-01-01'
        GROUP BY
          country
        HAVING
          MAX(salary) > 100000

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

    - id: expressions
      label: Expressions
      prql: |
        from track_plays
        derive {
          finished = started + unfinished,
          fin_share = finished / started,        # Use previous definitions
          fin_ratio = fin_share / (1-fin_share), # BTW, hanging commas are optional!
        }

      sql: |
        SELECT
          *,
          started + unfinished AS finished,
          (started + unfinished) / started AS fin_share,
          (started + unfinished) / started / (1 - (started + unfinished) / started) AS fin_ratio
        FROM
          track_plays

    # markdown-link-check-disable
    - id: f-strings
      label: F-strings
      prql: |
        from web
        # Just like Python
        select url = f"https://www.{domain}.{tld}/{page}"
      sql: |
        SELECT CONCAT('https://www.', domain, '.', tld,
          '/', page) AS url
        FROM web
    # markdown-link-check-enable
    - id: windows
      label: Windows
      prql: |
        from employees
        group employee_id (
          sort month
          window rolling:12 (
            derive {trail_12_m_comp = sum paycheck}
          )
        )
      sql: |
        SELECT
          *,
          SUM(paycheck) OVER (
            PARTITION BY employee_id
            ORDER BY
              month ROWS BETWEEN 11 PRECEDING
              AND CURRENT ROW
          ) AS trail_12_m_comp
        FROM
          employees

    - id: functions
      label: Functions
      prql: |
        let fahrenheit_from_celsius = temp -> temp * 9/5 + 32

        from weather
        select temp_f = (fahrenheit_from_celsius temp_c)
      sql: |
        SELECT
          temp_c * 9 / 5 + 32 AS temp_f
        FROM
          weather

    - id: top-n
      label: Top N by group
      prql: |
        # Most recent employee in each role
        # Quite difficult in SQL...
        from employees
        group role (
          sort join_date
          take 1
        )
      sql: |
        WITH table_1 AS (
          SELECT
            *,
            ROW_NUMBER() OVER (
              PARTITION BY role
              ORDER BY
                join_date
            ) AS _expr_0
          FROM
            employees
        )
        SELECT
          *
        FROM
          table_1 AS table_0
        WHERE
          _expr_0 <= 1

    - id: s-string
      label: S-strings
      prql: |
        # There's no `version` in PRQL, but s-strings
        # let us embed SQL as an escape hatch:
        from x
        derive db_version = s"version()"
      sql: |
        SELECT
          *,
          version() AS db_version
        FROM x

    - id: joins
      label: Joins
      prql: |
        from employees
        join b=benefits {==employee_id}
        join side:left p=positions {p.id==employees.employee_id}
        select {employees.employee_id, p.role, b.vision_coverage}
      sql: |
        SELECT
          employees.employee_id,
          p.role,
          b.vision_coverage
        FROM
          employees
          JOIN benefits AS b ON employees.employee_id = b.employee_id
          LEFT JOIN positions AS p ON p.id = employees.employee_id

    - id: null-handling
      label: Null handling
      prql: |
        from users
        filter last_login != null
        filter deleted_at == null
        derive channel = channel ?? "unknown"
      sql: |
        SELECT
          *,
          COALESCE(channel, 'unknown') AS channel
        FROM
          users
        WHERE
          last_login IS NOT NULL
          AND deleted_at IS NULL

    - id: dialects
      label: Dialects
      prql: |
        prql target:sql.mssql  # Will generate TOP rather than LIMIT

        from employees
        sort age
        take 10
      sql: |
        SELECT
          TOP (10) *
        FROM
          employees
        ORDER BY
          age

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
      label: "PyPrql"
      text: |
        Provides Jupyter/IPython cell magic and Pandas accessor.

        `pip install pyprql`

    - link: https://crates.io/crates/prqlc
      label: "prqlc"
      text: |
        A CLI for PRQL compiler, written in Rust.

        `cargo install prqlc`

        `brew install prqlc`

integrations_section:
  enable: true
  title: "Integrations"
  sections:
    - label: "Jupyter/IPython"
      link: https://pyprql.readthedocs.io/en/latest/magic_readme.html
      text:
        "PyPrql contains a Jupyter extension, which executes a PRQL cell against
        a database. It can also set up an in-memory DuckDB instance, populated
        with a pandas DataFrame."

    - label: Visual Studio Code
      link: https://marketplace.visualstudio.com/items?itemName=prql-lang.prql-vscode
      text: Extension with syntax highlighting and an upcoming language server.

    - label: "Prefect"
      link: https://prql-lang.org/book/integrations/prefect.html
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
      text: Python bindings for prql-compiler.

    - link: https://www.npmjs.com/package/prql-js
      label: "prql-js"
      text: "JavaScript bindings for prql-compiler."

    - link: https://CRAN.R-project.org/package=prqlr
      label: "prqlr"
      text: "R bindings for prql-compiler."

    - link: "https://crates.io/crates/prql-compiler"
      label: "prql-compiler"
      text: |
        Reference compiler implementation, written in Rust. Transpile, format and annotate PRQL queries.

comments_section:
  enable: true
  title: "What people are saying"
  comments:
    # Tweets can be fetched with https://www.tweetic.io/docs

    - quote:
        text:
          It starts with FROM, it fixes trailing commas, and it's called PRQL??
          If this is a dream, don't wake me up.
        author: Jeremiah Lowin, Founder & CEO, Prefect.
    - tweet:
        user_id: "19042640"
        name: "Hamilton Ulmer"
        screen_name: "hamiltonulmer"
        profile_image_url_https: "https://pbs.twimg.com/profile_images/1201721914814656512/B6muDm76_normal.jpg"
        url: "https://twitter.com/hamiltonulmer/status/1522562664467107840"
        profile_url: "https://twitter.com/hamiltonulmer"
        created_at: "2022-05-06T13:03:21.000Z"
        favorite_count: 2
        conversation_count: 0
        text: very excited for prql!

    - tweet:
        user_id: "16080017"
        name: "Swanand."
        screen_name: "_swanand"
        profile_image_url_https: "https://pbs.twimg.com/profile_images/1607146722555224064/iTL4gp7m_normal.jpg"
        url: "https://twitter.com/_swanand/status/1485965394880131081"
        profile_url: "https://twitter.com/_swanand"
        created_at: "2022-01-25T13:18:52.000Z"
        favorite_count: 20
        conversation_count: 2
        text: >
          A few years ago, I started working on a language, called "dsql", short
          for declarative SQL, and a pun on "the sequel (to SQL)". I kinda
          chickened out of it then, the amount of study and research I needed
          was massive. prql here is better than I imagined:
          github.com/max-sixty/prql

    - quote:
        text:
          Column aliases would have saved me hundreds of hours over the course
          of my career.
        author: "@dvasdekis"
        link: https://news.ycombinator.com/item?id=30064873
    - tweet:
        user_id: "231773031"
        name: "Rishabh Software"
        screen_name: "RishabhSoft"
        profile_image_url_https: "https://pbs.twimg.com/profile_images/551974110566178817/JHuUzhjU_normal.png"
        url: "https://twitter.com/RishabhSoft/status/1514280454890872833"
        profile_url: "https://twitter.com/RishabhSoft"
        created_at: "2022-04-13T16:32:49.000Z"
        favorite_count: 0
        conversation_count: 0
        text: >
          SQL's hold on data retrieval is slipping! 8 new databases are
          emerging, and some speak entirely new languages for data querying.
          Know more infoworld.com/article/365490… #SQL #DataQuery #GraphQL #PRQL
          #WebAssembly

    - tweet:
        user_id: "40653789"
        name: "Burak Emir"
        screen_name: "burakemir"
        profile_image_url_https: "https://pbs.twimg.com/profile_images/215834651/BurakEmir-2007-04-23-full_normal.jpg"
        url: "https://twitter.com/burakemir/status/1485958835844100098"
        profile_url: "https://twitter.com/burakemir"
        created_at: "2022-01-25T12:52:48.000Z"
        favorite_count: 2
        conversation_count: 1
        text: >
          I want to give the PRQL a little boost here, "pipeline of
          transformations" is IMHO a good choice for readable query languages
          that need to deal with SQL-like aggregations, group by and count and
          sum all: github.com/max-sixty/prql
    - quote:
        text: >
          Having written some complex dbt projects...the first thing...it gets
          right is to start with the table and work down. This is an enormous
          readability boost in large projects and leads to great intellisense.
        author: Ruben Slabbert
        link: https://lobste.rs/s/oavgcx/prql_simpler_more_powerful_sql#c_nmzcd7
    - tweet:
        user_id: "103516223"
        name: "Michael Sumner"
        screen_name: "mdsumner"
        profile_image_url_https: "https://pbs.twimg.com/profile_images/1592964396091047937/CsbMRVRG_normal.jpg"
        url: "https://twitter.com/mdsumner/status/1485795181198983170"
        profile_url: "https://twitter.com/mdsumner"
        created_at: "2022-01-25T02:02:30.000Z"
        favorite_count: 1
        conversation_count: 0
        text: >
          what an *excellent* criticism of SQL #PRQL
          github.com/max-sixty/prql#an…
    - quote:
        text: Just wanna say, I absolutely love this.
        author: Alex Kritchevsky
        link: https://news.ycombinator.com/item?id=30063771
---
