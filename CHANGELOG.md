# PRQL Changelog

## [unreleased]

**Language**:

- _Breaking_: Exclude `group`'s `by` columns from the partition. See #3490
- _Breaking_: `round` is now in the `math` module and needs to be called via
  `math.round`. See #3928
- _Breaking_: `lower` and `upper` are now in the `text` module and need to be
  called via `text.lower` and `text.upper`. See #3913

**Features**:

- The `std.in` function now supports a list of values (@PrettyWood, #3883)
- Most standard mathematical functions are now supported: `abs`, `floor`,
  `ceil`, `pi`, `exp`, `ln`, `log10`, `log`, `sqrt`, `degrees`, `radians`,
  `cos`, `acos`, `sin`, `asin`, `tan`, `atan`, `pow` and `round`.\
  Those functions are in the `math` module (@PrettyWood, #3909, #3916 & 3928)
- Most standard string functions are now supported: `ltrim`, `rtrim`, `trim`,
  `length`, `extract`, `replace`. Utility functions `starts_with`, `contains`
  and `ends_with` are also available.\
  Those functions are in the `text` module (@PrettyWood, #3913, #3973)
- Formatting a date to a text is now available for Clickhouse, DuckDB, MySQL,
  MSSQL and Postgres. A new `date` module has been added with the `to_text`
  function (@PrettyWood, #3951, #3954 & #3955)

**Fixes**:

- Fix an issue with arithmetic precedence (@max-sixty, #3846)
- `+` and `-` can be used after a cast (@PrettyWood, #3923)

**Documentation**:

**Web**:

**Integrations**:

**Internal changes**:

- Bump `prql-compiler`'s MSRV to 1.70.0 (@eitsupi, #3876)

**New Contributors**:

- @PrettyWood, with #3883

## 0.10.1 — 2023-11-14

0.10.1 is a small release containing some internal fixes of the compiler.

This release has 36 commits from 7 contributors. Selected changes:

**Features**:

- The `std.sql.read_csv` function and the `std.sql.read_parquet` function
  supports the `sql.glaredb` target. (@eitsupi, #3749)

**Fixes**:

- Fix the bug of compiling to `DISTINCT ON` when `take 1` is used with
  `group by` for the targets `sql.clickhouse`, `sql.duckdb` and `sql.postgres`.
  (@aljazerzen, #3792)

**Integrations**:

- Enable integration tests for GlareDB. (@eitsupi, #3749)
- [trapd00r/LS_COLORS](https://github.com/trapd00r/LS_COLORS), a collection of
  LS_COLORS definitions colorizes `.prql` files. (@vanillajonathan)
- [vivid](https://github.com/sharkdp/vivid), a themeable LS_COLORS generator
  colorizes `.prql` files. (@vanillajonathan)
- [colorls](https://github.com/athityakumar/colorls), displays `.prql` files
  with a database icon. (@vanillajonathan)
- [Emoji File Icons](https://marketplace.visualstudio.com/items?itemName=mightbesimon.emoji-icons),
  a VS Code extension displays `.prql` files with a database emoji icon.
  (@vanillajonathan)
- [eza](https://eza.rocks/), a modern ls replacement colorizes `.prql` files.
  (@vanillajonathan)
- [lsd](https://github.com/lsd-rs/lsd), next gen ls command displays `.prql`
  files with a database icon. (@vanillajonathan)

## 0.10.0 — 2023-10-26

0.10.0 contains lots of small improvements, including support for new types of
literal notation, support for `read_*` functions in more dialects, playground
improvements, and a better Lezer grammar (which we're planning on using for a
Jupyter extension).

This release has 155 commits from 9 contributors. Selected changes:

**Language**:

- _Breaking:_ Case syntax now uses brackets `[]` rather than braces `{}`. To
  convert previous PRQL queries to this new syntax simply change `case { ... }`
  to `case [ ... ]`. (@AaronMoat, #3517)

**Features**:

- _Breaking_: The `std.sql.read_csv` function is now compiled to `read_csv` by
  default. Please set the target `sql.duckdb` to use the DuckDB's
  `read_csv_auto` function as previously. (@eitsupi, #3599)
- _Breaking_: The `std.every` function is renamed to `std.all` (@aljazerzen,
  #3703)
- The `std.sql.read_csv` function and the `std.sql.read_parquet` function
  supports the `sql.clickhouse` target. (@eitsupi, #1533)
- Add `std.prql_version` function to return PRQL version (@hulxv, #3533)
- A new type `anytype` is added. (@aljazerzen, #3703)
- Add support for hex escape sequences in strings. Example `"Hello \x51"`.
  (@vanillajonathan, #3568)
- Add support for long Unicode escape sequences. Example `"Hello \u{01F422}"`.
  (@vanillajonathan, #3569)
- Add support for binary numerical notation. Example
  `filter status == 0b1111000011110000`. (@vanillajonathan, #3661)
- Add support for hexadecimal numerical notation. Example
  `filter status == 0xff`. (@vanillajonathan, #3654)
- Add support for octal numerical notation. Example `filter status == 0o777`.
  (@vanillajonathan, #3672)
- New compile target `sql.glaredb` for [GlareDB](https://docs.glaredb.com/) and
  integration tests for it (However, there is a bug in the test and it is
  currently not running). (@universalmind303, @scsmithr, @eitsupi, #3669)

**Web**:

- Allow cmd-/ (Mac) or ctrl-/ (Windows) to toggle comments in the playground
  editor (@AaronMoat, #3522)

- Limit maximum height of the playground editor's error panel to avoid taking
  over whole screen (@AaronMoat, #3524)

- The playground now uses [Vite](https://vitejs.dev/) (@vanillajonathan).

**Integrations**:

- Add a CLI command `prqlc collect` to collect a project's modules into a single
  file (@aljazerzen, #3739)
- Add a CLI command `prqlc debug expand-pl` to parse & and expand into PL
  without resolving (@aljazerzen, #3739)
- Bump `prqlc`'s MSRV to 1.70.0 (@eitsupi, #3521)
- [Pygments](https://pygments.org/), a syntax highlighting library now has
  syntax highlighting for PRQL. (@vanillajonathan, #3564)
- [chroma](https://github.com/alecthomas/chroma), a syntax highlighting library
  written in Go and used by the static website generator
  [Hugo](https://gohugo.io/). (@vanillajonathan, #3597)
- [scc](https://github.com/boyter/scc), a source lines of code counter now has
  support for `.prql` files. (@vanillajonathan)
- [gcloc](https://github.com/JoaoDanielRufino/gcloc) a source lines of code
  counter now has support for `.prql` files. (@vanillajonathan)
- [cloc](https://github.com/AlDanial/cloc) a source lines of code counter now
  has support for `.prql` files. (@AlDanial)
- [gocloc](https://github.com/hhatto/gocloc) a source lines of code counter now
  has support for `.prql` files. (@vanillajonathan)
- [The Quarto VS Code extension](https://marketplace.visualstudio.com/items?itemName=quarto.quarto)
  supports editing PRQL code blocks
  ([`prqlr`](https://prql-lang.org/book/project/bindings/r.html) is required to
  render Quarto Markdown with PRQL code blocks). (@jjallaire)

**Internal**:

- Rename some of the internal crates, and refactored their paths in the repo.
  (@aljazerzen, #3683).
- Add a `justfile` for developers who prefer that above our `Taskfile.yml`
  (@aljazerzen, #3681)

**New Contributors**:

- @hulxv, with #3533
- @AaronMoat, with #3522
- @jangorecki, with #3634

## 0.9.5 — 2023-09-16

0.9.5 adds a line-wrapping character, fixes a few bugs, and improves our CI. The
release has 77 commits from 8 contributors. Selected changes are below.

Look out for some conference talks coming up over the next few weeks, including
[QCon SF on Oct 2](https://qconsf.com/presentation/oct2023/prql-simple-powerful-pipelined-sql-replacement)
and
[date2day on Oct 12](https://www.data2day.de/veranstaltung-21353-0-prql-a-modern-language-for-data-transformation.html).

**Language**:

- A new line-wrapping character, for lines that are long and we want to break up
  into multiple physical lines. This is slightly different from from many
  languages — it's on the subsequent line:

  ```prql no-eval
  from artists
  select is_europe =
  \ country == "DE"
  \ || country == "FR"
  \ || country == "ES"
  ```

  This allows for easily commenting out physical lines while maintaining a
  correct logical line; for example:

  ```diff
  from artists
  select is_europe =
  \ country == "DE"
  \ || country == "FR"
  \ || country == "FR"
  -\ || country == "ES"
  +#\ || country == "ES"
  ```

  (@max-sixty, #3408)

**Fixes**:

- Fix stack overflow on very long queries in Windows debug builds (@max-sixty,
  #2908)

- Fix panic when unresolved lineage appears in group or window (@davidot, #3266)

- Fix a corner-case in handling precedence, and remove unneeded parentheses in
  some outputs (@max-sixty, #3472)

**Web**:

- Compiler panics are now printed to the console (@max-sixty, #3446)

**Integrations**:

- [Ace](https://ace.c9.io/), the JavaScript code editor now has syntax
  highlighting for PRQL. (@vanillajonathan, #3493)

**Internal changes**:

- Simplify & speed up lexer (@max-sixty, #3426, #3418)

**New Contributors**:

- @davidot, with #3450

## 0.9.4 — 2023-08-24

0.9.4 is a small release with some improvements and bug fixes in the compiler
and `prqlc`. And, the documentation and CI are continually being improved.

This release has 110 commits from 9 contributors. Selected changes:

**Features**:

- Strings can be delimited with any odd number of quote characters. The logic
  for lexing quotes is now simpler and slightly faster. Escapes in
  single-quote-delimited strings escape single-quotes rather than double-quotes.
  (@max-sixty, #3274)

**Fixes**:

- S-strings within double braces now parse correctly (@max-sixty, #3265)

**Documentation**:

- New docs for strings (@max-sixty, #3281)

**Web**:

- Improve syntax highlighting for numbers in the book & website (@max-sixty,
  #3261)
- Add ClickHouse integration to docs (@max-sixty, #3251)

**Integrations**:

- `prqlc` no longer displays a prompt when piping a query into its stdin
  (@max-sixty, #3248).
- Add a minimal example for use `prql-lib` with Zig (@vanillajonathan, #3372)

**Internal changes**:

- Overhaul our CI to run a cohesive set of tests depending on the specific
  changes in the PR, and elide all others. This cuts CI latency to less than
  three minutes for most changes, and enables GitHub's auto-merge to wait for
  all relevant tests. It also reduces the CI time on merging to main, by moving
  some tests to only run on specific path changes or on our nightly run.

  We now have one label we can add to PRs to run more tests — `pr-nightly`.
  (@max-sixty, #3317 & others).

- Auto-merge PRs for backports or pre-commit updates (@max-sixty, #3246)
- Add a workflow to create an issue when the scheduled nightly workflow fails
  (@max-sixty, #3304)

**New Contributors**:

- @FinnRG, with #3292
- @sitiom, with #3353

## 0.9.3 — 2023-08-02

0.9.3 is a small release, with mostly documentation, internal, and CI changes.

This release has 85 commits from 10 contributors.

We'd like to welcome @not-my-profile as someone who has helped with lots of
internal refactoring in the past couple of weeks.

**New Contributors**:

- @vthriller, with #3171
- @postmeback, with #3216

## 0.9.2 — 2023-07-25

0.9.2 is a hotfix release to fix an issue in the 0.9.0 & 0.9.1 release
pipelines.

## 0.9.1 — 2023-07-25

0.9.1 is a hotfix release to fix an issue in the 0.9.0 release pipeline.

## 0.9.0 — 2023-07-24

0.9.0 is probably PRQL's biggest ever release. We have dialect-specific
standard-libraries, a regex operator, an initial implementation of multiple-file
projects & modules, lots of bug fixes, and many many internal changes.

We've made a few backward incompatible syntax changes. Most queries will work
with a simple find/replace; see below for details.

The release has 421 commits from 12 contributors.

A small selection of the changes:

**Language**:

- The major breaking change is a new syntax for lists, which have been renamed
  to _tuples_, and are now represented with braces `{}` rather than brackets
  `[]`.

  To convert previous PRQL queries to this new syntax simply change `[ ... ]` to
  `{ ... }`.

  We made the syntax change to incorporate arrays. Almost every major language
  uses `[]` for arrays. We are adopting that convention — arrays use `[]`,
  tuples will use `{}`. (Though we recognize that `{}` for tuples is also rare
  (Hi, Erlang!), but didn't want to further load parentheses with meaning.)

  Arrays are conceptually similar to columns — their elements have a single
  type. Array syntax can't contain assignments.

  As part of this, we've also formalized tuples as containing both individual
  items (`select {foo, baz}`), and assignments (`select {foo=bar, baz=fuz}`).

- Some significant changes regarding SQL dialects:

  - Operators and functions can be defined on per-dialect basis. (@aljazerzen,
    #2681)
  - _Breaking_: The `sql.duckdb` target supports DuckDB 0.8 (@eitsupi, #2810).
  - _Breaking_: The `sql.hive` target is removed (@eitsupi, #2837).

- New arithmetic operators. These compile to different function or operator
  depending on the target.

  - _Breaking_: Operator `/` now always performs floating division (@aljazerzen,
    #2684). See the
    [Division docs](https://prql-lang.org/book/reference/syntax/operators.html#division-and-integer-division)
    for details.

  - Truncated integer division operator `//` (@aljazerzen, #2684). See the
    [Division docs](https://prql-lang.org/book/reference/syntax/operators.html#division-and-integer-division)
    for details.

  - Regex search operator `~=` (@max-sixty, #2458). An example:

    ```prql no-eval
    from tracks
    filter (name ~= "Love")
    ```

    ...compiles to;

    ```sql
    SELECT
      *
    FROM
      tracks
    WHERE
      REGEXP(name, 'Love')
    ```

    ...though the exact form differs by dialect; see the
    [Regex docs](https://prql-lang.org/book/reference/syntax/operators.html#regex)
    for more details.

- New aggregation functions: `every`, `any`, `average`, and `concat_array`.
  _Breaking:_ Remove `avg` in favor of `average`.

- _Breaking:_ We've changed our function declaration syntax to match other
  declarations. Functions were one of the first language constructs in PRQL, and
  since then we've added normal declarations there's no compelling reason for
  functions to be different.

  ```prql no-eval
  let add = a b -> a + b
  ```

  Previously, this was:

  ```prql no-eval
  func add a b -> a + b
  ```

- Experimental modules, which allow importing declarations from other files.
  Docs are forthcoming.

- Relation literals create a relation (a "table") as an _array_ of _tuples_.
  This example demonstrates the new syntax for arrays `[]` and tuples `{}`.
  (@aljazerzen, #2605)

  ```prql no-eval
  from [{a=5, b=false}, {a=6, b=true}]
  filter b == true
  select a
  ```

- `this` can be used to refer to the current pipeline, for situations where
  plain column name would be ambiguous:

  ```prql no-eval
  from x
  derive sum = my_column
  select this.sum   # does not conflict with `std.sum`
  ```

  Within a `join` transform, there is also a reference to the right relation:
  `that`.

- _Breaking:_ functions `count`, `rank` and `row_number` now require an argument
  of the array to operate on. In most cases you can directly replace `count`
  with `count this`. The `non_null` argument of `count` has been removed.

**Features**:

- We've changed how we handle colors.

  `Options::color` is deprecated and has no effect. Code which consumes
  `prql_compiler::compile` should instead accept the output with colors and use
  a library such as `anstream` to handle the presentation of colors. To ensure
  minimal disruption, `prql_compiler` will currently strip color codes when a
  standard environment variable such as `CLI_COLOR=0` is set or when it detects
  `stderr` is not a TTY.

  We now use the [`anstream`](https://github.com/rust-cli/anstyle) library in
  `prqlc` & `prql-compiler`.

  (@max-sixty, #2773)

- `prqlc` can now show backtraces when the standard backtrace env var
  (`RUST_BACKTRACE`) is active. (@max-sixty, #2751)

**Fixes**:

- Numbers expressed with scientific notation — `1e9` — are now handled correctly
  by the compiler (@max-sixty, #2865).

**Integrations**:

- prql-python now provides type hints (@philpep, #2912)

**Internal changes**:

- Annotations in PRQL. These have limited support but are currently used to
  specify binding strengths. They're modeled after Rust's annotations, but with
  `@` syntax, more similar to traditional decorators. (#2729)

  ```prql no-eval
  @{binding_strength=11}
  let mod = l r -> s"{l} % {r}"
  ```

- Remove BigQuery's special handling of quoted identifiers, now that our module
  system handles its semantics (@max-sixty, #2609).

- ClickHouse is tested in CI (@eitsupi, #2815).

**New Contributors**:

- @maxmcd, with #2533
- @khoa165, with #2876
- @philpep, with #2912
- @not-my-profile, with #2971

## 0.8.1 — 2023-04-29

0.8.1 is a small release with a new `list-targets` command in `prqlc`, some
documentation improvements, and some internal improvements.

This release has 41 commits from 8 contributors.

From the broader perspective of the project, we're increasing the relative
prioritization of it being easy for folks to actually use PRQL — either with
existing tools, or a tool we'd build. We'll be thinking about & discussing the
best way to do that over the next few weeks.

## 0.8.0 — 2023-04-14

0.8.0 renames the `and` & `or` operators to `&&` & `||` respectively,
reorganizes the Syntax section in the book, and introduces `read_parquet` &
`read_csv` functions for reading files with DuckDB.

This release has 38 commits from 8 contributors. Selected changes:

**Features**:

- Rename `and` to `&&` and `or` to `||`. Operators which are symbols are now
  consistently infix, while "words" are now consistently functions (@aljazerzen,
  #2422).

- New functions `read_parquet` and `read_csv`, which mirror the DuckDB
  functions, instructing the database to read from files (@max-sixty, #2409).

## 0.7.1 — 2023-04-03

0.7.1 is a hotfix release to fix `prql-js`'s `npm install` behavior when being
installed as a dependency.

This release has 17 commits from 4 contributors.

## 0.7.0 — 2023-04-01

0.7.0 is a fairly small release in terms of new features, with lots of internal
improvements, such as integration tests with a whole range of DBs, a blog post
on Pi day, RFCs for a type system, and more robust language bindings.

There's a very small breaking change to the rust API, hence the minor version
bump.

Here's our April 2023 Update, from our
[Readme](https://github.com/PRQL/prql/blob/main/README.md):

> ### April 2023 update
>
> PRQL is being actively developed by a growing community. It's ready to use by
> the intrepid, either as part of one of our supported extensions, or within
> your own tools, using one of our supported language bindings.
>
> PRQL still has some minor bugs and some missing features, and probably is only
> ready to be rolled out to non-technical teams for fairly simple queries.
>
> Here's our current [Roadmap](https://prql-lang.org/roadmap/) and our
> [Milestones.](https://github.com/PRQL/prql/milestones)
>
> Our immediate focus for the code is on:
>
> - Building out the next few big features, including
>   [types](https://github.com/PRQL/prql/pull/1964) and
>   [modules](https://github.com/PRQL/prql/pull/2129).
> - Ensuring our supported features feel extremely robust; resolving any
>   [priority bugs](https://github.com/PRQL/prql/issues?q=is%3Aissue+is%3Aopen+label%3Abug+label%3Apriority).
>
> We're also spending time thinking about:
>
> - Making it really easy to start using PRQL. We're doing that by building
>   integrations with tools that folks already use; for example our VS Code
>   extension & Jupyter integration. If there are tools you're familiar with
>   that you think would be open to integrating with PRQL, please let us know in
>   an issue.
> - Making it easier to contribute to the compiler. We have a wide group of
>   contributors to the project, but contributions to the compiler itself are
>   quite concentrated. We're keen to expand this;
>   [#1840](https://github.com/PRQL/prql/issues/1840) for feedback.

---

The release has 131 commits from 10 contributors. Particular credit goes to to
@eitsupi & @jelenkee, who have made significant contributions, and
@vanillajonathan, whose prolific contribution include our growing language
bindings.

A small selection of the changes:

**Features**:

- `prqlc compile` adds `--color` & `--include-signature-comment` options.
  (@max-sixty, #2267)

**Web**:

- Added the PRQL snippets from the book to the
  [Playground](https://prql-lang.org/playground/) (@jelenkee, #2197)

**Internal changes**:

- _Breaking_: The `compile` function's `Options` now includes a `color` member,
  which determines whether error messages use ANSI color codes. This is
  technically a breaking change to the API. (@max-sixty, #2251)
- The `Error` struct now exposes the `MessageKind` enum. (@vanillajonathan,
  #2307)
- Integration tests run in CI with DuckDB, SQLite, PostgreSQL, MySQL and SQL
  Server (@jelenkee, #2286)

**New Contributors**:

- @k-nut, with #2294

## 0.6.1 — 2023-03-12

0.6.1 is a small release containing an internal refactoring and improved
bindings for C, PHP & .NET.

This release has 54 commits from 6 contributors. Selected changes:

**Fixes**:

- No longer incorrectly compile to `DISTINCT` when a `take 1` refers to a
  different set of columns than are in the `group`. (@max-sixty, with thanks to
  @cottrell, #2109)
- The version specification of the dependency Chumsky was bumped from `0.9.0` to
  `0.9.2`. `0.9.0` has a bug that causes an infinite loop. (@eitsupi, #2110)

**Documentation**:

- Add a policy for which bindings are supported / unsupported / nascent. See
  <https://prql-lang.org/book/project/bindings/index.html> for more details
  (@max-sixty, #2062) (@max-sixty, #2062)

**Integrations**:

- [prql-lib] Added C++ header file. (@vanillajonathan, #2126)

**Internal changes**:

- Many of the items that were in the root of the repo have been aggregated into
  `web` & `bindings`, simplifying the repo's structure. There's also `grammars`
  & `packages` (@max-sixty, #2135, #2117, #2121).

## 0.6.0 — 2023-03-08

0.6.0 introduces a rewritten parser, giving us the ability to dramatically
improve error messages, renames `switch` to `case` and includes lots of minor
improvements and fixes. It also introduces `loop`, which compiles to
`WITH RECURSIVE`, as a highly experimental feature.

There are a few cases of breaking changes, including switching `switch` to
`case`, in case that's confusing. There are also some minor parsing changes
outlined below.

This release has 108 commits from 11 contributors. Selected changes:

**Features**:

- Add a (highly experimental) `loop` language feature, which translates to
  `WITH RECURSIVE`. We expect changes and refinements in upcoming releases.
  (#1642, @aljazerzen)
- Rename the experimental `switch` function to `case` given it more closely
  matches the traditional semantics of `case`. (@max-sixty, #2036)
- Change the `case` syntax to use `=>` instead of `->` to distinguish it from
  function syntax.
- Convert parser from pest to Chumsky (@aljazerzen, #1818)
  - Improved error messages, and the potential to make even better in the
    future. Many of these improvements come from error recovery.
  - String escapes (`\n \t`).
  - Raw strings that don't escape backslashes.
  - String interpolations can only contain identifiers and not any expression.
  - Operator associativity has been changed from right-to-left to left-to-right
    to be more similar to other conventional languages.
  - `and` now has a higher precedence than `or` (of same reason as the previous
    point).
  - Dates, times and timestamps have stricter parsing rules.
  - `let`, `func`, `prql`, `case` are now treated as keywords.
  - Float literals without fraction part are not allowed anymore (`1.`).
- Add a `--format` option to `prqlc parse` which can return the AST in YAML
  (@max-sixty, #1962)
- Add a new subcommand `prqlc jinja`. (@aljazerzen, #1722)
- _Breaking_: prql-compiler no longer passes text containing `{{` & `}}` through
  to the output. (@aljazerzen, #1722)

  For example, the following PRQL query

  ```prql no-eval
  from {{foo}}
  ```

  was compiled to the following SQL previously, but now it raises an error.

  ```sql
  SELECT
    *
  FROM
    {{ foo }}
  ```

  This pass-through feature existed for integration with dbt.

  We're again considering how to best integrate with dbt, and this change is
  based on the idea that the jinja macro should run before the PRQL compiler.

  If you're interested in dbt integration, subscribe or 👍 to
  <https://github.com/dbt-labs/dbt-core/pull/5982>.

- A new compile target `"sql.any"`. When `"sql.any"` is used as the target of
  the compile function's option, the target contained in the query header will
  be used. (@aljazerzen, #1995)
- Support for SQL parameters with similar syntax (#1957, @aljazerzen)
- Allow `:` to be elided in timezones, such as `0800` in
  `@2020-01-01T13:19:55-0800` (@max-sixty, #1991).
- Add `std.upper` and `std.lower` functions for changing string casing
  (@Jelenkee, #2019).

**Fixes**:

- `prqlc compile` returns a non-zero exit code for invalid queries. (@max-sixty,
  #1924)
- Identifiers can contain any alphabetic unicode characters (@max-sixty, #2003)

**Documentation**:

- Operator precedence (@aljazerzen, #1818)
- Error messages for invalid queries are displayed in the book (@max-sixty,
  #2015)

**Integrations**:

- [prql-php] Added PHP bindings. (@vanillajonathan, #1860)
- [prql-dotnet] Added .NET bindings. (@vanillajonathan, #1917)
- [prql-lib] Added C header file. (@vanillajonathan, #1879)
- Added a workflow building a `.deb` on each release. (Note that it's not yet
  published on each release). (@vanillajonathan, #1883)
- Added a workflow building a `.rpm` on each release. (Note that it's not yet
  published on each release). (@vanillajonathan, #1918)
- Added a workflow building a Snap package on each release. (@vanillajonathan,
  #1881)

**Internal changes**:

- Test that the output of our nascent autoformatter can be successfully compiled
  into SQL. Failing examples are now clearly labeled. (@max-sixty, #2016)
- Definition files have been added to configure
  [Dev Containers](https://containers.dev/) for Rust development environment.
  (@eitsupi, #1893, #2025, #2028)

**New Contributors**:

- @linux-china, with #1971
- @Jelenkee, with #2019

## 0.5.2 — 2023-02-18

0.5.2 is a tiny release to fix an build issue in yesterday's `prql-js` 0.5.1
release.

This release has 7 commits from 2 contributors.

**New Contributors**:

- @matthias-Q, with #1873

## 0.5.1 — 2023-02-17

0.5.1 contains a few fixes, and another change to how bindings handle default
target / dialects.

This release has 53 commits from 7 contributors. Selected changes:

**Fixes**:

- Delegate dividing literal integers to the DB. Previously integer division was
  executed during PRQL compilation, which could be confusing given that behavior
  is different across DBs. Other arithmetic operations are still executed during
  compilation. (@max-sixty, #1747)

**Documentation**:

- Add docs on the `from_text` transform (@max-sixty, #1756)

**Integrations**:

- [prql-js] Default compile target changed from `Sql(Generic)` to `Sql(None)`.
  (@eitsupi, #1856)
- [prql-python] Compilation options can now be specified from Python. (@eitsupi,
  #1807)
- [prql-python] Default compile target changed from `Sql(Generic)` to
  `Sql(None)`. (@eitsupi, #1861)

**New Contributors**:

- @vanillajonathan, with #1766

## 0.5.0 — 2023-02-08

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

## 0.4.2 — 2023-01-25

**Features**:

- New `from_text format-arg string-arg` function that supports JSON and CSV
  formats. _format-arg_ can be `format:csv` or `format:json`. _string-arg_ can
  be a string in any format. (@aljazerzen & @snth, #1514)

  ```prql no-eval
  from_text format:csv """
  a,b,c
  1,2,3
  4,5,6
  """
  ```

  ```prql no-eval
  from_text format:json '''
      [{"a": 1, "b": "x", "c": false }, {"a": 4, "b": "y", "c": null }]
  '''
  ```

  ```prql no-eval
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

## 0.4.1 — 2023-01-18

0.4.1 comes a few days after 0.4.0, with a couple of features and the release of
`prqlc`, the CLI crate.

0.4.1 has 35 commits from 6 contributors.

**Features**:

- Inferred column names include the relation name (@aljazerzen, #1550):

  ```prql no-eval
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

## 0.4.0 — 2023-01-15

0.4.0 brings lots of new features including `case`, `select ![]` and numbers
with underscores. We have initial (unpublished) bindings to Elixir. And there's
the usual improvements to fixes & documentation (only a minority are listed
below in this release).

0.4.0 also has some breaking changes: `table` is `let`, `dialect` is renamed to
`target`, and the compiler's API has changed. Full details below.

**Features**:

- Defining a temporary table is now expressed as `let` rather than `table`
  (@aljazerzen, #1315). See the
  [tables docs](https://prql-lang.org/book/reference/declarations/variables.html)
  for details.

- _Experimental:_ The
  [`case`](https://prql-lang.org/book/reference/syntax/case.html) function sets
  a variable to a value based on one of several expressions (@aljazerzen,
  #1278).

  ```prql no-eval
  derive var = case [
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
  [`case` docs](https://prql-lang.org/book/reference/syntax/case.html) for more
  details.

- _Experimental:_ Columns can be excluded by name with `select` (@aljazerzen,
  #1329)

  ```prql no-eval
  from albums
  select ![title, composer]
  ```

- _Experimental:_ `append` transform, equivalent to `UNION ALL` in SQL.
  (@aljazerzen, #894)

  ```prql no-eval
  from employees
  append managers
  ```

  Check out the
  [`append` docs](https://prql-lang.org/book/reference/stdlib/transforms/append.html)
  for more details.

- Numbers can contain underscores, which can make reading long numbers easier
  (@max-sixty, #1467):

  ```prql no-eval
  from numbers
  select {
      small = 1.000_000_1,
      big = 5_000_000,
  }
  ```

- The SQL output contains a comment with the PRQL compiler version (@aljazerzen,
  #1322)
- `dialect` is renamed to `target`, and its values are prefixed with `sql.`
  (@max-sixty, #1388); for example:

  ```prql no-eval
  prql target:sql.bigquery  # previously was `dialect:bigquery`

  from employees
  ```

  This gives us the flexibility to target other languages than SQL in the long
  term.

- Tables definitions can contain a bare s-string (@max-sixty, #1422), which
  enables us to include a full CTE of SQL, for example:

  ```prql no-eval
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
  [`prql-compiler` docs](https://docs.rs/prql-compiler/latest/prql_compiler/)
  for more details.

**Fixes**:

[This release, the changelog only contains a subset of fixes]

- Allow interpolations in table s-strings (@aljazerzen, #1337)

**Documentation**:

[This release, the changelog only contains a subset of documentation
improvements]

- Add docs on aliases in
  [Select](https://prql-lang.org/book/reference/stdlib/transforms/select.html)
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

  ```prql no-eval
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

  ```prql no-eval
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

- Expose a shortened error message, in particular for the VS Code extension
  (@aljazerzen, #1005)

**Internal changes**:

- Specify 1.60.0 as minimum Rust version (@max-sixty, #1011)
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
[VS Code extension](https://github.com/PRQL/prql-code), courtesy of
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

- Add docs on Architecture (@aljazerzen, #904)
- Add Changelog (@max-sixty, #890 #891)

**Internal changes**:

- Start trial using Conventional Commits (@max-sixty, #889)
- Add crates.io release workflow, docs (@max-sixty, #887)

## 0.2.5 - 2022-07-29

0.2.5 is a very small release following 0.2.4 yesterday. It includes:

- Add the ability to represent single brackets in an s-string, with two brackets
  (#752, @max-sixty)
- Fix the "Copy to Clipboard" command in the Playground, for Firefox (#880,
  @mklopets)

## 0.2.4 - 2022-07-28

0.2.4 is a small release following 0.2.3 a few days ago. The 0.2.4 release
includes:

- Enrich our CLI, adding commands to get different stages of the compilation
  process (@aljazerzen , #863)
- Fix multiple `take n` statements in a query, leading to duplicate proxy
  columns in generated SQL (@charlie-sanders)
- Fix BigQuery quoting of identifiers in `SELECT` statements (@max-sixty)
- Some internal changes — reorganize top-level functions (@aljazerzen), add a
  workflow to track our Rust compilation time (@max-sixty), simplify our simple
  prql-to-sql tests (@max-sixty)

Thanks to @ankane, `prql-compiler` is now available from homebrew core;
`brew install prql-compiler`[^1].

[^1]:
    we still need to update docs and add a release workflow for this:
    <https://github.com/PRQL/prql/issues/866>

## 0.2.3 - 2022-07-24

A couple of weeks since the 0.2.2 release: we've squashed a few bugs, added some
mid-sized features to the language, and made a bunch of internal improvements.

The 0.2.3 release includes:

- Allow for escaping otherwise-invalid identifiers (@aljazerzen & @max-sixty)
- Fix a bug around operator precedence (@max-sixty)
- Add a section the book on the language bindings (@charlie-sanders)
- Add tests for our `Display` representation while fixing some existing bugs.
  This is gradually becoming our code formatter (@arrizalamin)
- Add a "copy to clipboard" button in the Playground (@mklopets)
- Add lots of guidance to our `CONTRIBUTING.md` around our tests and process for
  merging (@max-sixty)
- Add a `prql!` macro for parsing a prql query at compile time (@aljazerzen)
- Add tests for `prql-js` (@charlie-sanders)
- Add a `from_json` method for transforming json to a PRQL string (@arrizalamin)
- Add a workflow to release `prql-java` to Maven (@doki23)
- Enable running all tests from a PR by adding a `pr-run-all-tests` label
  (@max-sixty)
- Have `cargo-release` to bump all crate & npm versions (@max-sixty)
- Update `prql-js` to use the bundler build of `prql-js` (@mklopets)

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
interest and contributions! 0.2.2 has some fixes & some internal improvements:

- We now test against SQLite & DuckDB on every commit, to ensure we're producing
  correct SQL. (@aljazerzen)
- We have the beginning of Java bindings! (@doki23)
- Idents surrounded by backticks are passed through to SQL (@max-sixty)
- More examples on homepage; e.g. `join` & `window`, lots of small docs
  improvements
- Automated releases to homebrew (@roG0d)
- [prql-js](https://github.com/PRQL/prql/tree/main/prqlc/bindings/js) is now a
  single package for Node, browsers & webpack (@charlie-sanders)
- Parsing has some fixes, including `>=` and leading underscores in idents
  (@mklopets)
- Ranges receive correct syntax highlighting (@max-sixty)

Thanks to Aljaž Mur Eržen @aljazerzen , George Roldugin @roldugin , Jasper
McCulloch @Jaspooky , Jie Han @doki23 , Marko Klopets @mklopets , Maximilian
Roos @max-sixty , Rodrigo Garcia @roG0d , Ryan Russell @ryanrussell , Steven
Maude @StevenMaude , Charlie Sanders @charlie-sanders .

We're planning to continue collecting bugs & feature requests from users, as
well as working on some of the bigger features, like type-inference.

For those interesting in joining, we also have a new
[Contributing page](https://github.com/PRQL/prql/blob/main/.github/CONTRIBUTING.md).

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

[Contribute](https://github.com/PRQL/prql/blob/main/.github/CONTRIBUTING.md) to
the project — we're a really friendly community, whether you're a recent SQL
user or an advanced Rust programmer. We need bug reports, documentation tweaks &
feature requests — just as much as we need compiler improvements written in
Rust.

---

I especially want to give [Aljaž Mur Eržen](https://github.com/aljazerzen)
(@aljazerzen) the credit he deserves, who has contributed the majority of the
difficult work of building out the compiler. Much credit also goes to
[Charlie Sanders](https://github.com/charlie-sanders) (@charlie-sanders), one of
PRQL's earliest supporters and the author of pyprql, and
[Ryan Patterson-Cross](https://github.com/rbpatt2019) (@rbpatt2019), who built
the Jupyter integration among other Python contributions.

Other contributors who deserve a special mention include: @roG0d, @snth,
@kwigley

---

Thank you, and we look forward to your feedback!
