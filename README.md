# PRQL

<!-- User badges on first line (language docs & chat) -->

[![Language Docs](https://img.shields.io/badge/DOCS-LANGUAGE-blue?style=for-the-badge)](https://prql-lang.org)
[![Discord](https://img.shields.io/discord/936728116712316989?label=discord%20chat&style=for-the-badge)](https://discord.gg/eQcfaCmsNc)
[![Twitter](https://img.shields.io/twitter/follow/prql_lang?color=%231DA1F2&style=for-the-badge)](<https://twitter.com/prql_lang>)

<!-- Dev badges on first line (language docs & chat) -->

[![GitHub CI Status](https://img.shields.io/github/workflow/status/prql/prql/tests?logo=github&style=for-the-badge)](https://github.com/prql/prql/actions?query=workflow:tests)
[![GitHub contributors](https://img.shields.io/github/contributors/prql/prql?style=for-the-badge)](https://github.com/prql/prql/graphs/contributors)
[![Stars](https://img.shields.io/github/stars/prql/prql?style=for-the-badge)](https://github.com/prql/prql/stargazers)

**P**ipelined **R**elational **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a modern language for transforming data — a simple, powerful, pipelined
SQL replacement. Like SQL, it's readable, explicit and declarative. Unlike SQL, it forms a
logical pipeline of transformations, and supports abstractions such as variables
and functions. It can be used with any database that uses SQL, since it
transpiles to SQL.

PRQL was discussed on [Hacker
News](https://news.ycombinator.com/item?id=30060784#30062329) and
[Lobsters](https://lobste.rs/s/oavgcx/prql_simpler_more_powerful_sql) earlier
this year when it was just a proposal.

Here's a short example of the language; for more examples, visit
[prql-lang.org][prql website]. To experiment with PRQL in the browser, check out
[PRQL Playground][prql playground].

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

To learn more, check out the [PRQL Website][prql website].

<!-- should we have a call-to-action like following #1 or on Twitter? -->

For specific resources, check out:

- [PRQL Playground][prql playground] — experiment with PRQL in the browser.
- [PRQL Book][prql book] — the language documentation.
- [Contributing][contributing] — join us in building PRQL, through writing code or
  inspiring others to use it.
- [PyPRQL Docs](https://pyprql.readthedocs.io) — the PyPRQL documentation, the
  python bindings to PRQL, including Jupyter magic.
- [dbt-prql][dbt-prql] — write PRQL in dbt models.
- [PRQL VSCode Extension](https://marketplace.visualstudio.com/items?itemName=prql.prql)
- [PRQL-js](https://www.npmjs.com/package/prql-js) — JavaScript bindings for PRQL.

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

[prql book]: https://prql-lang.org/book
[prql website]: https://prql-lang.org
[prql playground]: https://prql-lang.org/playground
[contributing]: ./CONTRIBUTING.md
[dbt-prql]: https://github.com/prql/dbt-prql
