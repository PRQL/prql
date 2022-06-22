---
title: "Roadmap"
url: roadmap
---

> I'm excited and inspired by the level of enthusiasm behind the project, both
> from individual contributors and the broader community of users who are
> unsatisfied with SQL. We currently have an initial working version for the
> intrepid early user.
>
> I'm hoping we can build a beautiful language, an app that's approachable &
> powerful, and a vibrant community. Many projects have reached the current stage
> and fallen, so this requires compounding on what we've done so far.
>
> -- <cite>Maximilian Roos</cite>

{{< columns >}}

## Language

Already since becoming public, the language has improved dramatically, thanks to
the feedback of dozens of contributors. The current state of the basics is now
stable and while we'll hit corner-cases, I expect we'll only make small changes
to the existing features — even as we continue adding features.

On this foundation we are planning to build advanced features like type checking,
function currying, pivot/melt/wide_to_long/long_to_wide operations, operator overloading and
[a few more](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3Alanguage-design).
This will take time, because we want to build a consistent language that feels like it is
made to last.

## Friendliness

Currently the compiler is not sufficiently friendly, despite significant recent improvements.
We'd like to make error messages better and sand off sharp corners.

Both bug reports of unfriendliness, and code contributions to improve them are welcome; there's a
[friendliness label.](https://github.com/prql/prql/issues?q=is%3Aissue+label%3Afriendlienss+is%3Aopen)

## Standard library

Currenty, the standard library is [quite limited](https://github.com/prql/prql/blob/main/prql-compiler/src/sql/stdlib.prql).
It contains only basic arithmetic functions (AVERAGE, SUM) and lacks functions for string manipulation,
date handling and many math functions.
Problem here is that PRQL is limited with functionality of target DBMS and its capabilities and the amount
of different dialects.

Before that, we need to setup a testing framework that would run queries against actual databases,
so we know that the dialect implementation is on-point.

<--->

## Alternative backends

Currently, PRQL only transpiles into SQL. It could be much more powerful (and in some cases performant)
if we develop a data-frame-handling-library backend. To be more precise, we would want to apply PRQL's
AST to a in-memory dataframe of a performance-optimized library (such as [Polars](https://www.pola.rs/)).

This would allow data scientists, analists and general Python developers to transform dataframes with
PRQL queries. One language for all data transformations.


## PRQL as a tool

PyPrql is a step into direction of a general data handling program. But we want to build a tool that
can read many data sources, offers syntax highlighting, auto-complete and type-inference using
information from database's schema.

If done right, it could replace many uses of classical spreadsheet software while producing reproducible
data transformations and intuitivate, interactive experienece with fast feedback.

## Integrations

PRQL is focused at the language layer, which means we can easily integrate with
existing tools & apps. This will be the primary way that people can start using
PRQL day-to-day. Probably the most impactful initial integrations will be tools that
engineers use to build data pipelines, like
[`dbt-prql`](https://github.com/prql/prql/issues/375).


## Not in focus

We should focus on solving a distinct problem really well. PRQL's goal is to
make reading and writing analytical queries easier, and so for the moment that
means putting some things out of scope:

- Building infrastructure outside of queries, like lineage. dbt is excellent at
  that! ([#13](https://github.com/prql/prql/issues/13)).
- Writing DDL / index / schema manipulation / inserting data
  ([#16](https://github.com/prql/prql/issues/16)).

{{< /columns >}}
