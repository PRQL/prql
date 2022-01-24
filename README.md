# PRQL

**P**ipelined **R**elational **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a modern language for transforming data — a simpler and more powerful
SQL. Like SQL, it's readable, explicit and declarative. Unlike SQL, it forms a
logical pipeline of transformations, and supports abstractions such as variables
and functions. It can be used with any database that uses SQL, since it
transpiles to SQL.

PRQL was discussed on Hacker News
[here](https://news.ycombinator.com/item?id=30060784#30062329).

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
let gross_salary = salary + payroll_tax          # This _adds_ a column / variable.
let gross_cost   = gross_salary + benefits_cost  # Variables can use other variables.
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are the calcs to run on the groups.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum     gross_cost,
    count,
]
sort sum_gross_cost                              # Uses the auto-generated column name.
filter count > 200
take 20
```

As well as using variables to reduce unnecessary repetition, the query is also
more readable — it flows from top to bottom, each line representing a
transformation of the previous line's result. For example, `TOP 20` / `take 20`
modify the final result in both queries — but only PRQL represents it as the
final transformation. And context is localized — the `aggregate` function
contains both the calculations and the columns to group by.

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
prql version:0.0.1 db:snowflake                       # Version number & database name.

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
let return_total      = prices_adj   | ret | if_valid    # `|` can be used rather than newlines.
let return_usd        = prices_usd   | ret | if_valid
let return_excess     = return_total | excess
let return_usd_excess = return_usd   | excess
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
  that uses SQL. Where possible PRQL can unify syntax across databases. PRQL
  should allow for a gradual onramp — it should be practical to mix SQL into a
  PRQL query where PRQL doesn't yet have an implementation.
- *Analytical* — PRQL's focus is analytical queries; we de-emphasize other SQL
  features such as inserting data or transactions.
- *Extensible* — PRQL can be extended through its abstractions, and can evolve
  without breaking backward-compatibility, because its queries can specify their
  PRQL version.

## Interested in seeing this happen?

If you're interested in the ideas here and would like to see them explored:

- Star this repo.
- Open an issue:
  - An analytical SQL query that's awkward and we could use as a case for
    translating to PRQL. If you'd like to add a suggestion of the equivalent
    PRQL, that's very welcome too.
  - An area that isn't sufficiently discussed in the existing proposal.
  - An analytical SQL query that you think would be _more_ difficult to express
    in PRQL.
- Send the repo to a couple of people whose opinion you respect.
- Subscribe to [Issue #1](https://github.com/max-sixty/prql/issues/1) for
  updates.
- (If you'd be up for helping on the complier, let me know, but it's at a very
  early stage.)

Any of these will inspire me to spend more time developing this; thank you in
advance.

## Inspired by

- [dplyr](https://dplyr.tidyverse.org/) is a beautiful language for manipulating
  data, in R. It's very similar to PRQL. It only works on in-memory R data.
  - There's also [dbplyr](https://dbplyr.tidyverse.org/) which compiles a subset
    of dplyr to SQL. It requires an R runtime.
- [Kusto](https://docs.microsoft.com/en-us/azure/data-explorer/kusto/query/samples?pivots=azuredataexplorer)
  is also a beautiful pipelined language, very similar to PRQL. But it can only
  use Kusto-compatible DBs.
  - A Kusto-to-SQL transpiler would be a legitimate alternative to PRQL, though
     there would be some impediment mismatch in some areas. My central criticism
     of Kusto is that it gives up broad compatibility without getting that much
     in return.
- [Against SQL](https://www.scattered-thoughts.net/writing/against-sql/) gives
  a fairly complete description of SQL's weaknesses, both for analytical and
  transactional queries. @jamii consistently writes insightful pieces, and it's
  worth sponsoring him for his updates.
- Julia's [DataPipes.jl](https://gitlab.com/aplavin/DataPipes.jl) &
  [Chain.jl](https://github.com/jkrumbiegel/Chain.jl), which demonstrate how
  effective point-free pipelines can be, and how line-breaks can work as pipes.
- [Ocaml](https://ocaml.org/)'s elegant and simple syntax.

## Similar projects

- [Malloy](https://github.com/looker-open-source/malloy), from @lloydtabb looks
  very interesting, and has the team to make it successful. I'll spend some more
  time checking it out.
- [FunSQL.jl](https://github.com/MechanicalRabbit/FunSQL.jl) is a library in
  Julia which compiles a nice query syntax to SQL. It requires a Julia runtime.
- After writing this proposal (including the name!), I found
  [Preql](https://github.com/erezsh/Preql). Despite the similar name and
  compiling to SQL, it seems to focus more on making the language python-like,
  which is very different to this proposal.

## TODOs

- Write a basic parser
  - Currently writing it using `nom`.
- Write a basic complier
  - This should be fairly easy since it's just generating SQL.
- Demonstrate some more complicated examples — e.g. most of the examples in
  <https://github.com/dbt-labs/dbt-utils> could all be covered much better by
  this.

## Notes

### Joins

- Joins are implemented as `{join_type} {table} {[conditions]}`. For example:

  ```elm
  from employees
  left_join positions [id=employee_id]
  ```

  ...is equivalent to...

  ```sql
  SELECT * FROM employees LEFT JOIN positions ON id = employee_id
  ```

- Possibly we could shorten `[id=id]` to `id`, and use SQL's `USING`, but it may
  be ambiguous with using `id` as a boolean column.

### Functions

- Functions can take two disjoint types of arguments:
  1. Positional arguments. Callers must pass these.
  2. Named arguments, which can optionally have a default value.
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

- To create a column, we use `let {column_name} = {calculation}` in a pipeline.
  Technically this "upserts" the column — it'll either create or overwrite a
  column, depending on whether it already exists.
- Previously the syntax was just `{column_name} = {calculation}`, but it breaks
  the pattern of every line starting with a keyword.
- We could discard the `=` to just have `let {column_name} {calculation}`, which
  would be more consistent with the other functions, but I think less readable
  (and definitely less familiar).
- I'm still open to iterating here.

### Lists

- Currently lists require brackets; there's no implicit list like:

  ```elm
  from employees
  select salary  # fails, would require `select [salary]`
  ```

- For some functions where we're only expecting a single arg, like `select`,
  we could accept a single arg not as a list?

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

## Thinking about

- The previous result is passed as the final argument of a function; i.e.
  `aggregate` would be like this; where `X` is taken from the line above:

  ```elm
  aggregate by=[] calcs X
  ```

- CTE syntax — something like `table =`?
- Raw syntax — I think we should have backticks represent raw SQL; i.e. `UPPER`
  could be defined as:

  ```elm
  func upper col = `UPPER(`col`)`
  # or with f-string-like syntax
  func upper col = `UPPER({col})`
  # or with " rather than `
  func upper col = "UPPER({col})"
  ```

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

- In advance of a full parser & compiler, could we use something like
  [Codex](https://beta.openai.com/examples/default-sql-translate) to generate
  the transformations, and let us explore the space? We can provide our owen
  [examples](https://openai.com/blog/customized-gpt3/), by using
  [fine-tuning](https://beta.openai.com/docs/guides/fine-tuning/advanced-usage).
  Changing examples is easier than changing compilers!
