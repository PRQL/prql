---
title: "Milestone 0.1"
date: 2022-03-30
draft: false
---

PRQL just 0.1! This means:

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
