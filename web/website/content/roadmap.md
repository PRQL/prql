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
[quite limited](https://github.com/PRQL/prql/blob/main/prqlc/crates/prql-compiler/src/semantic/std.prql).
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

While PRQL compiler will never depend on a database to compile queries, an LSP
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

#### Language

While the core semantics and syntax of the language are now fairly stable, we
are planning
[a few major features](https://github.com/PRQL/prql/issues?q=is%3Aopen+is%3Aissue+label%3Amajor-feature+label%3Alanguage-design)
that will give PRQL the feeling of a real programming language and elevate it in
[the chomsky hierarchy](https://en.wikipedia.org/wiki/Chomsky_hierarchy).
Honorable mentions here are recursive CTEs (or rather functions), algebraic type
system, pre-specified join conditions and regex.

Note that these features will probably inflict breaking changes with each minor
release before we stabilize the 1.0, the first indefinitely supported language
edition.

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

### PRQL IDE

We'd like to make it easier to try PRQL. We currently have the playground, which
compiles PRQL and runs queries with a DuckDB wasm module, but there's much more
we could do. Could we support for importing arbitrary CSV and parquet input
files and then exporting the results? Could it integrate an LSP?

We can balance this against building integrations with existing tools.

{{< /columns >}}

## Not in focus

We should focus on solving a distinct problem really well. PRQL's goal is to
make reading and writing analytical queries easier, and so for the moment that
means putting some things out of scope:

- Building infrastructure outside of queries, like lineage. dbt is excellent at
  that! ([#13](https://github.com/PRQL/prql/issues/13)).
- Writing DDL / index / schema manipulation / inserting data
  ([#16](https://github.com/PRQL/prql/issues/16)).
