# PRQL

<!-- User badges on first line (language docs & chat) -->
[![Language Docs](https://img.shields.io/badge/DOCS-LANGUAGE-blue?style=for-the-badge)](https://prql-lang.org)
[![Discord](https://img.shields.io/discord/936728116712316989?label=discord%20chat&style=for-the-badge)](https://discord.gg/eQcfaCmsNc)
<!-- Dev badges on first line (language docs & chat) -->
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

Here's a short example of the language; for more examples, visit
[prql-lang.org][PRQL Website]. To experiment with PRQL in the browser, check out
[PRQL Playground][PRQL Playground].

```elm
from employees                                # Each line transforms the previous result.
filter start_date > @2021-01-01               # Clear date syntax.
derive [                                      # `derive` adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost   # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (                      # `group` runs a pipeline over each group.
  aggregate [                                 # `aggregate` reduces each group to a row.
    average salary,
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost = sum gross_cost,          # `=` sets a column name.
    ct = count,
  ]
)
sort [sum_gross_cost, -country]               # `-country` means descending order.
filter ct > 200
take 20
```

## Resources

- [PRQL Website][PRQL Website]
- [PRQL Playground][PRQL Playground] — experiment with PRQL in the browser.
- [PRQL Book][PRQL Book] — read documentation on the language.
- [PyPRQL Docs](https://pyprql.readthedocs.io) — read documentation on PyPRQL, the
  python bindings to PRQL, including Jupyter magic.
- [PRQL VSCode Extension](https://marketplace.visualstudio.com/items?itemName=prql.prql)
- [PRQL-js](https://www.npmjs.com/package/prql-js) — JavaScript bindings for PRQL.

<!-- this document is intended for developers and contributors of the language -->
## Contributing

If you're interested in joining the community to build a better SQL, there are
lots of ways of contributing; big and small:

- Star this repo.
- Send a link to PRQL to a couple of people whose opinion you respect.
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
  - Start with something tiny! Write a test / write a docstring / make some rust
    nicer — it's a great way to get started in 30 minutes.
- Contribute towards the language.
  - Find instances where the compiler produces incorrect results, and post a bug
    report — feel free to use the [online compiler](https://prql-lang.org/playground).
  - Open an issue / append to an existing issue with examples of queries that
    are difficult to express in PRQL — especially if more difficult than SQL.
  - With sufficient examples, suggest a change to the language! (Though
    suggestions *without* examples are difficult to engage with, so please do
    anchor suggestions in examples.)

Any of these will inspire others to invest their time and energy into the
project; thank you in advance.

### Development environment

Setting up a local dev environment is simple, thanks to the rust ecosystem:

- Install [`rustup` & `cargo`](https://doc.rust-lang.org/cargo/getting-started/installation.html).
- That's it! Running `cargo test` should complete successfully.
- For more advanced development; e.g. adjusting `insta` outputs or compiling for
  web, run the commands in [Taskfile.yml](Taskfile.yml), either by copying &
  pasting or by installing [Task](https://taskfile.dev/#/installation) and
  running `task setup-dev`.
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
- [**@charlie-sanders**](https://github.com/charlie-sanders) — Charlie Sanders

We welcome others to join who have a track record of contributions.

[PRQL Book]: https://prql-lang.org/book
[PRQL Website]: https://prql-lang.org
[PRQL Playground]: https://prql-lang.org/playground
