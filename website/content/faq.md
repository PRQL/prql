---
title: "FAQ"
---

## Cool story Hansel, but what can I actually do with PRQL now?

We're still early, and the opportunities for using PRQL are focused on two
integrations:

- **[dbt-prql](https://github.com/prql/dbt-prql)** allows writing PRQL in
  [dbt](https://www.getdbt.com/) models. It very simple to use — install
  `dbt-prql` with pip, and then any text between a `{% prql %}` & `{% endprql %}` tag is compiled from PRQL.
- **[Jupyter](https://pyprql.readthedocs.io/en/latest/magic_readme.html)**
  allows writing PRQL in a Jupyter notebook or IPython repl, with a `%%prql`
  magic. As well as connecting to existing DBs, our integration with DuckDB
  enables querying pandas dataframes, CSVs & Parquet files, and writing the
  output to a dataframe.

Beyond these two integrations, it's very easy to add PRQL to your own apps with
our [bindings](../content/_index.md#Bindings), for Rust, Python & JS.

## Something here reminds me of another project, did you take the idea from them?

Yes, probably. We're standing on the shoulders of giants:

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

And there are many projects similar to PRQL:

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
- After writing the original proposal (including the name!), we found
  [Preql](https://github.com/erezsh/Preql). Despite the similar name and
  compiling to SQL, it seems to focus more on making the language python-like,
  which is very different to this proposal.

> If any of these descriptions can be improved, please feel free to PR changes.

## How is PRQL different from all the projects that SQL has defeated?

Many languages have attempted to replace SQL, and yet SQL has massively _grown_
in usage and importance in the past decade. There are lots
[of](https://twitter.com/seldo/status/1513599841355526145)
[reasonable](https://benn.substack.com/p/has-sql-gone-too-far?s=r#footnote-anchor-2)
[critiques](https://erikbern.com/2018/08/30/i-dont-want-to-learn-your-garbage-query-language.html)
on these attempts. So a reasonable question is "Why are y'all building something that
many others have failed at?". Some thoughts:

- PRQL is open. It's not designed for a specific database. PRQL will always be
  fully open-source. There will never be a commercial product. We'll never have
  to balance profitability against compatibility, or try and expand up the stack
  to justify a valuation. Whether someone is building a new tool or writing a
  simple query — PRQL can be _more_ compatible across DBs than SQL.
- PRQL is analytical. The biggest growth in SQL usage has been from querying large
  amounts of data, often from analytical DBs that are specifically designed for
  this — with columnar storage and wide denormalized tables. SQL carries a lot
  of baggage unrelated to this, and focusing on the analytical use-case lets us
  make a better language.
- PRQL is simple. There's often a tradeoff between power and accessibility
  — rust is powerful vs. Excel is accessible — but there are also instances
  where we can expand the frontier. PRQL's orthogonality is an example of
  synthesizing this tradeoff — have a single `filter` rather than `WHERE` & `HAVING`
  & `QUALIFY` brings both more power _and_ more accessibility.

In the same way that "SQL was invented in the 1970s and therefore must be bad"
is questionable logic, "`n` languages have tried and failed so therefore SQL
cannot be improved." suffers a similar fallacy. SQL isn't bad because it's old.
It's bad because — in some cases — it's bad.

## Which databases does PRQL work with?

PRQL compiles to SQL, so it's compatible with any database that accepts SQL.

A query's dialect can be explicitly specified, allowing for dialect-specific SQL
to be generated. See the [Dialect
docs](https://prql-lang.org/book/queries/dialect_and_version.html) for more
info; note that there is currently very limited implementation of this, and most
dialects' implementation are identical to a generic implementation.

## What's going on with this `aggregate` function? What's wrong with `SELECT` & `GROUP BY`?

SQL uses `SELECT` for all of these:

- Selecting or computing columns, without changing the shape of the data:

  ```sql
  SELECT x * 2 FROM y as two_x
  ```

- Reducing a column into a single row, with a reduction function:

  ```sql
  SELECT SUM(x) FROM y
  ```

- Reducing a column into groups, with a reduction function and a `GROUP BY` function:

  ```sql
  SELECT SUM(x) FROM y GROUP BY z
  ```

These are not orthogonal — `SELECT` does lots of different things depending on
the context. It's difficult for both people and machines to evaluate the shape
of the output. It's easy to mix meanings and raise an error (e.g. `SELECT x, MIN(y) FROM z`).

PRQL clearly delineates two operations with two transforms:

- `select` — picking & calculating columns.
  These calculations always produce exactly one output row for every input row.

  ```prql
  from employees
  select name = f"{first_name} {last_name}"
  ```

- `aggregate` — reducing multiple rows to a single row, with a reduction
  function like `sum` or `min`.

  ```prql
  from employees
  aggregate [total_salary = sum salary]
  ```

`aggregate` can then be used in a `group` transform, where it has exactly the
same semantics on the group as it would on a whole table — another example of
PRQL's orthogonality.

```prql
from employees
group department (
  aggregate [total_salary = sum salary]
)
```

While you should be skeptical of new claims from new entrants, but [Hadley
Wickham](https://twitter.com/hadleywickham), the developer of
[Tidyverse](https://www.tidyverse.org/)
[commented](https://news.ycombinator.com/item?id=30067406) in a discussion on
PRQL:

> FWIW the separate `group_by()` is one of my greatest design regrets with dplyr
> — I wish I had made `by` a parameter of `summarise()`, `mutate()`, `filter()`
> etc.

For more detail, check out the docs in the [PRQL Book](https://prql-lang.org/book).
