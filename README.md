# PRQL

<!-- User badges on first line (language docs & chat) -->
[![Language Docs](https://img.shields.io/badge/DOCS-LANGUAGE-blue?style=for-the-badge)](https://lang.prql.builders)
[![Discord](https://img.shields.io/discord/936728116712316989?label=discord%20chat&style=for-the-badge)](https://discord.gg/eQcfaCmsNc)
[![VSCode](https://img.shields.io/visual-studio-marketplace/v/prql.prql?label=vscode&style=for-the-badge)](https://marketplace.visualstudio.com/items?itemName=prql.prql)
<!-- Dev badges on first line (language docs & chat) -->
[![Rust API Docs](https://img.shields.io/badge/DOCS-RUST-brightgreen?style=for-the-badge&logo=rust)](https://docs.rs/prql/)
[![GitHub CI Status](https://img.shields.io/github/workflow/status/prql/prql/tests?logo=github&style=for-the-badge)](https://github.com/prql/prql/actions?query=workflow:tests)
[![GitHub contributors](https://img.shields.io/github/contributors/prql/prql?style=for-the-badge)](https://github.com/prql/prql/graphs/contributors)
[![Stars](https://img.shields.io/github/stars/prql/prql?style=for-the-badge)](https://github.com/prql/prql/stargazers)

**P**ipelined **R**elational **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a modern language for transforming data — a simpler and more powerful
SQL. Like SQL, it's readable, explicit and declarative. Unlike SQL, it forms a
logical pipeline of transformations, and supports abstractions such as variables
and functions. It can be used with any database that uses SQL, since it
transpiles to SQL.

PRQL was discussed on [Hacker
News](https://news.ycombinator.com/item?id=30060784#30062329) and
[Lobsters](https://lobste.rs/s/oavgcx/prql_simpler_more_powerful_sql) earlier
this year when it was just a proposal.

## Overview

### A simple example

Here's a fairly simple SQL query:

```sql
SELECT TOP 20
    title,
    country,
    AVG(salary) AS average_salary,
    SUM(salary) AS sum_salary,
    AVG(salary + payroll_tax) AS average_gross_salary,
    SUM(salary + payroll_tax) AS sum_gross_salary,
    AVG(salary + payroll_tax + benefits_cost) AS average_gross_cost,
    SUM(salary + payroll_tax + benefits_cost) AS sum_gross_cost,
    COUNT(*) as count
FROM employees
WHERE salary + payroll_tax + benefits_cost > 0 AND country = 'USA'
GROUP BY title, country
ORDER BY sum_gross_cost
HAVING count > 200
```

Even this simple query demonstrates some of the problems with SQL's lack of
abstractions:

- Unnecessary repetition — the calculations for each measure are repeated,
  despite deriving from a previous measure. The repetition in the `WHERE`
  clause obfuscates the meaning of the expression.
- Functions have multiple operators — `HAVING` & `WHERE` are fundamentally
  similar operations applied at different stages of the pipeline but SQL's lack
  of pipeline-based precedence requires it to have two different operators.
- Operators have multiple functions — the `SELECT` operator both
  creates new aggregations, and selects which columns to include.
- Awkward syntax — when developing the query, commenting out the final line of
  the `SELECT` list causes a syntax error because of how commas are handled, and
  we need to repeat the columns in the `GROUP BY` clause in the `SELECT` list.

Here's the same query with PRQL:

```elm
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    ct: count,
]
sort sum_gross_cost
filter ct > 200
take 20
```

As well as using variables to reduce unnecessary repetition, the query is also
more readable — it flows from top to bottom, each line representing a
transformation of the previous line's result. For example, `TOP 20` / `take 20`
modify the final result in both queries — but only PRQL represents it as the
final transformation. And context is localized — the `aggregate` function
contains both the calculations and the columns to group by.

While PRQL is designed for reading & writing by people, it's also much simpler
for code to construct or edit PRQL queries. In SQL, adding a filter to a query
involves parsing the query to find and then modify the `WHERE` statement, or
wrapping the existing query in a CTE. In PRQL, adding a filter just involves
appending a `filter` transformation to the query.

### Try it out

Try it out at <https://lang.prql.builders/editor>, where PRQL is compiled into
SQL on every keystroke.

> The link will not open in a new tab by default.

[![Editor Link](https://github.com/prql/prql/blob/main/.github/live-editor-screenshot.png?raw=true)](https://lang.prql.builders/editor)

### A more complex example

Here's another SQL query, which calculates returns from prices on days with
valid prices.

```sql
WITH total_returns AS (
  SELECT
    date,
    sec_id,
    -- Can't use a `WHERE` clause, as it would affect the row that the `LAG` function referenced.
    IF(is_valid_price, price_adjusted / LAG(price_adjusted, 1) OVER
      (PARTITION BY sec_id ORDER BY date) - 1 + dividend_return, NULL) AS return_total,
    IF(is_valid_price, price_adjusted_usd / LAG(price_adjusted_usd, 1) OVER
      (PARTITION BY sec_id ORDER BY date) - 1 + dividend_return, NULL) AS return_usd,
    IF(is_valid_price, price_adjusted / LAG(price_adjusted, 1) OVER
      (PARTITION BY sec_id ORDER BY date) - 1 + dividend_return, NULL)
      - interest_rate / 252 AS return_excess,
    IF(is_valid_price, price_adjusted_usd / LAG(price_adjusted_usd, 1) OVER
      (PARTITION BY sec_id ORDER BY date) - 1 + dividend_return, NULL)
      - interest_rate / 252 AS return_usd_excess
  FROM prices
)
SELECT
  *,
  return_total - (interest_rate / 252) AS return_excess,
  EXP(SUM(LN(GREATEST(1 + return_total - (interest_rate / 252), 0.01))) OVER (ORDER BY date)) AS return_excess_index
FROM total_returns
JOIN interest_rates USING (date)
```

> This might seem like a convoluted example, but it's taken from a real query.
> Indeed, it's also simpler and smaller than the full logic — note that it
> starts from `price_adjusted`, whose logic had to be split into a previous
> query to avoid the SQL becoming even less readable.

Here's the same query with PRQL:

```elm
prql version:0.1 db:snowflake                         # PRQL version & database name.

func excess x = (x - interest_rate) / 252             # Functions are clean and simple.
func if_valid x = is_valid_price ? x : null
func lag_day x = (
  window                                              # Windows are pipelines too.
  by sec_id
  sort date
  lag 1
  x
)
func ret x = x / (x | lag_day) - 1 + dividend_return

from prices
join interest_rates [date]
derive [
  return_total:      prices_adj   | ret | if_valid    # `|` can be used rather than newlines.
  return_usd:        prices_usd   | ret | if_valid
  return_excess:     return_total | excess
  return_usd_excess: return_usd   | excess
  return_excess_index:  (                             # No need for a CTE.
    return_total + 1 | excess | greatest 0.01         # Complicated logic remains clear.
      | ln | (window | sort date | sum) | exp
  )
]
select [
  date,
  sec_id,
  return_total,
  return_usd,
  return_excess,
  return_usd_excess,
  return_excess_index,
]
```

Because we define the functions once rather than copying & pasting the code, we
get all the benefits of encapsulation and extensibility — we have reliable &
tested functions, whose purpose is explicit, which we can share across queries
and between colleagues.

We needed a CTE in the SQL query, because the lack of variables would have
required a nested window clause, which isn't allowed. With PRQL, our logic isn't
constrained by these arbitrary constraints — and is more compressed as a result.

## Current status

PRQL just hit 0.1! This means:

- It works™, for basic transformations such as `filter`, `select`, `aggregate`, `take`,
  `sort`, & `join`. Variables (`derive`), functions (`func`) and CTEs (`table`) work.
  - More advanced language features are forthcoming, like better inline pipelines, window
    clauses, and arrays.
- It's not friendly at the moment:
  - It runs from a CLI only, taking input from a file or stdin and writing to a
    file or stdout.
  - Error messages are bad.
  - For an interactive experience, combine with a tool like
    [Up](https://github.com/akavel/up).
- The documentation is lacking.
  - Our current top priority is to have some decent documentation
    [#233](https://github.com/prql/prql/issues/232).
- It doesn't support changing the dialect.
- It has bugs. Please report them!
- It has sharp corners. Please report grazes!
- We'll release backward-incompatible changes. The versioning system for the
  language is not yet implemented.

### Installation

PRQL can be installed with `cargo`, or built from source.

```sh
cargo install prql
```

### Usage

```sh
$ echo "from employees | filter has_dog" | prql compile

SELECT
  *
FROM
  employees
WHERE
  has_dog
```

See above for fuller examples of PRQL.

### Python implementation

There is a python implementation at
[qorrect/PyPrql](https://github.com/qorrect/PyPrql), which can be installed with
`pip install pyprql`. It has some great features, including a native interactive
console with auto-complete for column names.

## Principles

PRQL is intended to be a modern, simple, declarative language for transforming
data, with abstractions such as variables & functions. It's intended to replace
SQL, but doesn't have ambitions as a general-purpose programming language. While
it's at a pre-alpha stage, it has some immutable principles:

- *Pipelined* — PRQL is a linear pipeline of transformations — each line of the
  query is a transformation of the previous line's result. This makes it easy to
  read, and simple to write. This is also known as
  "[point-free](https://en.wikipedia.org/w/index.php?title=Point-free_programming)".
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

## Roadmap

I'm excited and inspired by the level of enthusiasm behind the project, both
from individual contributors and the broader community of users who are
unsatisfied with SQL. We currently have an initial working version for the
intrepid early user.

I'm hoping we can build a beautiful language, an app that's approachable &
powerful, and a vibrant community. Many projects have reached the current stage
and fallen, so this requires compounding on what we've done so far.

### Language design

Already since becoming public, the language has improved dramatically, thanks to
the feedback of dozens of contributors. The current state of the basics is now
stable and while we'll hit corner-cases, I expect we'll only make small changes
to the existing features — even as we continue adding features.

Feel free to post questions or continue discussions on [Language Design
Issues](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3Alanguage-design).

### Documentation

Currently the documentation exists in [tests](tests/test_transpile.rs),
[examples](https://github.com/prql/prql/tree/main/examples), [docs.rs](https://docs.rs/prql/latest/prql/) and some
[Notes](#notes) below.

If you're up for contributing and don't have a preference for writing code or
not, this is the area that would most benefit from your contribution. Issues are
tagged with
[documentation](https://github.com/prql/prql/labels/documentation).

### Friendliness

Currently the language is not friendly, as described in [Current
status](#current-status). We'd like to make error messages better, sand off
sharp corners, etc.

Both bug reports of unfriendliness, and code contributions to improve them are
welcome; there's a
[friendliness](https://github.com/prql/prql/issues?q=is%3Aissue+label%3Afriendlienss+is%3Aopen)
label.

### Fast feedback

As well as a command-line tool that transpiles queries, we'd like to make
developing in PRQL a wonderful experience, where it feels like it's on your
side:

- Syntax highlighting in editors.
- A live transpiler in a browser, including compiling to wasm
  [#175](https://github.com/prql/prql/pull/175).
- Initial type-inference, where it's possible without connecting to the DB, e.g.
  [#55](https://github.com/prql/prql/pull/55).
- (I'm sure there's more, ideas welcome)

### Database cohesion

One benefit of PRQL over SQL is that auto-complete, type-inference, and
error checking can be much more powerful.

This is harder to build, since it requires a connection to the database in order
to understand the schema of the table.

### Not in focus

We should focus on solving a distinct problem really well. PRQL's goal is to
make reading and writing analytical queries easier, and so for the moment that
means putting some things out of scope:

- Building infrastructure outside of queries, like lineage. dbt is excellent at
  that! ([#13](https://github.com/prql/prql/issues/13)).
- Writing DDL / index / schema manipulation / inserting data
  ([#16](https://github.com/prql/prql/issues/16)).
- Add typing into the syntax
  ([#15](https://github.com/prql/prql/issues/15)) (though type
  *inference* is a goal above, and this could be a useful extension at some
  point).

## Contributing

If you're interested in joining the community to build a better SQL, there are
lots of ways of contributing; big and small:

- Star this repo.
- Send the repo to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/prql/prql/issues/1) for
  updates.
- Join the [Discord](https://discord.gg/eQcfaCmsNc).
- Contribute towards the code. There are many ways of contributing, for any
  level of experience with rust. And if you have rust questions, there are lots of
  friendly people on the Discord who will patiently help you.
  - Find an issue labeled [help
    wanted](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)
    or [good first
    issue](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
    and try to fix it. Feel free to PR partial solutions, or ask any questions on
    the Issue or Discord.
  - Build the code, find examples that yield incorrect results, and post a bug
    report.
  - Start with something tiny! Write a test / write a docstring / make some rust
    nicer — it's a great way to get started in 30 minutes.

Any of these will inspire others to invest their time and energy into the
project; thank you in advance.

### Development environment

Setting up a local dev environment is simple, thanks to the rust ecosystem:

- Install [`rustup` & `cargo`](https://doc.rust-lang.org/cargo/getting-started/installation.html).
- That's it! Running `cargo test` should complete successfully.
- For more advanced development; e.g. adjusting `insta` outputs or compiling for
  web, run the commands in [Taskfile.yml](Taskfile.yml), either by copying &
  pasting or by installing [Task](https://taskfile.dev/#/installation) and
  running `task install-dev-tools`.
- For quick contributions, hit `.` in GitHub to launch a [github.dev
  instance](https://github.dev/prql/prql).
- Any problems: post an issue and we'll help.

### Contributors

Many thanks to those who've made our progress possible:

[![Contributors](https://contrib.rocks/image?repo=prql/prql)](https://github.com/prql/prql/graphs/contributors)

### Core developers

We have a few core developers who are responsible for reviewing code, making
decisions on the direction of the language, and project administration:

- [**@aljazerzen**](https://github.com/aljazerzen) — Aljaž Mur Eržen
- [**@max-sixty**](https://github.com/max-sixty) — Maximilian Roos
- [**@qorrect**](https://github.com/qorrect) — Charlie Sando

We welcome others to join who have a track record of contributions.

## Inspired by

- [dplyr](https://dplyr.tidyverse.org/) is a beautiful language for manipulating
  data, in R. It's very similar to PRQL. It only works on in-memory R data.
  - There's also [dbplyr](https://dbplyr.tidyverse.org/) which compiles a subset
    of dplyr to SQL, though requires an R runtime.
- [Kusto](https://docs.microsoft.com/azure/data-explorer/kusto/query/samples?pivots=azuredataexplorer)
  is also a beautiful pipelined language, very similar to PRQL. But it can only
  use Kusto-compatible DBs.
  - A Kusto-to-SQL transpiler would be a legitimate alternative to PRQL, though
     there would be some impedance mismatch in some areas. My central criticism
     of Kusto is that it gives up broad compatibility without getting that much
     in return.
- [Against SQL](https://www.scattered-thoughts.net/writing/against-sql/) gives a
  fairly complete description of SQL's weaknesses, both for analytical and
  transactional queries. [**@jamii**](https://github.com/jamii) consistently
  writes insightful pieces, and it's worth sponsoring him for his updates.
- Julia's [DataPipes.jl](https://gitlab.com/aplavin/DataPipes.jl) &
  [Chain.jl](https://github.com/jkrumbiegel/Chain.jl), which demonstrate how
  effective point-free pipelines can be, and how line-breaks can work as pipes.
- [OCaml](https://ocaml.org/), for its elegant and simple syntax.

## Similar projects

- [Ecto](https://hexdocs.pm/ecto/Ecto.html#module-query) is a sophisticated
  ORM library in Elixir which has pipelined queries as well as more traditional
  ORM features.
- [Morel](http://blog.hydromatic.net/2020/02/25/morel-a-functional-language-for-data.html)
  is a functional language for data, also with a pipeline concept. It doesn't
  compile to SQL but states that it can access external data.
- [Malloy](https://github.com/looker-open-source/malloy) from Looker &
  [**@lloydtabb**](https://github.com/lloydtabb) in a new language which
  combines a declarative syntax for querying with a modelling layer.
- [FunSQL.jl](https://github.com/MechanicalRabbit/FunSQL.jl) is a library in
  Julia which compiles a nice query syntax to SQL. It requires a Julia runtime.
- [LINQ](https://docs.microsoft.com/dotnet/csharp/linq/write-linq-queries),
  is a pipelined language for the `.NET` ecosystem which can (mostly) compile to
  SQL. It was one of the first languages to take this approach.
- [Sift](https://github.com/RCHowell/Sift) is an experimental language which
  heavily uses pipes and relational algebra.
- After writing this proposal (including the name!), I found
  [Preql](https://github.com/erezsh/Preql). Despite the similar name and
  compiling to SQL, it seems to focus more on making the language python-like,
  which is very different to this proposal.

> If any of these descriptions can be improved, please feel free to PR changes.
