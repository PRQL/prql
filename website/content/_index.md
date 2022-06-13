---
####################### Genaral #########################
layout: home
title: PRQL

####################### Hero section #########################
hero_section:
  enable: true
  heading: "PRQL is a modern language for transforming data"
  bottom_text: "-- a simpler and more powerful SQL"
  button:
    enable: true
    link: "https://prql-lang.org/reference/"
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
      main_text: "PRQL is a linear pipeline of transformations"
      content: "Each line of the query is a transformation of the previous line’s result. This makes it easy to read, and simple to write."

    - title: "Simple"
      main_text: "PRQL serves both sophisticated engineers and analyst's without coding experience."
      content: |
        We believe that there should be only one way of expressing each operation,
        so there is only a few patterns to memorize. This opposes query tweaking with
        intention to improve performance, because that should be handled by the compiler and
        the database.

    - title: "Open"
      main_text: "PRQL will always be open-source"
      content: "Free-as-in-free, and doesn’t prioritize one database over others. By compiling to SQL, PRQL is instantly compatible with most databases, and existing tools or programming languages that manage SQL. Where possible, PRQL unifies syntax across databases."

    - title: "Extensible"
      main_text: "PRQL can be extended through its abstractions"
      content: "Its explicit versioning allows changes without breaking backward-compatibility. PRQL allows embedding SQL through S-Strings, where PRQL doesn’t yet have an implementation."

    - title: "Analytical"
      main_text: "PRQL’s focus is analytical queries"
      content: "We de-emphasize other SQL features such as inserting data or transactions."

####################### SQL Section #########################
showcase_section:
  enable: true
  title: "Showcase"
  subtitile: "Get familiar with your data"
  content:
    - "Even though wildly adopted and readable as a sentence, SQL is inconsistent and becomes unmanageable as soon as query complexity goes beyond the most simple queries."
    - "Because each transform in PRQL is orthogonal to all previous transforms, it is always easy to extend your query. On top of that, PRQL offers modern features, such syntax for dates, ranges and f-strings as well as functions, type checking and better null handling."
  buttons:
  - link: "/examples/"
    label: "More examples"
  
  - link: "/playground/"
    label: "Playground"
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
  
  - id: syntax
    label: Syntax
    prql: |
      from order  # this is a comment
      filter created_at > @2022-06-13 
      filter status == "done"
      derive promo_amount = amount * (promo ?? 0)
      sort [-amount]
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
    sql: |
      SELECT users.*
      FROM users
      WHERE last_login IS NOT NULL
        AND deleted_at IS NULL

  - id: f-strings
    label: F-strings
    prql: |
      from web
      select url = f"http://www.{domain}.{tld}/{page}"
    sql: |
      SELECT CONCAT('http://www.', domain, '.', tld, 
        '/', page) AS url
      FROM web

  - id: functions
    label: Functions
    prql: |
      func celsius_of_fahrenheit temp -> (temp - 32) * 3

      from cities
      select temp_c = (celsius_of_fahrenheit temp_f)
    sql: |
      SELECT temp_f - 32 * 3 AS temp_c
      FROM cities

tools_section:
  enable: true
  title: "TOOLS"
  sections:
    - link: "https://prql-lang.org/playground/"
      label: "Playground"
      text: "Online in-browser playground that compiles PRQL to SQL as you type."

    - link: "https://github.com/prql/prql"
      label: "prql-compiler"
      text: |
        Reference compiler implementation. Has a CLI utility that can transpile, format and annotate PRQL queries.
        
        `cargo install prql`

        `brew install prql`

    - link: https://github.com/prql/PyPrql
      label: "PyPrql"
      text: |
        Python TUI for connecting to databases. 
        Provides a native interactive console with auto-complete for column names and Jupyter/IPython cell magic.
        
        `pip install pyprql`

libraries_section:
  enable: true
  title: "LIBRARIES"
  sections:
    - link: "https://pypi.org/project/pyprql/"
      label: "prql-py"
      text: "Python compiler library. Wrapper for prql-compiler."

    - link: "https://www.npmjs.com/package/prql-js"
      label: "prql-js"
      text: "JavaScript compiler library. Wrapper for prql-compiler."

integrations_section:
  enable: true
  title: "INTEGRATIONS"
  sections:
    - label: Visual Studio Code 
      link: 'https://marketplace.visualstudio.com/items?itemName=prql.prql'
      text: Extension with syntax highlighting and an upcoming language server.

    - label: 'Jupyter/IPython'
      link: 'https://pyprql.readthedocs.io/en/latest/magic_readme.html'
      text: |
        PyPrql has a magic extension, which executes a PRQL cell against a database. 
        It can also set up an in-memory DuckDB instance, populated with a pandas dataframes.

    - label: DBT
      link: 'https://github.com/prql/dbt-prql'
      text: |
        Allows writing PRQL in dbt models. 
        This combines the benefits of PRQL's power & simplicity within queries, with dbt's version control, lineage & testing across queries.

    - label: 'prefect-prql'
      text: Upcoming.

---
