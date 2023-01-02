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

#### Type system

Because PRQL is meant to be the querying interface of the database, a type
system that can describe database schema as well as all intermediate results of
the queries is needed. We want it to provide clear distinctions between
different nullable and non-nullable values, and different kinds of containers
(e.g. scalars vs. columns).

Currently PRQL compiles into SQL with no understanding of the underlying tables.
We plan to introduce database schema declarations into the language, so PRQL
compiler and tooling can enrich the developer experience with autocomplete and
early error messages.

The goal here is to catch all errors at PRQL compile time, instead of at the
database's PREPARE stage.

<--->

#### Friendliness

Currently the compiler output's friendliness is variable — sometimes it produces
much better error messages than SQL, but sometimes they can be confusing.

Both bug reports of unfriendliness, and code contributions to improve them are
welcome; there's a
[friendliness label.](https://github.com/PRQL/prql/issues?q=is%3Aissue+label%3Afriendliness+is%3Aopen)

#### Developer ergonomics — LSP

The PRQL language can offer a vastly improved developer experience over SQL,
both when exploring data and building robust data pipelines. We'd like to offer
autocomplete both for PRQL itself and for columns of the underlying database,
because fast iteration cycle can drastically decrease frustrations caused by
banal misspellings.

This requires development across multiple dimensions — writing an
[LSP server](https://langserver.org/), better support for typing in the
compiler, and possibly database cohesion.

While PRQL compiler will never depend on a database to compile queries, LPS
server could greatly help with generating type definitions from the information
schema of a database.

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
multiple tools.

For example, we could compile PRQL to RQ (Relational Query intermediate
representation) and then use that to apply the transformations to an in-memory
dataframe of a performance-optimized library (such as
[Polars](https://www.pola.rs/)) or a Google Sheets spreadsheet. Alternatively,
we could even convert RQ to [Substrait](https://substrait.io/).

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

aljaz: I did a breakdown of all issues marked with "language-design":

Work in progress:

- #761 Intersect and Difference Operators
- #656 Union operator
- #286 Notation for creating sample data
- #172 an exclude clause, that would select all columns except whats specified

Stale discussions:

- #819 Syntax to break an expression over multiple lines?
- #1069 Should window default to `expanding` with a `sort`?
- #523 Table definition syntax
- #1286 switch / case / match semantics
- Consistency/correctness:
  - #1111 Deterministic/pure functions
  - #985 Corrections of SQL's aggregation functions
  - #905 `null`s in expressions
- Joins:
  - #1206 Join three tables on same column name
  - #723 Natural Joins
  - #716 Consider Datalog-like logic variable based JOINs

Major features TODO:

- #1384 feature request: grouping sets
- #1123 filter foo LIKE ""%abc%""? (regex)
- #562 Regex implementation
- #993 filter based on a list of values (this is IN, ANY, ALL)
- #746 `include` other prql files & module system
- #644 Pivot and melt
- #610 Date diff with `-` operator
- #366 Date to string function
- #566 Resolve \* to specific column names
- #407 `WITH RECURSIVE` based iteration
- #381 Types
- #54 Table vs Value types

Low priority:

- #1225 Support escaping quotes inside strings & S-strings
- #1092 Mutation queries (DML)
- #968 Caching relations
- #879 `LATERAL` joins (AKA `CROSS APPLY`) - necessary for subqueries in which
  the rhs can reference the lhs
- #730 Multiline comments and prql-doc
- #643 Should compiler strive to evaulate expressions?
- #438 Struct syntax
- #14 JSON queries
- #285 Write a language specification (EBNF syntax)
- #82 Inline filters

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
