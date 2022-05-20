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
from employees
filter country = "USA"                        # Each line transforms the previous result.
derive [                                      # `derive` adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost  # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (                      # `group` runs a pipeline over each group
  aggregate [                                 # `aggregate` reduces a column to a row
    average salary,
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    ct: count,
  ]
)
sort sum_gross_cost
filter ct > 200
take 20
```

## Principles

PRQL is a modern language for transforming data — a simpler and more powerful
SQL. Like SQL, it's readable, explicit and declarative. Unlike SQL, it forms a
logical pipeline of transformations, and supports abstractions such as variables
and functions. It can be used with any database that uses SQL, since it
transpiles to SQL.

PRQL's principles:

- *Pipelined* — PRQL is a linear pipeline of transformations — each line of the
  query is a transformation of the previous line's result. This makes it easy to
  read, and simple to write.
- *Simple* — PRQL serves both sophisticated engineers and analysts without
  coding experience. By providing simple, clean abstractions, the
  language can be both powerful and easy to use.
- *Open* — PRQL will always be open-source, free-as-in-free, and doesn't
  prioritize one database over others. By compiling to SQL, PRQL is instantly
  compatible with most databases, and existing tools or programming languages
  that manage SQL. Where possible, PRQL unifies syntax across databases.
- *Extensible* — PRQL can be extended through its abstractions, and its explicit
  versioning allows changes without breaking backward-compatibility. PRQL allows
  embedding SQL through S-Strings, where PRQL doesn't yet have an
  implementation.
- *Analytical* — PRQL's focus is analytical queries; we de-emphasize other SQL
  features such as inserting data or transactions.

## Motivation

Even though wildly adopted and readable as a sentence, SQL is inconsistent and becomes
unmanageable as soon as query complexity goes beyond the most simple queries.

<!-- expand this?  -->

<!-- markdown-link-check-disable-next-line -->
[Here are examples](./motivation/) on how PRQL can simplifies analytical SQL queries.

<!-- something about unifying pandas/dplyr/data.table? -->
## Tools

- [prql-compiler](https://github.com/prql/prql) reference compiler implementation,
- [PyPrql](https://github.com/prql/PyPrql) python TUI for connecting to databases.
  Has some great features, including a native interactive console with auto-complete
  for column names,
- [prql-py](https://pypi.org/project/pyprql/) Python compiler library,
- [prql-js](https://www.npmjs.com/package/prql-js) JavaScript compiler library.

## Integrations

- Use a plugin for your existing tool:
  - `dbt-prql`: TODO
  - `jupyter`: TODO
- Install the compiler:
  - Install with `cargo`: `cargo install prql`
  - Install with `pip`: `pip install pyprql`
    <!-- Brew not yet working, tbc -->
  - Install with `brew`: `brew install prql`

## Keep in touch

- Star this repo.
- Send a link to PRQL to a couple of people whose opinion you respect.
- Subscribe to [GitHub issue #1](https://github.com/prql/prql/issues/1) for
  a GitHub notification on updates.
- Join the [Discord](https://discord.gg/eQcfaCmsNc).
  <!-- TODO: Replace with a link to a CONTRIBUTING.md  -->
- [Contribute to PRQL](https://github.com/prql/prql)
  <!-- Do we start a Twitter?? Maybe better to have nothing than one that's hardly used? Would be great to have a way for people to say "Yes I'd like to know more about this", without having to commit to joining the discord. Very open to other options; we could even do something like a Substack with updates for each release? -->
