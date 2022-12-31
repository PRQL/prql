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

## Medium term

{{< columns >}}

#### Integrations

PRQL is focused at the language layer, which means we can easily integrate with
existing tools & apps. Integrations will be the primary way that people can
start using PRQL day-to-day. At first, the most impactful initial integrations
will be tools that engineers use to build data pipelines, like
[`dbt-prql`](https://github.com/PRQL/prql/issues/375).

#### Standard library

Currently, the standard library is
[quite limited](https://github.com/PRQL/prql/blob/main/prql-compiler/src/semantic/std.prql).
It contains only basic arithmetic functions (`AVERAGE`, `SUM`) and lacks
functions for string manipulation, date handling and many math functions. We're
looking to gradually introduce these as needed, and reduce the need for
s-strings.

One challenge here is the variety of functionalities and syntax of target DBMSs;
e.g. there's no standard regex function.

#### Strong typing support

including type checking both the data (e.g. numbers vs. strings) and containers
(e.g. scalars vs. columns).

#### Friendliness

Currently the compiler output's friendliness is variable — sometimes it produces
much better error messages than SQL, but sometimes they can be confusing.

Both bug reports of unfriendliness, and code contributions to improve them are
welcome; there's a
[friendliness label.](https://github.com/PRQL/prql/issues?q=is%3Aissue+label%3Afriendliness+is%3Aopen)

<--->

#### Developer ergonomics — LSP

The PRQL language can offer a vastly improved developer experience over SQL,
both when exploring data and building robust data pipelines. We'd like to offer
autocomplete both for PRQL itself and for columns of the underlying database.
We'd like to be able to offer developers a much faster iteration cycle when
writing a query,

This requires development across multiple dimensions — writing an
[LSP server](https://langserver.org/), better support for typing in the
compiler, possibly database cohesion.

#### Database cohesion

Currently PRQL compiles into SQL with no understanding of the underlying tables.
While PRQL never _require_ a database to compile queries, we can enrich the
developer experience — for example, autocomplete, or early error messages — with
information about the underlying tables, such as their columns and types. We'll
likely implement this with an intermediate layer which can be stored on disk.

#### Query transparency

PRQL's compiler already contains structured data about the query. We'd like to
offer transparency to tools which use PRQL, so they can offer lineage
information, such as which tables are queried, and a DAG of transformations for
each column.

{{< /columns >}}

## Long term

{{< columns >}}

#### SQL-to-PRQL conversion

While PRQL already allows for a gradual on-ramp — there's no need to switch
everything to PRQL right away — it would also be useful to be able to convert
existing SQL queries to PRQL, rather than having to rewrite them manually. For
many queries, this should be fairly easy. (For some it will be very difficult,
but we can start with the easy ones...)

#### Rethinking joins

Currently joins are not fundamentally different from SQL's approach.

Tools which have a semantic model of the underlying tables can offer a better
experience here, such as pre-specifying join conditions. While PRQL's focus is
on the developer experience rather than heavy semantic models, we should
consider whether there are ways to make joins easier without introducing weight
to the language.

<--->

#### Alternative backends

Currently, PRQL only transpiles into SQL, using connectors such as DuckDB to
access other formats, such as Pandas dataframes. But PRQL can be much more
general than SQL — we could directly compile to any relational backend, offering
more flexibility and performance — and a consistent experience for those who use
multiple tools. For example, we could to apply PRQL's AST to a in-memory
dataframe of a performance-optimized library (such as
[Polars](https://www.pola.rs/)), or to [Substrait](https://substrait.io/) or to
Google Sheets.

#### PRQL as a tool

<!-- @snth do you want to mention prql-query? I would but I know you've been suggesting we delay -->

We'll likely continue pursuing this through integrations with other tools;
combining the potential of PRQL with its openness and ecosystem.

If successful, we can have reproducible data transformations with an intuitive,
interactive experience with fast feedback.

{{< /columns >}}

## Not in focus

We should focus on solving a distinct problem really well. PRQL's goal is to
make reading and writing analytical queries easier, and so for the moment that
means putting some things out of scope:

- Building infrastructure outside of queries, like lineage. dbt is excellent at
  that! ([#13](https://github.com/PRQL/prql/issues/13)).
- Writing DDL / index / schema manipulation / inserting data
  ([#16](https://github.com/PRQL/prql/issues/16)).

<!--

TODO: What's remaining in the language itself (not the stdlib)?


#### Language

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
<!--
We haven't kept this up to date — let's only add it if we think we have a path to doing that...

#### Milestones

We have assigned features to a broad timeline in our
[Milestones.](https://github.com/PRQL/prql/milestones) -->
