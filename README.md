# PRQL

<!-- User badges on first line (language docs & chat) -->

[![Language Docs](https://img.shields.io/badge/DOCS-LANGUAGE-blue?style=for-the-badge)](https://prql-lang.org)
[![Discord](https://img.shields.io/discord/936728116712316989?label=discord%20chat&style=for-the-badge)](https://discord.gg/eQcfaCmsNc)
[![Twitter](https://img.shields.io/twitter/follow/prql_lang?color=%231DA1F2&style=for-the-badge)](https://twitter.com/prql_lang)

<!-- Dev badges on first line (language docs & chat) -->

[![GitHub CI Status](https://img.shields.io/github/actions/workflow/status/PRQL/prql/test-all.yaml?branch=main&logo=github&style=for-the-badge)](https://github.com/PRQL/prql/actions?query=branch%3Amain+workflow%3Atest-all)
[![GitHub contributors](https://img.shields.io/github/contributors/PRQL/prql?style=for-the-badge)](https://github.com/PRQL/prql/graphs/contributors)
[![Stars](https://img.shields.io/github/stars/PRQL/prql?style=for-the-badge)](https://github.com/PRQL/prql/stargazers)

**P**ipelined **R**elational **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a modern language for transforming data — a simple, powerful, pipelined
SQL replacement. Like SQL, it's readable, explicit and declarative. Unlike SQL,
it forms a logical pipeline of transformations, and supports abstractions such
as variables and functions. It can be used with any database that uses SQL,
since it compiles to SQL.

PRQL can be as simple as:

```elm
from employees
filter country == "USA"                       # Each line transforms the previous result
aggregate [                                   # `aggregate` reduces each column to a value
  max salary,
  min salary,
  count,                                      # Trailing commas are allowed
]
```

Here's a fuller example of the language;

```elm
from employees
filter start_date > @2021-01-01               # Clear date syntax
derive [                                      # `derive` adds columns / variables
  gross_salary = salary + (tax ?? 0),         # Terse coalesce
  gross_cost = gross_salary + benefits_cost,  # Variables can use other variables
]
filter gross_cost > 0
group [title, country] (                      # `group` runs a pipeline over each group
  aggregate [                                 # `aggregate` reduces each group to a value
    average gross_salary,
    sum_gross_cost = sum gross_cost,          # `=` sets a column name
  ]
)
filter sum_gross_cost > 100_000               # `filter` replaces both of SQL's `WHERE` & `HAVING`
derive id = f"{title}_{country}"              # F-strings like python
derive country_code = s"LEFT(country, 2)"     # S-strings allow using SQL as an escape hatch
sort [sum_gross_cost, -country]               # `-country` means descending order
take 1..20                                    # Range expressions (also valid here as `take 20`)
```

For more on the language, more examples & comparisons with SQL, visit
[prql-lang.org][prql website]. To experiment with PRQL in the browser, check out
[PRQL Playground][prql playground].

## Current Status - January 2023

PRQL is being actively developed by a growing community. It's ready to use by
the intrepid, either as part of one of our supported extensions, or within your
own tools, using one of our supported language bindings.

PRQL continues to evolve toward the
[0.4 Milestone.](https://github.com/PRQL/prql/milestone/4) The
[CHANGELOG.md](https://github.com/PRQL/prql/blob/main/CHANGELOG.md) gives more
information.

PRQL still has some minor bugs and some missing features, and probably is only
ready to rolled out to non-technical teams for fairly simple queries. We're
exploring where to focus further development; we welcome use-cases. Here's our
current [Roadmap](https://prql-lang.org/roadmap/) and our longer-term
[set of Milestones.](https://github.com/PRQL/prql/milestones)

## Get involved

To stay in touch with PRQL:

- Follow us on [Twitter](https://twitter.com/prql_lang)
- Join us on [Discord](https://discord.gg/eQcfaCmsNc)
- Star this repo
- [Contribute][contributing] — join us in building PRQL, through writing code
  [(send us your use-cases!)](https://github.com/PRQL/prql/discussions), or
  inspiring others to use it.
- See the [development](DEVELOPMENT.md) documentation for PRQL. It's easy to get
  started — the project can be built in a couple of commands, and we're a really
  friendly community!

## Explore

- [PRQL Playground][prql playground] — experiment with PRQL in the browser.
- [PRQL Book][prql book] — the language documentation.
- [dbt-prql][dbt-prql] — write PRQL in dbt models.
- [Jupyter magic](https://pyprql.readthedocs.io/en/latest/magic_readme.html) —
  run PRQL in Jupyter, either against a DB, or a Pandas DataFrame / CSV /
  Parquet file through DuckDB.
- [pyprql Docs](https://pyprql.readthedocs.io) — the pyprql documentation, the
  python bindings to PRQL, including Jupyter magic.
- [PRQL VSCode Extension](https://marketplace.visualstudio.com/items?itemName=prql-lang.prql-vscode)
- [prql-js](https://www.npmjs.com/package/prql-js) — JavaScript bindings for
  PRQL.

### Contributors

Many thanks to those who've made our progress possible:

[![Contributors](https://contrib.rocks/image?repo=PRQL/prql)](https://github.com/PRQL/prql/graphs/contributors)

### Core developers

We have core developers who are responsible for reviewing code, making decisions
on the direction of the language, and project administration:

- [**@aljazerzen**](https://github.com/aljazerzen) — Aljaž Mur Eržen
- [**@max-sixty**](https://github.com/max-sixty) — Maximilian Roos
- [**@snth**](https://github.com/snth) — Tobias Brandt

We welcome others to join who have a track record of contributions.

[prql book]: https://prql-lang.org/book
[prql website]: https://prql-lang.org
[contributing]: ./CONTRIBUTING.md
[prql playground]: https://prql-lang.org/playground
[dbt-prql]: https://github.com/prql/dbt-prql
