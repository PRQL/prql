---
title: "Roadmap"
url: roadmap
---

> We're excited and inspired by the level of enthusiasm behind the project, both
> from individual contributors and the broader community of users who are
> unsatisfied with SQL. We currently have an working version for the intrepid
> users.
>
> We're hoping we can build a beautiful language, integrations that are
> approachable & powerful, and a vibrant community. Many projects have reached
> the current stage and fallen, so this requires compounding on what we've done
> so far.
>
> -- <cite>PRQL Developers</cite>

{{< columns >}}

## Developer ergonomics

The PRQL language can offer a vastly improved developer experience, both when
exploring data and building robust data pipelines. We'd like to offer
autocomplete both for internal language features and columns of the underlying
database. We'd like to be able to offer developers a much faster iteration cycle
when writing a query, including type checking both the data (e.g. numbers vs.
strings) and containers (e.g. scalars vs. columns).

This requires development across multiple dimensions — writing an LSP server,
better support for typing in the compiler.

## Database cohesion

Currently PRQL compiles into SQL with no understanding of the underlying tables.
While our focus is on TODO

## Query transparency

PRQL's compiler already contains structured data about the query. We'd like to
offer transparency to tools which integrate PRQL, so they can offer lineage
information, such as which tables are queried, TODO

## SQL-to-PRQL conversion

While PRQL already allows for a gradual on-ramp — there's no need to switch
everything to PRQL right away — it would also be useful to be able to convert
existing SQL queries to PRQL, rather than having to rewrite them manually. For
many queries, this should be fairly easy. (For some it will be very difficult,
but we can start with the easy ones...)

## Rethinking joins

Currently joins are basically the same as SQL. Tools which have a semantic model
of the underlying tables can offer TODO

## Friendliness

Currently the compiler is not sufficiently friendly, despite significant recent
improvements. We'd like to make error messages better and sand off sharp
corners.

Both bug reports of unfriendliness, and code contributions to improve them are
welcome; there's a
[friendliness label.](https://github.com/PRQL/prql/issues?q=is%3Aissue+label%3Afriendliness+is%3Aopen)

## Standard library

Currently, the standard library is
[quite limited](https://github.com/PRQL/prql/blob/main/prql-compiler/src/semantic/std.prql).
It contains only basic arithmetic functions (`AVERAGE`, `SUM`) and lacks
functions for string manipulation, date handling and many math functions. One
challenge here is the variety of functionalities and syntax of target DBMSs;
e.g. there's no standard regex function. Improving our testing framework to
include integration tests will help give us confidence here.

## Milestones

We have assigned features to a broad timeline in our
[Milestones.](https://github.com/PRQL/prql/milestones)

<--->

## Alternative backends

Currently, PRQL only transpiles into SQL. It could be much more powerful (and in
some cases performant) if we develop a data-frame-handling-library backend. To
be more precise, we would want to apply PRQL's AST to a in-memory dataframe of a
performance-optimized library (such as [Polars](https://www.pola.rs/)).

This would allow data scientists, analysts and general Python developers to
transform DataFrames with PRQL queries. One language for all data
transformations!

## PRQL as a tool

`pyprql` is a step into direction of a general data handling program. Building
on this, we want to build a tool that can read many data sources, offers syntax
highlighting, auto-complete and type-inference using information from database's
schema.

We'll likely continue pursuing this through integrations with other tools;
combining the potential of PRQL with its openness and ecosystem.

If successful, we can have reproducible data transformations with an intuitive,
interactive experience with fast feedback.

## Integrations

PRQL is focused at the language layer, which means we can easily integrate with
existing tools & apps. This will be the primary way that people can start using
PRQL day-to-day. Probably the most impactful initial integrations will be tools
that engineers use to build data pipelines, like
[`dbt-prql`](https://github.com/PRQL/prql/issues/375).

## Not in focus

We should focus on solving a distinct problem really well. PRQL's goal is to
make reading and writing analytical queries easier, and so for the moment that
means putting some things out of scope:

- Building infrastructure outside of queries, like lineage. dbt is excellent at
  that! ([#13](https://github.com/PRQL/prql/issues/13)).
- Writing DDL / index / schema manipulation / inserting data
  ([#16](https://github.com/PRQL/prql/issues/16)).

{{< /columns >}}

<!--

TODO: What's remaining in the language itself (not the stdlib)?


## Language

The language is now fairly stable. While we'll hit corner-cases, we expect we'll
only make small changes to the existing features, even as we continue adding
features.

There are still some features that are missing: a native regex operator,

On this foundation we are planning to build advanced features like type
checking, function currying,  -->

<!--

TODO: are we planning to offer these? Where the underlying DB doesn't offer them, it'll be quite hard!

pivot/melt/wide_to_long/long_to_wide operations,
operator overloading and
[a few more](https://github.com/PRQL/prql/issues?q=is%3Aissue+is%3Aopen+label%3Alanguage-design). -->
