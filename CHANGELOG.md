# PRQL Changelog

## 0.2.12 — [unreleased]

Features:

Fixes:

Documentation:

Web:

Integrations:

Internal changes:

## 0.2.11 — 2022-11-20

0.2.11 contains a few helpful fixes.

Work continues on our `semantic` refactor — look out for 0.3.0 soon! Many thanks
to @aljazerzen for his continued contributions to this.

Note: 0.2.10 was skipped due to this maintainer's inability to read his own docs
on bumping versions...

Features:

- Detect when compiler version is behind query version (@MarinPostma, #1058)
- Add `__version__` to prql-python package (@max-sixty, #1034)

Fixes:

- Fix nesting of expressions with equal binding strength and left associativity,
  such as `a - (b - c)` (@max-sixty, #1136)
- Retain floats without significant digits as floats (@max-sixty, #1141)

Documentation:

- Add documentation of `prqlr` bindings (@eitsupi, #1091)
- Add a 'Why PRQL' section to the website (@max-sixty, #1098)
- Add @snth to core-devs (@max-sixty, #1050)

Internal changes:

- Use workspace versioning (@max-sixty, #1065)

## 0.2.9 — 2022-10-14

0.2.9 is a small release containing a bug fix for empty strings.

Fixes:

- Fix parsing of empty strings (@aljazerzen, #1024)

## 0.2.8 — 2022-10-10

0.2.8 is another modest release with some fixes, doc improvements, bindings
improvements, and lots of internal changes. Note that one of the fixes causes
the behavior of `round` and `cast` to change slightly — though it's handled as a
fix rather than a breaking change in semantic versioning.

Fixes:

- Change order of the `round` & `cast` function parameters to have the column
  last; for example `round 2 foo_col` /
  `cast int foo`. This is consistent with other functions, and makes piping
  possible:

  ```prql
  derive [
    gross_salary = (salary + payroll_tax | as int),
    gross_salary_rounded = (gross_salary | round 0),
  ]
  ```

Documentation:

- Split `DEVELOPMENT.md` from `CONTRIBUTING.md` (@richb-hanover, #1010)
- Make s-strings more prominent in website intro (@max-sixty, #982)

Web:

- Add GitHub star count to website (@max-sixty, #990)

Integrations:

- Expose a shortened error message, in particular for the VSCode extension
  (@aljazerzen, #1005)

Internal changes:

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

We also have new features in the [VSCode
extension](https://github.com/prql/prql-code), courtesy of @jiripospisil,
including a live output panel.

Fixes:

- `range_of_ranges` checks the Range end is smaller than its start (@shuozeli, #946)

Documentation:

- Improve various docs (@max-sixty, #974, #971, #972, #970, #925)
- Add reference to EdgeDB's blog post in our FAQ (@max-sixty, #922)
- Fix typos (@kianmeng, #943)

Integrations:

- Add `prql-lib`, enabling language bindings with `go` (@sigxcpu76, #923)
- Fix line numbers in JS exceptions (@charlie-sanders, #929)

Internal changes:

- Lock the version of the rust-toolchain, with auto-updates (@max-sixty, #926, #927)

## 0.2.6 — 2022-08-05

Fixes:

- Adjust `fmt` to only escape names when needed (@aljazerzen, #907)
- Fix quoting on upper case `table` names (@max-sixty, #893)
- Fix scoping of identical column names from multiple tables (@max-sixty, #908)
- Fix parse error on newlines in a `table` (@sebastiantoh 🆕, #902)
- Fix quoting of upper case table names (@max-sixty, #893)

Documentation:

- Add docs on [Architecture](prql-compiler/ARCHITECTURE.md) (@aljazerzen, #904)
- Add Changelog (@max-sixty, #890 #891)

Internal changes:

- Start trial using Conventional Commits (@max-sixty, #889)
- Add crates.io release workflow, docs (@max-sixty, #887)

## 0.2.5 - 2022-07-29

0.2.5 is a very small release following 0.2.4 yesterday. It includes:

- Add the ability to represent single brackets in an s-string,
  with two brackets (#752, @max-sixty )
- Fix the "Copy to Clipboard" command in the Playground, for Firefox (#880,
  @mklopets )

## 0.2.4 - 2022-07-28

0.2.4 is a small release following 0.2.3 a few days ago. The 0.2.4 release includes:

- Enrich our CLI, adding commands to get different stages
  of the compilation process (@aljazerzen , #863)
- Fix multiple `take n` statements in a query,
  leading to duplicate proxy columns in generated SQL (@charlie-sanders )
- Fix BigQuery quoting of identifiers in `SELECT` statements (@max-sixty )
- Some internal changes — reorganize top-level functions (@aljazerzen ),
  add a workflow to track our rust compilation time (@max-sixty ),
  simplify our simple prql-to-sql tests (@max-sixty )

Thanks to @ankane, `prql-compiler` is now available from homebrew core;
`brew install prql-compiler`[^2].

[^2]: we still need to update docs and add a release workflow for this: <https://github.com/prql/prql/issues/866>

## 0.2.3 - 2022-07-24

A couple of weeks since the 0.2.2 release: we've squashed a few bugs,
added some mid-sized features to the language,
and made a bunch of internal improvements.

The 0.2.3 release includes:

- Allow for escaping otherwise-invalid identifiers (@aljazerzen & @max-sixty )
- Fix a bug around operator precedence (@max-sixty )
- Add a section the book on the language bindings (@charlie-sanders )
- Add tests for our `Display` representation while fixing some existing bugs.
  This is gradually becoming our code formatter (@arrizalamin )
- Add a "copy to clipboard" button in the Playground (@mklopets )
- Add lots of guidance to our `CONTRIBUTING.md` around our tests
  and process for merging (@max-sixty )
- Add a `prql!` macro for parsing a prql query at compile time (@aljazerzen )
- Add tests for `prql-js` (@charlie-sanders )
- Add a `from_json` method for transforming json to a
  PRQL string (@arrizalamin )
- Add a workflow to release `prql-java` to Maven (@doki23 )
- Enable running all tests from a PR by adding a `pr-run-all-tests`
  label (@max-sixty )
- Have `cargo-release` to bump all crate & npm versions (@max-sixty )
- Update `prql-js` to use the bundler build of `prql-js` (@mklopets )

As well as those contribution changes, thanks to those who've reported issues,
such as @mklopets @huw @mm444 @ajfriend.

From here, we're planning to continue squashing bugs
(albeit more minor than those in this release),
adding some features like `union`,
while working on bigger issues such as type-inference.

We're also going to document and modularize the compiler further.
It's important that we give more people an opportunity to contribute
to the guts of PRQL, especially given the number and enthusiasm of
contributions to project in general —
and it's not that easy to do so at the moment.
While this is ongoing if anyone has something they'd like
to work on in the more difficult parts of the compiler,
let us know on GitHub or Discord, and we'd be happy to
work together on it.

Thank you!

## 0.2.2 - 2022-07-10

We're a couple of weeks since our 0.2.0 release.
Thanks for the surge in interest and contributions!
0.2.2[^1] has some fixes & some internal improvements:

- We now test against SQLite & DuckDB on every commit,
  to ensure we're producing correct SQL. (@aljazerzen )
- We have the beginning of Java bindings! (@doki23 )
- Idents surrounded by backticks are passed through to SQL (@max-sixty )
- More examples on homepage; e.g. `join` & `window`,
  lots of small docs improvements
- Automated releases to homebrew (@roG0d )
- [prql-js](https://github.com/prql/prql/tree/main/prql-js) is now
  a single package for node, browsers & webpack (@charlie-sanders )
- Parsing has some fixes, including `>=` and leading underscores
  in idents (@mklopets )
- Ranges receive correct syntax highlighting (@max-sixty )

Thanks to Aljaž Mur Eržen @aljazerzen , George Roldugin @roldugin ,
Jasper McCulloch @Jaspooky , Jie Han @doki23 , Marko Klopets @mklopets ,
Maximilian Roos @max-sixty , Rodrigo Garcia @roG0d ,
Ryan Russell @ryanrussell , Steven Maude @StevenMaude ,
Charlie Sanders @charlie-sanders .

We're planning to continue collecting bugs & feature requests from users,
as well as working on some of the bigger features, like type-inference.

For those interesting in joining, we also have a new
[Contributing page](https://github.com/prql/prql/blob/main/CONTRIBUTING.md).

[^1]: Think of 0.2.1 like C+ :)

## 0.2.0 - 2022-06-27

🎉 🎉 **After several months of building, PRQL is ready to use!** 🎉 🎉

---

How we got here:

At the end of January, we published a proposal of a better language
for data transformation: PRQL.
The reception was better than I could have hoped for —
we were no. 2 on HackerNews for a day, and gained 2.5K GitHub stars
over the next few days.

But man cannot live on GitHub Stars alone — we had to do the work to build it.
So over the next several months, during many evenings & weekends,
a growing group of us gradually built the compiler, evolved the language,
and wrote some integrations.

We want to double-down on the community and its roots in open source —
it's incredible that a few of us from all over the globe have
collaborated on a project without ever having met.
We decided early-on that PRQL would always be open-source and would
never have a commercial product (despite lots of outside interest
to fund a seed round!).
Because languages are so deep in the stack, and the data stack
has so many players, the best chance of building a
great language is to build an open language.

---

We still have a long way to go. While PRQL is usable,
it has lots of missing features, and an incredible amount of
unfulfilled potential, including a language server,
cohesion with databases, and type inference.
Over the coming weeks, we'd like to grow the number of
intrepid users experimenting PRQL in their projects,
prioritize features that will unblock them,
and then start fulfilling PRQL's potential by working through
our [roadmap](https://prql-lang.org/roadmap/).

The best way to experience PRQL is to try it. Check out our
[website](https://prql-lang.org) and the
[Playground](https://prql-lang.org/playground).
Start using PRQL for your own projects in
[dbt](https://github.com/prql/dbt-prql),
[Jupyter notebooks](https://pyprql.readthedocs.io/en/latest/magic_readme.html) and Prefect workflows.

Keep in touch with PRQL by following the project on
[Twitter](https://twitter.com/prql_lang),
joining us on [Discord](https://discord.gg/eQcfaCmsNc),
starring the [repo](https://github.com/prql/prql).

[Contribute](https://github.com/prql/prql/blob/main/CONTRIBUTING.md)
to the project — we're a really friendly community,
whether you're a recent SQL user or an advanced rust programmer.
We need bug reports, documentation tweaks & feature requests —
just as much as we need compiler improvements written in rust.

---

I especially want to give
[Aljaž Mur Eržen](https://github.com/aljazerzen) (@aljazerzen)
the credit he deserves, who has contributed the majority of
the difficult work of building out the compiler.
Much credit also goes to
[Charlie Sanders](https://github.com/charlie-sanders)
(@charlie-sanders),
one of PRQL's earliest supporters and the author of PyPrql, and
[Ryan Patterson-Cross](https://github.com/orgs/prql/people/rbpatt2019)
(@rbpatt2019), who built the Jupyter integration
among other Python contributions.

Other contributors who deserve a special mention include:
@roG0d, @snth, @kwigley

---

Thank you, and we look forward to your feedback!
