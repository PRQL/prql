---
title: "FAQ"
---

Here are some of the most common questions we hear. Have something else you'd
like to ask? Pop by our [Discord](https://discord.com/invite/eQcfaCmsNc) and ask
away!

{{< faq "Cool story Hansel, but what can I actually do with PRQL _now_?" >}}

PRQL is ready to use by the intrepid, either with our supported integrations, or
within your own tools, using one of our supported language bindings. The easiest
way is with our integrations:

- **Prototype your PRQL queries** in the
  [Playground](https://prql-lang.org/playground/) or the
  [VS Code extension](https://marketplace.visualstudio.com/items?itemName=PRQL-lang.prql-vscode)
  and copy/paste the resulting SQL into your database. It's not the perfect
  workflow, but it's easy to get started.
- **[Jupyter](https://pyprql.readthedocs.io/en/latest/magic_readme.html)**
  allows writing PRQL in a Jupyter notebook or IPython repl, with a `%%prql`
  magic. As well as connecting to existing DBs, our integration with DuckDB
  enables querying pandas dataframes, CSVs & Parquet files, and writing the
  output to a dataframe.
- **[DuckDB extension](https://github.com/ywelsch/duckdb-prql)** — is a DuckDB
  extension which allows querying a DuckDB instance with PRQL.

It's also possible to query PRQL from your code with our [bindings](/#bindings)
for R, Rust, Python & JS. For an example of using PRQL with DuckDB, check out
[Querying with PRQL](https://eitsupi.github.io/querying-with-prql/).

{{</ faq >}}

{{< faq "Something here reminds me of another project, did you take the idea from them?" >}}

Yes, probably. We're standing on the shoulders of giants:

- [dplyr](https://dplyr.tidyverse.org/) is a beautiful language for manipulating
  data, in R. It's very similar to PRQL. It only works on in-memory R data.
  - There's also [dbplyr](https://dbplyr.tidyverse.org/) which compiles a subset
    of dplyr to SQL, though requires an R runtime.
- [Kusto](https://docs.microsoft.com/azure/data-explorer/kusto/query/samples?pivots=azuredataexplorer)
  is also a beautiful pipelined language, similar to PRQL. But it can only use
  Kusto-compatible DBs.
  <!-- We can add more articles by linking from works in the "There are other similar piecs out there" sentence -->
- [Against SQL](https://www.scattered-thoughts.net/writing/against-sql/) gives a
  fairly complete description of SQL's weaknesses, both for analytical and
  transactional queries. [**@jamii**](https://github.com/jamii) consistently
  writes insightful pieces, and it's worth sponsoring him for his updates. There
  are
  [other](https://buttondown.email/jaffray/archive/sql-scoping-is-surprisingly-subtle-and-semantic/)
  similar pieces out there.
- Julia's [DataPipes.jl](https://gitlab.com/aplavin/DataPipes.jl) &
  [Chain.jl](https://github.com/jkrumbiegel/Chain.jl) demonstrate how effective
  point-free pipelines can be, and how line breaks can work as pipes.
- [OCaml](https://ocaml.org/)'s elegant and simple syntax serves as inspiration.

And there are many projects similar to PRQL:

- [Ecto](https://hexdocs.pm/ecto/Ecto.html#module-query) is a sophisticated ORM
  library in Elixir which has pipelined queries as well as more traditional ORM
  features.
- [Morel](https://www.thestrangeloop.com/2021/morel-a-functional-query-language.html)
  is a functional language for data, also with a pipeline concept. It doesn't
  compile to SQL but states that it can access external data.
- [Malloy](https://github.com/looker-open-source/malloy) from Looker &
  [**@lloydtabb**](https://github.com/lloydtabb) in a new language which
  combines a declarative syntax for querying with a modelling layer.
- [EdgeDB](https://www.edgedb.com/) is an alternative to SQL focused on
  traditional transactional workloads (as opposed to PRQL's focus on analytical
  workloads). Their post
  [We can do better than SQL](https://www.edgedb.com/blog/we-can-do-better-than-sql)
  contains many of the criticisms of SQL that inspired PRQL.
- [FunSQL.jl](https://github.com/MechanicalRabbit/FunSQL.jl) is a library in
  Julia which compiles a nice query syntax to SQL. It requires a Julia runtime.
- [LINQ](https://docs.microsoft.com/dotnet/csharp/linq/write-linq-queries), is a
  pipelined language for the `.NET` ecosystem which can (mostly) compile to SQL.
  It was one of the first languages to take this approach.
- [Sift](https://github.com/RCHowell/Sift) is an experimental language which
  heavily uses pipes and relational algebra.

> If any of these descriptions can be improved, please feel free to PR changes.

{{</ faq >}}

{{< faq "How is PRQL different from all the projects that SQL has defeated?" >}}

Many languages have attempted to replace SQL, and yet SQL has massively _grown_
in usage and importance in the past decade. There are lots
[of](https://twitter.com/seldo/status/1513599841355526145)
[reasonable](https://benn.substack.com/p/has-sql-gone-too-far?s=r#footnote-anchor-2)
[critiques](https://erikbern.com/2018/08/30/i-dont-want-to-learn-your-garbage-query-language.html)
on these attempts. So a reasonable question is "Why are y'all building something
that many others have failed at?". Some thoughts:

- PRQL is open. It's not designed for a specific database. PRQL will always be
  fully open-source. There will never be a commercial product. We'll never have
  to balance profitability against compatibility, or try and expand up the stack
  to justify a valuation. Whether someone is building a new tool or writing a
  simple query — PRQL can be _more_ compatible across DBs than SQL.
- PRQL is analytical. The biggest growth in SQL usage over the past decade has
  been from querying large amounts of data, often from analytical DBs that are
  specifically designed for this — with columnar storage and wide denormalized
  tables. SQL carries a lot of unrelated baggage, and focusing on the analytical
  use-case lets us make a better language.
- PRQL is simple. There's often a tradeoff between power and accessibility
  — e.g. rust is powerful vs. Excel is accessible — but there are also instances
  where we can expand the frontier. PRQL's orthogonality is an example of
  synthesizing this tradeoff — have a single `filter` rather than `WHERE` &
  `HAVING` & `QUALIFY` brings both more power _and_ more accessibility.

In the same way that "SQL was invented in the 1970s and therefore must be bad"
is questionable logic, "`n` languages have tried and failed so therefore SQL
cannot be improved." suffers a similar fallacy. SQL isn't bad because it's old.
It's bad because — in some cases — it's bad.

{{</ faq >}}

{{< faq "Which databases does PRQL work with?" >}}

PRQL compiles to SQL, so it's compatible with any database that accepts SQL.

A query's dialect can be explicitly specified, allowing for dialect-specific SQL
to be generated. See the
[Dialect docs](https://prql-lang.org/book/project/target.html) for more info;
note that there is currently very limited implementation of this, and most
dialects' implementation are identical to a generic implementation.

{{</ faq >}}

{{< faq "What's this `aggregate` function?" >}}

**...and why not just use `SELECT` & `GROUP BY`?**

SQL uses `SELECT` for all of these:

- Selecting or computing columns, without changing the shape of the data:

  ```sql
  SELECT x * 2 FROM y as two_x
  ```

- Reducing a column into a single row, with a reduction function:

  ```sql
  SELECT SUM(x) FROM y
  ```

- Reducing a column into groups, with a reduction function and a `GROUP BY`
  function:

  ```sql
  SELECT SUM(x) FROM y GROUP BY z
  ```

These are not orthogonal — `SELECT` does lots of different things depending on
the context. It's difficult for both people and machines to evaluate the shape
of the output. It's easy to mix meanings and raise an error (e.g.
`SELECT x, MIN(y) FROM z`).

PRQL clearly delineates two operations with two transforms:

- `select` — picking & calculating columns. These calculations always produce
  exactly one output row for every input row.

  ```prql
  from db.employees
  select name = f"{first_name} {last_name}"
  ```

- `aggregate` — reducing multiple rows to a single row, with a reduction
  function like `sum` or `min`.

  ```prql
  from db.employees
  aggregate [total_salary = sum salary]
  ```

`aggregate` can then be used in a `group` transform, where it has exactly the
same semantics on the group as it would on a whole table — another example of
PRQL's orthogonality.

```prql
from db.employees
group department (
  aggregate [total_salary = sum salary]
)
```

While you should be skeptical of new claims from new entrants
[Hadley Wickham](https://twitter.com/hadleywickham), the developer of
[Tidyverse](https://www.tidyverse.org/)
[commented](https://news.ycombinator.com/item?id=30067406) in a discussion on
PRQL:

> FWIW the separate `group_by()` is one of my greatest design regrets with dplyr
> — I wish I had made `by` a parameter of `summarise()`, `mutate()`, `filter()`
> etc.

For more detail, check out the docs in the
[PRQL Book](https://prql-lang.org/book/reference/stdlib/transforms/aggregate.html).

{{</ faq >}}

{{< faq "Can PRQL write to databases?" >}}

PRQL is focused on analytical queries, so we don't currently support writing or
modifying data in databases. However, PRQL queries can be used to generate SQL
statements that write to databases. For example, surround the SQL output of a
PRQL query in `CREATE OR REPLACE TABLE foo AS (...)`.

{{</ faq >}}

{{< faq "Is it 'PRQL' or 'prql' or 'Prql'?" >}}

It's `PRQL`, since it's a backronym! We name the repo and some libraries `prql`
because of a strong convention around lowercase, but everywhere else we use
`PRQL`.

{{</ faq >}}

{{< faq "Where can I find the logos?" >}}

See our [press materials](https://github.com/PRQL/prql-brand).

{{</ faq >}}
