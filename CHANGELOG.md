# PRQL Changelog

## 0.5.1 — [unreleased]

**Features**:

- Convert parser from PEST to Chumsky (@aljazerzen, #1818)
  - Error recovery in some cases and more readable errors in general.
  - String escapes (` \n \t `).
  - Raw strings that don't escape backslashes.
  - String interpolations can only contain identifiers and not any expression.
  - Operator associativity has been changed from right-to-left to left-to-right
    to be more similar to other conventional languages.
  - `and` now has a higher precedence than `or` (of same reason as the previous point).
  - Dates, times and timestamps have a stricter parsing rules.
  - Ranges are now parsed as normal binary operators, which sometimes requires adding parenthesis
    to existing expressions.
  - Jinja expressions have been removed.
  - `let`, `func`, `prql`, `switch` are now treated as keywords.

**Fixes**:

- Delegate dividing literal integers to the DB. Previously integer division was
  executed during PRQL compilation, which could be confusing given that behavior
  is different across DBs. Other arithmetic operations are still executed during
  compilation. (@max-sixty #1747)

**Documentation**:

- Operator precedence

**Web**:

**Integrations**:

**Internal changes**:

**New Contributors**:

## 0.5.0 — 2022-02-08

0.5.0 contains a few fixes, some improvements to bindings, lots of docs
improvements, and some work on forthcoming features. It contains one breaking
change in the compiler's `Options` interface.

This release has 74 commits from 12 contributors. Selected changes:

**Features**:

- Change public API to use target instead of dialect in preparation for feature
  work (@aljazerzen, #1684)

- `prqlc watch` command which watches filesystem for changes and compiles .prql
  files to .sql (@aljazerzen, #1708)

**Fixes**:

- Support double brackets in s-strings which aren't symmetric (@max-sixty,
  #1650)
- Support Postgres's Interval syntax (@max-sixty, #1649)
- Fixed tests for `prql-elixir` with MacOS (@kasvith, #1707)

**Documentation**:

- Add a documentation test for prql-compiler, update prql-compiler README, and
  include the README in the prql book section for Rust bindings. The code
  examples in the README are included and tested as doctests in the
  prql-compiler (@nkicg6, #1679)

**Internal changes**:

- Add tests for all PRQL website examples to prql-python to ensure compiled
  results match expected SQL (@nkicg6, #1719)

**New Contributors**:

- @ruslandoga, with #1628
- @RalfNorthman, with #1632
- @nicot, with #1662

## 0.4.2 — 2022-01-25

**Features**:

- New `from_text format-arg string-arg` function that supports JSON and CSV
  formats. _format-arg_ can be `format:csv` or `format:json`. _string-arg_ can
  be a string in any format. (@aljazerzen & @snth, #1514)

  ```prql
  from_text format:csv """
  a,b,c
  1,2,3
  4,5,6
  """

  from_text format:json '''
      [{"a": 1, "b": "x", "c": false }, {"a": 4, "b": "y", "c": null }]
  '''

  from_text format:json '''{
      "columns": ["a", "b", "c"],
      "data": [
          [1, "x", false],
          [4, "y", null]
      ]
  }'''
  ```

  For now, the argument is limited to string constants.

**Fixes**

- Export constructor for SQLCompileOptions (@bcho, #1621)
- Remove backticks in count_distinct (@aljazerzen, #1611)

**New Contributors**

- @1Kinoti, with #1596
- @veenaamb, with #1614

## 0.4.1 — 2022-01-18

0.4.1 comes a few days after 0.4.0, with a couple of features and the release of
`prqlc`, the CLI crate.

0.4.1 has 35 commits from 6 contributors.

**Features**:

- Inferred column names include the relation name (@aljazerzen, #1550):

  ```prql
  from albums
  select title # name used to be inferred as title only
  select albums.title # so using albums was not possible here
  ```

- Quoted identifiers such as `dir/*.parquet` are passed through to SQL.
  (@max-sixty, #1516).

- The CLI is installed with `cargo install prqlc`. The binary was renamed in
  0.4.0 but required an additional `--features` flag, which has been removed in
  favor of this new crate (@max-sixty & @aljazerzen, #1549).

**New Contributors**:

- @fool1280, with #1554
- @nkicg6, with #1567

## 0.4.0 — 2022-01-15

0.4.0 brings lots of new features including `switch`, `select ![]` and numbers
with underscores. We have initial (unpublished) bindings to Elixir. And there's
the usual improvements to fixes & documentation (only a minority are listed
below in this release).

0.4.0 also has some breaking changes: `table` is `let`, `dialect` is renamed to
`target`, and the compiler's API has changed. Full details below.

**Features**:

- Defining a temporary table is now expressed as `let` rather than `table`
  (@aljazerzen, #1315). See the
  [tables docs](https://prql-lang.org/book/queries/tables.html) for details.

- _Experimental:_ The
  [`switch`](https://prql-lang.org/book/language-features/switch.html) function
  sets a variable to a value based on one of several expressions (@aljazerzen,
  #1278).

  ```prql
  derive var = switch [
    score <= 10 -> "low",
    score <= 30 -> "medium",
    score <= 70 -> "high",
    true -> "very high",
  ]
  ```

  ...compiles to:

  ```sql
  SELECT
    *,
    CASE
      WHEN score <= 10 THEN 'low'
      WHEN score <= 30 THEN 'medium'
      WHEN score <= 70 THEN 'high'
      ELSE 'very high'
    END AS var
  FROM
    bar
  ```

  Check out the
  [`switch` docs](https://prql-lang.org/book/language-features/switch.html) for
  more details.

- _Experimental:_ Columns can be excluded by name with `select` (@aljazerzen,
  #1329)

  ```prql
  from albums
  select ![title, composer]
  ```

- _Experimental:_ `append` transform, equivalent to `UNION ALL` in SQL.
  (@aljazerzen, #894)

  ```prql
  from employees
  append managers
  ```

  Check out the
  [`append` docs](https://prql-lang.org/book/transforms/append.html) for more
  details.

- Numbers can contain underscores, which can make reading long numbers easier
  (@max-sixty, #1467):

  ```prql
  from numbers
  select [
      small = 1.000_000_1,
      big = 5_000_000,
  ]
  ```

- The SQL output contains a comment with the PRQL compiler version (@aljazerzen,
  #1322)
- `dialect` is renamed to `target`, and its values are prefixed with `sql.`
  (@max-sixty, #1388); for example:

  ```prql
  prql target:sql.bigquery  # previously was `dialect:bigquery`

  from employees
  ```

  This gives us the flexibility to target other languages than SQL in the long
  term.

- Tables definitions can contain a bare s-string (@max-sixty, #1422), which
  enables us to include a full CTE of SQL, for example:

  ```prql
  let grouping = s"""
    SELECT SUM(a)
    FROM tbl
    GROUP BY
      GROUPING SETS
      ((b, c, d), (d), (b, d))
  """
  ```

- Ranges supplied to `in` can be half-open (@aljazerzen, #1330).

- The crate's external API has changed to allow for compiling to intermediate
  representation. This also affects bindings. See
  [`prql_compiler` docs](https://docs.rs/prql-compiler/latest/prql_compiler/)
  for more details.

**Fixes**:

[This release, the changelog only contains a subset of fixes]

- Allow interpolations in table s-strings (@aljazerzen, #1337)

**Documentation**:

[This release, the changelog only contains a subset of documentation
improvements]

- Add docs on aliases in
  [Select](https://prql-lang.org/book/transforms/select.html)
- Add JS template literal and multiline example (@BCsabaEngine, #1432)
- JS template literal and multiline example (@BCsabaEngine, #1432)
- Improve prql-compiler docs & examples (@aljazerzen, #1515)
- Fix string highlighting in book (@max-sixty, #1264)

**Web**:

- The playground allows querying some sample data. As before, the result updates
  on every keystroke. (@aljazerzen, #1305)

**Integrations**:

[This release, the changelog only contains a subset of integration improvements]

- Added Elixir integration exposing PRQL functions as NIFs (#1500, @kasvith)
- Exposed Elixir flavor with exceptions (#1513, @kasvith)
- Rename `prql-compiler` binary to `prqlc` (@aljazerzen #1515)

**Internal changes**:

[This release, the changelog only contains a subset of internal changes]

- Add parsing for negative select (@max-sixty, #1317)
- Allow for additional builtin functions (@aljazerzen, #1325)
- Add an automated check for typos (@max-sixty, #1421)
- Add tasks for running playground & book (@max-sixty, #1265)
- Add tasks for running tests on every file change (@max-sixty, #1380)

**New contributors**:

- @EArazli, with #1359
- @boramalper, with #1362
- @allurefx, with #1377
- @bcho, with #1375
- @JettChenT, with #1385
- @BlurrechDev, with #1411
- @BCsabaEngine, with #1432
- @kasvith, with #1500

## 0.3.1 - 2022-12-03

0.3.1 brings a couple of small improvements and fixes.

**Features**:

- Support for using s-strings for `from` (#1197, @aljazerzen)

  ```prql
  from s"SELECT * FROM employees WHERE foo > 5"
  ```

- Helpful error message when referencing a table in an s-string (#1203,
  @aljazerzen)

**Fixes**:

- Multiple columns with same name created (#1211, @aljazerzen)
- Renaming via select breaks preceding sorting (#1204, @aljazerzen)
- Same column gets selected multiple times (#1186, @mklopets)

**Internal**:

- Update Github Actions and Workflows to current version numbers (and avoid
  using Node 12)

## 0.3.0 — 2022-11-29

🎉 0.3.0 is the biggest ever change in PRQL's compiler, rewriting much of the
internals: the compiler now has a semantic understanding of expressions,
including resolving names & building a DAG of column lineage 🎉.

While the immediate changes to the language are modest — some long-running bugs
are fixed — this unlocks the development of many of the project's long-term
priorities, such as type-checking & auto-complete. And it simplifies the
building of our next language features, such as match-case expressions, unions &
table expressions.

@aljazerzen has (mostly single-handedly) done this work over the past few
months. The project owes him immense appreciation.

**Breaking changes**:

We've had to make some modest breaking changes for 0.3:

- _Pipelines must start with `from`_. For example, a pipeline with only
  `derive foo = 5`, with no `from` transform, is no longer valid. Depending on
  demand for this feature, it would be possible to add this back.

- _Shared column names now require `==` in a join_. The existing approach is
  ambiguous to the compiler — `id` in the following example could be a boolean
  column.

  ```diff
  from employees
  -join positions [id]
  +join positions [==id]
  ```

- _Table references containing periods must be surrounded by backticks_. For
  example, when referencing a schema name:

  ```diff
  -from public.sometable
  +from `public.sometable`
  ```

**Features**:

- Change self equality op to `==` (#1176, @aljazerzen)
- Add logging (@aljazerzen)
- Add clickhouse dialect (#1090, @max-sixty)
- Allow namespaces & tables to contain `.` (#1079, @aljazerzen)

**Fixes**:

- Deduplicate column appearing in `SELECT` multiple times (#1186, @aljazerzen)
- Fix uppercase table names (#1184, @aljazerzen)
- Omit table name when only one ident in SELECT (#1094, @aljazerzen)

**Documentation**:

- Add chapter on semantics' internals (@aljazerzen, #1028)
- Add note about nesting variables in s-strings (@max-sixty, #1163)

**Internal changes**:

- Flatten group and window (#1120, @aljazerzen)
- Split ast into expr and stmt (@aljazerzen)
- Refactor associativity (#1156, @aljazerzen)
- Rename Ident constructor to `from_name` (#1084, @aljazerzen)
- Refactor rq folding (#1177, @aljazerzen)
- Add tests for reported bugs fixes in semantic (#1174, @aljazerzen)
- Bump duckdb from 0.5.0 to 0.6.0 (#1132)
- Bump once_cell from 1.15.0 to 1.16.0 (#1101)
- Bump pest from 2.4.0 to 2.5.0 (#1161)
- Bump pest_derive from 2.4.0 to 2.5.0 (#1179)
- Bump sqlparser from 0.25.0 to 0.27.0 (#1131)
- Bump trash from 2.1.5 to 3.0.0 (#1178)

## 0.2.11 — 2022-11-20

0.2.11 contains a few helpful fixes.

Work continues on our `semantic` refactor — look out for 0.3.0 soon! Many thanks
to @aljazerzen for his continued contributions to this.

Note: 0.2.10 was skipped due to this maintainer's inability to read his own docs
on bumping versions...

**Features**:

- Detect when compiler version is behind query version (@MarinPostma, #1058)
- Add `__version__` to prql-python package (@max-sixty, #1034)

**Fixes**:

- Fix nesting of expressions with equal binding strength and left associativity,
  such as `a - (b - c)` (@max-sixty, #1136)
- Retain floats without significant digits as floats (@max-sixty, #1141)

**Documentation**:

- Add documentation of `prqlr` bindings (@eitsupi, #1091)
- Add a 'Why PRQL' section to the website (@max-sixty, #1098)
- Add @snth to core-devs (@max-sixty, #1050)

**Internal changes**:

- Use workspace versioning (@max-sixty, #1065)

## 0.2.9 — 2022-10-14

0.2.9 is a small release containing a bug fix for empty strings.

**Fixes**:

- Fix parsing of empty strings (@aljazerzen, #1024)

## 0.2.8 — 2022-10-10

0.2.8 is another modest release with some fixes, doc improvements, bindings
improvements, and lots of internal changes. Note that one of the fixes causes
the behavior of `round` and `cast` to change slightly — though it's handled as a
fix rather than a breaking change in semantic versioning.

**Fixes**:

- Change order of the `round` & `cast` function parameters to have the column
  last; for example `round 2 foo_col` / `cast int foo`. This is consistent with
  other functions, and makes piping possible:

  ```prql
  derive [
    gross_salary = (salary + payroll_tax | as int),
    gross_salary_rounded = (gross_salary | round 0),
  ]
  ```

**Documentation**:

- Split `DEVELOPMENT.md` from `CONTRIBUTING.md` (@richb-hanover, #1010)
- Make s-strings more prominent in website intro (@max-sixty, #982)

**Web**:

- Add GitHub star count to website (@max-sixty, #990)

**Integrations**:

- Expose a shortened error message, in particular for the VSCode extension
  (@aljazerzen, #1005)

**Internal changes**:

- Specify 1.60.0 as minimum rust version (@max-sixty, #1011)
- Remove old `wee-alloc` code (@max-sixty, #1013)
- Upgrade clap to version 4 (@aj-bagwell, #1004)
- Improve book-building script in Taskfile (@max-sixty, #989)
- Publish website using an artifact rather than a long-lived branch (@max-sixty,
  #1009)

## 0.2.7 — 2022-09-17

0.2.7 is a fairly modest release, six weeks after 0.2.6. We have some more
significant features, including a `union` operator and an overhaul of our type
system, as open PRs which will follow in future releases.

We also have new features in the
[VSCode extension](https://github.com/PRQL/prql-code), courtesy of
@jiripospisil, including a live output panel.

**Fixes**:

- `range_of_ranges` checks the Range end is smaller than its start (@shuozeli,
  #946)

**Documentation**:

- Improve various docs (@max-sixty, #974, #971, #972, #970, #925)
- Add reference to EdgeDB's blog post in our FAQ (@max-sixty, #922)
- Fix typos (@kianmeng, #943)

**Integrations**:

- Add `prql-lib`, enabling language bindings with `go` (@sigxcpu76, #923)
- Fix line numbers in JS exceptions (@charlie-sanders, #929)

**Internal changes**:

- Lock the version of the rust-toolchain, with auto-updates (@max-sixty, #926,
  #927)

## 0.2.6 — 2022-08-05

**Fixes**:

- Adjust `fmt` to only escape names when needed (@aljazerzen, #907)
- Fix quoting on upper case `table` names (@max-sixty, #893)
- Fix scoping of identical column names from multiple tables (@max-sixty, #908)
- Fix parse error on newlines in a `table` (@sebastiantoh 🆕, #902)
- Fix quoting of upper case table names (@max-sixty, #893)

**Documentation**:

- Add docs on
  [Architecture](https://prql-lang.org/book/internals/compiler-architecture.html)
  (@aljazerzen, #904)
- Add Changelog (@max-sixty, #890 #891)

**Internal changes**:

- Start trial using Conventional Commits (@max-sixty, #889)
- Add crates.io release workflow, docs (@max-sixty, #887)

## 0.2.5 - 2022-07-29

0.2.5 is a very small release following 0.2.4 yesterday. It includes:

- Add the ability to represent single brackets in an s-string, with two brackets
  (#752, @max-sixty )
- Fix the "Copy to Clipboard" command in the Playground, for Firefox (#880,
  @mklopets )

## 0.2.4 - 2022-07-28

0.2.4 is a small release following 0.2.3 a few days ago. The 0.2.4 release
includes:

- Enrich our CLI, adding commands to get different stages of the compilation
  process (@aljazerzen , #863)
- Fix multiple `take n` statements in a query, leading to duplicate proxy
  columns in generated SQL (@charlie-sanders )
- Fix BigQuery quoting of identifiers in `SELECT` statements (@max-sixty )
- Some internal changes — reorganize top-level functions (@aljazerzen ), add a
  workflow to track our rust compilation time (@max-sixty ), simplify our simple
  prql-to-sql tests (@max-sixty )

Thanks to @ankane, `prql-compiler` is now available from homebrew core;
`brew install prql-compiler`[^2].

[^2]:
    we still need to update docs and add a release workflow for this:
    <https://github.com/PRQL/prql/issues/866>

## 0.2.3 - 2022-07-24

A couple of weeks since the 0.2.2 release: we've squashed a few bugs, added some
mid-sized features to the language, and made a bunch of internal improvements.

The 0.2.3 release includes:

- Allow for escaping otherwise-invalid identifiers (@aljazerzen & @max-sixty )
- Fix a bug around operator precedence (@max-sixty )
- Add a section the book on the language bindings (@charlie-sanders )
- Add tests for our `Display` representation while fixing some existing bugs.
  This is gradually becoming our code formatter (@arrizalamin )
- Add a "copy to clipboard" button in the Playground (@mklopets )
- Add lots of guidance to our `CONTRIBUTING.md` around our tests and process for
  merging (@max-sixty )
- Add a `prql!` macro for parsing a prql query at compile time (@aljazerzen )
- Add tests for `prql-js` (@charlie-sanders )
- Add a `from_json` method for transforming json to a PRQL string (@arrizalamin
  )
- Add a workflow to release `prql-java` to Maven (@doki23 )
- Enable running all tests from a PR by adding a `pr-run-all-tests` label
  (@max-sixty )
- Have `cargo-release` to bump all crate & npm versions (@max-sixty )
- Update `prql-js` to use the bundler build of `prql-js` (@mklopets )

As well as those contribution changes, thanks to those who've reported issues,
such as @mklopets @huw @mm444 @ajfriend.

From here, we're planning to continue squashing bugs (albeit more minor than
those in this release), adding some features like `union`, while working on
bigger issues such as type-inference.

We're also going to document and modularize the compiler further. It's important
that we give more people an opportunity to contribute to the guts of PRQL,
especially given the number and enthusiasm of contributions to project in
general — and it's not that easy to do so at the moment. While this is ongoing
if anyone has something they'd like to work on in the more difficult parts of
the compiler, let us know on GitHub or Discord, and we'd be happy to work
together on it.

Thank you!

## 0.2.2 - 2022-07-10

We're a couple of weeks since our 0.2.0 release. Thanks for the surge in
interest and contributions! 0.2.2[^1] has some fixes & some internal
improvements:

- We now test against SQLite & DuckDB on every commit, to ensure we're producing
  correct SQL. (@aljazerzen )
- We have the beginning of Java bindings! (@doki23 )
- Idents surrounded by backticks are passed through to SQL (@max-sixty )
- More examples on homepage; e.g. `join` & `window`, lots of small docs
  improvements
- Automated releases to homebrew (@roG0d )
- [prql-js](https://github.com/PRQL/prql/tree/main/prql-js) is now a single
  package for node, browsers & webpack (@charlie-sanders )
- Parsing has some fixes, including `>=` and leading underscores in idents
  (@mklopets )
- Ranges receive correct syntax highlighting (@max-sixty )

Thanks to Aljaž Mur Eržen @aljazerzen , George Roldugin @roldugin , Jasper
McCulloch @Jaspooky , Jie Han @doki23 , Marko Klopets @mklopets , Maximilian
Roos @max-sixty , Rodrigo Garcia @roG0d , Ryan Russell @ryanrussell , Steven
Maude @StevenMaude , Charlie Sanders @charlie-sanders .

We're planning to continue collecting bugs & feature requests from users, as
well as working on some of the bigger features, like type-inference.

For those interesting in joining, we also have a new
[Contributing page](https://github.com/PRQL/prql/blob/main/CONTRIBUTING.md).

[^1]: Think of 0.2.1 like C+ :)

## 0.2.0 - 2022-06-27

🎉 🎉 **After several months of building, PRQL is ready to use!** 🎉 🎉

---

How we got here:

At the end of January, we published a proposal of a better language for data
transformation: PRQL. The reception was better than I could have hoped for — we
were no. 2 on HackerNews for a day, and gained 2.5K GitHub stars over the next
few days.

But man cannot live on GitHub Stars alone — we had to do the work to build it.
So over the next several months, during many evenings & weekends, a growing
group of us gradually built the compiler, evolved the language, and wrote some
integrations.

We want to double-down on the community and its roots in open source — it's
incredible that a few of us from all over the globe have collaborated on a
project without ever having met. We decided early-on that PRQL would always be
open-source and would never have a commercial product (despite lots of outside
interest to fund a seed round!). Because languages are so deep in the stack, and
the data stack has so many players, the best chance of building a great language
is to build an open language.

---

We still have a long way to go. While PRQL is usable, it has lots of missing
features, and an incredible amount of unfulfilled potential, including a
language server, cohesion with databases, and type inference. Over the coming
weeks, we'd like to grow the number of intrepid users experimenting PRQL in
their projects, prioritize features that will unblock them, and then start
fulfilling PRQL's potential by working through our
[roadmap](https://prql-lang.org/roadmap/).

The best way to experience PRQL is to try it. Check out our
[website](https://prql-lang.org) and the
[Playground](https://prql-lang.org/playground). Start using PRQL for your own
projects in [dbt](https://github.com/prql/dbt-prql),
[Jupyter notebooks](https://pyprql.readthedocs.io/en/latest/magic_readme.html)
and Prefect workflows.

Keep in touch with PRQL by following the project on
[Twitter](https://twitter.com/prql_lang), joining us on
[Discord](https://discord.gg/eQcfaCmsNc), starring the
[repo](https://github.com/PRQL/prql).

[Contribute](https://github.com/PRQL/prql/blob/main/CONTRIBUTING.md) to the
project — we're a really friendly community, whether you're a recent SQL user or
an advanced rust programmer. We need bug reports, documentation tweaks & feature
requests — just as much as we need compiler improvements written in rust.

---

I especially want to give [Aljaž Mur Eržen](https://github.com/aljazerzen)
(@aljazerzen) the credit he deserves, who has contributed the majority of the
difficult work of building out the compiler. Much credit also goes to
[Charlie Sanders](https://github.com/charlie-sanders) (@charlie-sanders), one of
PRQL's earliest supporters and the author of PyPrql, and
[Ryan Patterson-Cross](https://github.com/orgs/prql/people/rbpatt2019)
(@rbpatt2019), who built the Jupyter integration among other Python
contributions.

Other contributors who deserve a special mention include: @roG0d, @snth,
@kwigley

---

Thank you, and we look forward to your feedback!
