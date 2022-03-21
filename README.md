# PRQL

[![GitHub CI Status](https://img.shields.io/github/workflow/status/max-sixty/prql/tests?logo=github&style=for-the-badge)](https://github.com/max-sixty/prql/actions?query=workflow:tests)
[![Discord](https://img.shields.io/discord/936728116712316989?style=for-the-badge)](https://discord.gg/eQcfaCmsNc)
[![Stars](https://img.shields.io/github/stars/max-sixty/prql?style=for-the-badge)](https://github.com/max-sixty/prql/stargazers)

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
  - Our current top priority is to have some decent documentation.
- It doesn't support changing the dialect.
- It has bugs. Please report them!
- It has sharp corners. Please report grazes!
- We'll release backward-incompatible changes. The versioning system for the
  language is not yet built.

### Installation

PRQL can be installed with `cargo`, or built from source.

```sh
cargo install prql
```

### Usage

```sh
echo "from employees | filter has_dog" | prql compile
SELECT
  *
FROM
  employees
WHERE
  has_dog
```

More details in `prql compile --help`. See below for better examples of PRQL.

### Python implementation

There is a python implementation at
[qorrect/PyPrql](https://github.com/qorrect/PyPrql), which can be installed with
`pip install pyprql`. It has some great features, including a native interactive
console with auto-complete for column names.

## An example

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
    ct: count *,
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
adding a `filter` transformation to the final line.

## An example using Functions

Here's another SQL query, which calculates returns from prices on days with
valid prices.

```sql
SELECT
  date,
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
```

> This might seem like a convoluted example, but it's taken from a real query.
> Indeed, it's also simpler and smaller than the full logic — note that it
> starts from `price_adjusted`, whose logic had to be split into a previous
> query to avoid the SQL becoming even less readable.

Here's the same query with PRQL:

```elm
prql version:0.1 db:snowflake                         # Version number & database name.

func lag_day x = (
  window x
  by sec_id
  sort date
  lag 1
)
func ret x = x / (x | lag_day) - 1 + dividend_return
func excess x = (x - interest_rate) / 252
func if_valid x = is_valid_price ? x : null

from prices
derive [
  return_total:      prices_adj   | ret | if_valid  # `|` can be used rather than newlines.
  return_usd:        prices_usd   | ret | if_valid
  return_excess:     return_total | excess
  return_usd_excess: return_usd   | excess
]
select [
  date,
  sec_id,
  return_total,
  return_usd,
  return_excess,
  return_usd_excess,
]
```

Because we define the functions once rather than copying & pasting the code, we
get all the benefits of encapsulation and extensibility — we can have reliable &
tested functions, whose purpose is explicit, which we can share across queries
and colleagues.

## Principles

PRQL is intended to be a modern, simple, declarative language for transforming
data, with abstractions such as variables & functions. It's intended to replace
SQL, but doesn't have ambitions as a general-purpose programming language. While
it's at a pre-alpha stage, it has some immutable principles:

- *Pipelined* — PRQL is a linear pipeline of transformations — each line of the
  query is a transformation of the previous line's result. This makes it easy to
  read, and simple to write. This is also known as "[point-free
  style](https://en.wikipedia.org/w/index.php?title=Point-free_programming)".
- *Simple* — PRQL serves both sophisticated engineers and analysts without
  coding experience. By providing simple, clean abstractions, the
  language can be both powerful and easy to use.
- *Compatible* — PRQL transpiles to SQL, so it can be used with any database
  that uses SQL, and with any existing tools or programming languages that
  manage SQL. PRQL should allow for a gradual onramp — it should be practical to
  mix SQL into a PRQL query where PRQL doesn't yet have an implementation. Where
  possible PRQL can unify syntax across databases.
- *Analytical* — PRQL's focus is analytical queries; we de-emphasize other SQL
  features such as inserting data or transactions.
- *Extensible* — PRQL can be extended through its abstractions, and can evolve
  without breaking backward-compatibility, because its queries can specify their
  PRQL version.

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
Issues](https://github.com/max-sixty/prql/issues?q=is%3Aissue+is%3Aopen+label%3Alanguage-design).

### Documentation

Currently the documentation exists in [tests](tests/test_transpile.rs),
[examples](https://github.com/max-sixty/prql/tree/main/examples), and some
[Notes](#notes) below.

If you're up for contributing and don't have a preference for writing code or
not, this is the area that would most benefit from your contribution. Issues are
tagged with
[documentation](https://github.com/max-sixty/prql/labels/documentation).

### Friendliness

Currently the language is not friendly, as described in [Current
status](#current-status). We'd like to make error messages better, sand off
sharp corners, etc.

Both bug reports of unfriendliness, and code contributions to improve them are
welcome; there's a
[friendliness](https://github.com/max-sixty/prql/issues?q=is%3Aissue+label%3Afriendlienss+is%3Aopen)
label.

### Fast feedback

As well as a command-line tool that transpiles queries, we'd like to make
developing in PRQL a wonderful experience, where it feels like it's on your
side:

- Syntax highlighting in editors.
- A live transpiler in a browser, including compiling to wasm
  [#175](https://github.com/max-sixty/prql/pull/175).
- Initial type-inference, where it's possible without connecting to the DB, e.g.
  [#55](https://github.com/max-sixty/prql/pull/55).
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
  that! ([#13](https://github.com/max-sixty/prql/issues/13)).
- Writing DDL / index / schema manipulation / inserting data
  ([#16](https://github.com/max-sixty/prql/issues/16)).
- Add typing into the syntax
  ([#15](https://github.com/max-sixty/prql/issues/15)) (though type
  *inference* is a goal above, and this could be a useful extension at some
  point).

## Interested in joining?

If you're interested in joining the community to build a better SQL, there are
lots of ways of contributing; big and small:

- Star this repo.
- Send the repo to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/max-sixty/prql/issues/1) for
  updates.
- Join the [Discord](https://discord.gg/eQcfaCmsNc).
- Contribute towards the code. There are many ways of contributing, for any
  level of experience with rust. And if you have rust questions, there are lots of
  people on the Discord who will patiently help you.
  - Find an issue labeled [help
    wanted](https://github.com/max-sixty/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)
    or [good first
    issue](https://github.com/max-sixty/prql/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
    and try to fix it. Feel free to PR partial solutions, or ask any questions on
    the Issue or Discord.
  - Build the code, find examples that yield incorrect results, and post a bug
    report.
  - Start with something tiny! Write a test / write a docstring / make some rust
    nicer — it's a great way to get started in 30 minutes.

Any of these will inspire others to spend more time developing this; thank you
in advance.

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
- [Against SQL](https://www.scattered-thoughts.net/writing/against-sql/) gives
  a fairly complete description of SQL's weaknesses, both for analytical and
  transactional queries. @jamii consistently writes insightful pieces, and it's
  worth sponsoring him for his updates.
- Julia's [DataPipes.jl](https://gitlab.com/aplavin/DataPipes.jl) &
  [Chain.jl](https://github.com/jkrumbiegel/Chain.jl), which demonstrate how
  effective point-free pipelines can be, and how line-breaks can work as pipes.
- [OCaml](https://ocaml.org/)'s elegant and simple syntax.

## Similar projects

- [Ecto](https://hexdocs.pm/ecto/Ecto.html#module-query) is a sophisticated
  ORM library in Elixir which has pipelined queries as well as more traditional
  ORM features.
- [Morel](http://blog.hydromatic.net/2020/02/25/morel-a-functional-language-for-data.html)
  is a functional language for data, also with a pipeline concept. It doesn't
  compile to SQL but states that it can access external data.
- [Malloy](https://github.com/looker-open-source/malloy) from Looker &
  @lloydtabb in a new language which combines a declarative syntax for querying
  with a modelling layer.
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

## Notes

### Joins

- Joins are implemented as `join side:{join_type} {table} {[conditions]}`. For example:

  ```elm
  from employees
  join side:left positions [id=employee_id]
  ```

  ...is equivalent to...

  ```sql
  SELECT * FROM employees LEFT JOIN positions ON id = employee_id
  ```

- Possibly we could shorten `[id=id]` to `id`, and use SQL's `USING`, but it may
  be ambiguous with using `id` as a boolean column.
- Previously the syntax was `{join_type} {table} {[conditions]}`. For example:

  ```elm
  left_join positions [id=employee_id]
  ```

  ...but it was not expandable.

### Functions

- Functions can take two disjoint types of arguments:
  1. Positional arguments, which are required.
  2. Named arguments, which are optional and have a default value.
- So a function like:

  ```elm
  func lag col sort_col by_col=id = (
    window col
    by by_col
    sort sort_col
    lag 1
  )
  ```

  ...is called `lag`, takes three arguments `col`, `sort_col` & `by_col`, of
  which the first two much be supplied, the third can optionally be supplied
  with `by_col:sec_id`.

### Assignments

- To create a column, we use `derive {column_name}: {calculation}` in a
  pipeline. `derive` can also take a list of pairs.
  Technically this "upserts" the column — it'll either create or overwrite a
  column, depending on whether it already exists.
  - Potentially it would be possible to discriminate between those, but during
    the most recent discussion we didn't find a suitable approach.
- Previously the syntax was just `{column_name} = {calculation}`, but it breaks
  the pattern of every line starting with a keyword.
- We could discard the `:` to just have `derive {column_name} ({calculation})`, which
  would be more consistent with the other functions, but I think less readable
  (and definitely less familiar).

### S-Strings

An s-string inserts SQL directly. It's similar in form to a python f-string, but
the result is SQL, rather than a string literal; i.e.:

```elm
func sum col = s"SUM({col})"
sum salary
```

transpiles to:

```sql
SUM(salary)
```

...whereas if it were a python f-string, it would make `"sum(salary)"`, with the
quotes.

### Lists

- Most keywords that take a single argument can also take a list, so these are equivalent:

  ```diff
   from employees
  -select salary
  +select [salary]
  ```

- More examples in [**list-equivalence.md**](examples/list-equivalence.md).

### Pipelines

- A line-break generally creates a pipelined transformation. For example:

  ```elm
  from tbl
  select [
    col1,
    col2,
  ]
  filter col1 = col2
  ```

  ...is equivalent to:

  ```elm
  from tbl | select [col1, col2] | filter col1 = col2
  ```

- A line-break doesn't created a pipeline in a few cases:
  - Within a list (e.g. the `select` example above).
  - When the following line is a new statement, by starting with a keyword such
    as `func`.

### CTEs

- See [CTE Example](examples/cte-1.md).
- This is no longer point-free, but that's a feature rather than a requirement.
  The alternative is subqueries, which are fine at small scale, but become
  difficult to digest as complexity increases.

## Thinking about

- How functions represent the previous result — the previous result is passed as
  the final argument of a function; i.e. `aggregate` would be like this; where
  `X` is taken from the line above:

  ```elm
  aggregate by=[] calcs X
  ```

- Literal strings & f-strings: <https://github.com/max-sixty/prql/issues/109>

- Arrays — PRQL is in part inspired by
  [DataPipes.jl](https://gitlab.com/aplavin/DataPipes.jl), which demonstrates
  how effective point-free pipelines can be
  ([Chain.jl](https://github.com/jkrumbiegel/Chain.jl) is similar). One benefit
  of this is how well it deals with arbitrarily nested pipelines — which are
  difficult to read in SQL and even in `jq`. Could we do something similar for
  nested data in PRQL?
  - Here's a snippet from `DataPipes.jl` — and we could avoid the macros / `do` / `end`):

    ```julia
    @p begin
      text
      strip
      split(__, "\n")
      map() do __
          collect
          map() do __
            __ == chars[begin] ? 1 : 0
          end
      end
      hcat(__...)'
    end
    ```

- Partials — how functional do we want to make the lang? e.g. should we have
  partial functions? e.g. [now based on an old version of `window`] potentially
  we don't need the `col` in `lag` here?

  ```elm
  func lag col = window col by:sec_id sort:date lag:1
  ```

- Boolean logic — how should we represent boolean logic like `or`? With some
  `or` function that takes `*args` (which we don't currently have a design for)?
  Or implement dyadic operators; either `or` or `||`? (Same for `not`)

- `from` — do we need `from`? A previous version of this proposal didn't require
  this — just start with the table name. But some initial feedback was that
  removing `from` made it less clear.

- Readme syntax — we can't get syntax highlighting in GitHub's markdown — is
  there a solution to this aside from submitting a parser to GitHub /
  screenshots / creating a website?
  - Currently we use `elm` as it coincidentally provides the best syntax
    highlight (open to suggestions for others!).
