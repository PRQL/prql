---
title: "PRQL"
date: 2022-05-14
layout: 'single'
no_head: true
---

**P**ipelined **R**elational **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a modern language for transforming data — a simpler and more powerful
SQL.

```prql
from employees                                # Each line transforms the previous result.
filter start_date > @2021-01-01               # Clear date syntax.
derive [                                      # `derive` adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost   # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (                      # `group` runs a pipeline over each group.
  aggregate [                                 # `aggregate` reduces each group to a row.
    average salary,
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost = sum gross_cost,          # `=` sets a column name.
    ct = count,
  ]
)
sort [sum_gross_cost, -country]               # `-country` means descending order.
filter ct > 200
take 20
```

## Principles

- ***Pipelined*** — PRQL is a linear pipeline of transformations — each line of the
  query is a transformation of the previous line's result. This makes it easy to
  read, and simple to write.

- ***Simple*** — PRQL serves both sophisticated engineers and analysts without
  coding experience. 
  We believe that there should be only one way of expressing each operation,
  so there is only a few patterns to memorize. This opposes query tweaking with 
  intention to improve performance, because that should be handled by the compiler and 
  the database.

- ***Open*** — PRQL will always be open-source, free-as-in-free, and doesn't
  prioritize one database over others. By compiling to SQL, PRQL is instantly
  compatible with most databases, and existing tools or programming languages
  that manage SQL. Where possible, PRQL unifies syntax across databases.

- ***Extensible*** — PRQL can be extended through its abstractions, and its explicit
  versioning allows changes without breaking backward-compatibility. PRQL allows
  embedding SQL through S-Strings, where PRQL doesn't yet have an
  implementation.

- ***Analytical*** — PRQL's focus is analytical queries; we de-emphasize other SQL
  features such as inserting data or transactions.

## Motivation

Even though wildly adopted and readable as a sentence, SQL is inconsistent and becomes
unmanageable as soon as complexity grows beyond the most simple queries.

<!-- expand this?  -->

<!-- markdown-link-check-disable-next-line -->
[Here are examples](./motivation/) on how PRQL compares to SQL analytical queries.

<!-- something about unifying pandas/dplyr/data.table? -->
## Tools

- [prql-compiler](https://github.com/prql/prql) - reference compiler implementation.
  - Install with `cargo`: `cargo install prql`
  <!-- Brew not yet working, tbc -->
  - Install with `brew`: `brew install prql`
- [PyPrql](https://github.com/prql/PyPrql) - python TUI for connecting to databases.
  It has some great features, including a native interactive console with auto-complete
  for column names.
  - Install with `pip`: `pip install pyprql`
- [prql-python](https://pypi.org/project/prql-python/) - Python compiler library.
- [prql-js](https://www.npmjs.com/package/prql-js) - JavaScript compiler library.
- [PRQL Playground](/playground/) - in-browser playground.

## Integrations

- [Visual Studio Code extension](https://marketplace.visualstudio.com/items?itemName=prql.prql), 
  provides syntax highlighting and an upcoming language server
- [Jupyter/IPython](https://pyprql.readthedocs.io/en/latest/magic_readme.html): 
  PyPrql has a magic extension, which executes a PRQL cell against a database. 
  It can also set up an in-memory DuckDB instance, populated with a pandas
  dataframes.
- `dbt-prql`: upcoming
- `prefect-prql`: upcoming

## Keep in touch

- Star [the main repo](https://github.com/prql/prql).

- Send a link to PRQL to a couple of people whose opinion you respect.

- Subscribe to [GitHub issue #1](https://github.com/prql/prql/issues/1) for
  a GitHub notification on updates.

- Rewrite any of your own queries to PRQL to see if it makes sense. You can use the 
  [playground](./playground/) and submit issues [here](https://github.com/prql/prql/issues).
  We are looking for any use-cases that expose a poor design choice, a need of a feature, 
  a pain point or just a sharp edge of the language.

- Join the [Discord](https://discord.gg/eQcfaCmsNc).
  <!-- TODO: Replace with a link to a CONTRIBUTING.md  -->

- [Contribute to PRQL](https://github.com/prql/prql/issues)
  <!-- Do we start a Twitter?? Maybe better to have nothing than one that's hardly used? Would be great to have a way for people to say "Yes I'd like to know more about this", without having to commit to joining the discord. Very open to other options; we could even do something like a Substack with updates for each release? -->
