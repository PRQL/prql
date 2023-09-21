# PRQL

<!-- User badges on first line (language docs & chat) -->

[![Website](https://img.shields.io/badge/INTRO-WEB-blue?style=for-the-badge)](https://prql-lang.org)
[![Playground](https://img.shields.io/badge/INTRO-PLAYGROUND-blue?style=for-the-badge)](https://prql-lang.org/playground)
[![Language Docs](https://img.shields.io/badge/DOCS-BOOK-blue?style=for-the-badge)](https://prql-lang.org/book)
[![Discord](https://img.shields.io/discord/936728116712316989?label=discord%20chat&style=for-the-badge)](https://discord.gg/eQcfaCmsNc)

<!-- Doesn't seem to be working; add back if it is -->
<!-- [![Twitter](https://img.shields.io/twitter/follow/prql_lang?color=%231DA1F2&style=for-the-badge)](https://twitter.com/prql_lang) -->
<!-- Dev badges on second line -->

[![GitHub CI Status](https://img.shields.io/github/actions/workflow/status/prql/prql/tests.yaml?event=push&branch=main&logo=github&style=for-the-badge)](https://github.com/PRQL/prql/actions?query=branch%3Amain+workflow%3Atests)
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
from tracks
filter artist == "Bob Marley"                 # Each line transforms the previous result
aggregate {                                   # `aggregate` reduces each column to a value
  plays    = sum plays,
  longest  = max length,
  shortest = min length,                      # Trailing commas are allowed
}
```

Here's a fuller example of the language;

```elm
from employees
filter start_date > @2021-01-01               # Clear date syntax
derive {                                      # `derive` adds columns / variables
  gross_salary = salary + (tax ?? 0),         # Terse coalesce
  gross_cost = gross_salary + benefits_cost,  # Variables can use other variables
}
filter gross_cost > 0
group {title, country} (                      # `group` runs a pipeline over each group
  aggregate {                                 # `aggregate` reduces each group to a value
    average gross_salary,
    sum_gross_cost = sum gross_cost,          # `=` sets a column name
  }
)
filter sum_gross_cost > 100_000               # `filter` replaces both of SQL's `WHERE` & `HAVING`
derive id = f"{title}_{country}"              # F-strings like Python
derive country_code = s"LEFT(country, 2)"     # S-strings allow using SQL as an escape hatch
sort {sum_gross_cost, -country}               # `-country` means descending order
take 1..20                                    # Range expressions (also valid here as `take 20`)
```

For more on the language, more examples & comparisons with SQL, visit
[prql-lang.org][prql website]. To experiment with PRQL in the browser, check out
[PRQL Playground][prql playground].

## Current Status - October 2023

PRQL is being actively developed by a growing community. It's ready to use by
the intrepid, either with our supported integrations, or within your own tools,
using one of our supported language bindings.

PRQL still has some bugs and some missing features, and is probably only ready
to be rolled out to non-technical teams for fairly simple queries.

We released [0.9.0](https://github.com/PRQL/prql/releases/tag/0.9.0), in July,
our biggest ever release. Here's our current
[Roadmap](https://prql-lang.org/roadmap/).

<!-- TODO: add back when we get them
and our
[Milestones](https://github.com/PRQL/prql/milestones). -->

Our immediate focus for the code is on:

- Ensuring our supported features feel extremely robust; resolving any
  [priority bugs](https://github.com/PRQL/prql/issues?q=is%3Aissue+is%3Aopen+label%3Abug+label%3Apriority).
  As more folks have started using PRQL, we've had more bug reports — good news,
  but also gives us more to work on.
- Filling remaining feature gaps, so that PRQL is possible to use for almost all
  standard SQL queries; for example
  [date to string functions](https://github.com/PRQL/prql/issues/366).
- Expanding our set of supported features — we've recently added experimental
  support for modules / multi-file projects, and for auto-formatting.

We're also spending time thinking about:

- Making it really easy to start using PRQL. We're doing that by building
  integrations with tools that folks already use; for example our VS Code
  extension & Jupyter integration. If there are tools you're familiar with that
  you think would be open to integrating with PRQL, please let us know in an
  issue.
- Whether all our initial decisions were correct — for example
  [how we handle window functions outside of a `window` transform](https://github.com/PRQL/prql/issues/2723).
- Making it easier to contribute to the compiler. We have a wide group of
  contributors to the project, but contributions to the compiler itself are
  quite concentrated. We're keen to expand this;
  [#1840](https://github.com/PRQL/prql/issues/1840) for feedback, some
  suggestions on starter issues are below.

## Get involved

To stay in touch with PRQL:

- Follow us on [Twitter](https://twitter.com/prql_lang)
- Join us on [Discord](https://discord.gg/eQcfaCmsNc)
- Star this repo
- [Contribute][contributing] — join us in building PRQL, through writing code
  [(send us your use-cases!)](https://github.com/PRQL/prql/discussions), or
  inspiring others to use it.
- See the [development][development] documentation for PRQL. It's easy to get
  started — the project can be built in a couple of commands, and we're a really
  friendly community!
- For those who might be interested in contributing to the code now, check out
  issues with the
  [good first issue](https://github.com/PRQL/prql/labels/good%20first%20issue)
  label. Always feel free to ask questions or open a draft PR.

## Explore

- [PRQL Playground][prql playground] — experiment with PRQL in the browser.
- [PRQL Book][prql book] — the language documentation.
- [dbt-prql][dbt-prql] — write PRQL in dbt models.
- [Jupyter magic](https://pyprql.readthedocs.io/en/latest/magic_readme.html) —
  run PRQL in Jupyter, either against a DB, or a Pandas DataFrame / CSV /
  Parquet file through DuckDB.
- [pyprql Docs](https://pyprql.readthedocs.io) — the pyprql documentation, the
  Python bindings to PRQL, including Jupyter magic.
- [PRQL VS Code extension](https://marketplace.visualstudio.com/items?itemName=prql-lang.prql-vscode)
- [prql-js](https://www.npmjs.com/package/prql-js) — JavaScript bindings for
  PRQL.

## Repo organization

This repo is composed of:

- **[prql-compiler](./crates/prql-compiler/)** — the compiler, written in rust,
  whose main role is to compile PRQL into SQL. It also includes
  [prqlc](./crates/prqlc/), the CLI.
- **[web](./web/)** — our web content: the [Book][prql book],
  [Website][prql website], and [Playground][prql playground].
- **[bindings](./bindings/)** — bindings from various languages to
  `prql-compiler`.

It also contains our testing / CI infrastructure and development tools. Check
out our [development docs][development] for more details.

## Contributors

Many thanks to those who've made our progress possible:

[![Contributors](https://contrib.rocks/image?repo=PRQL/prql)](https://github.com/PRQL/prql/graphs/contributors)

[prql book]: https://prql-lang.org/book
[prql website]: https://prql-lang.org
[contributing]: https://prql-lang.org/book/project/contributing/
[development]: https://prql-lang.org/book/project/contributing/development.html
[prql playground]: https://prql-lang.org/playground
[dbt-prql]: https://github.com/prql/dbt-prql
