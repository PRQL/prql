# Type system

> For variables, the type system determines the allowed values of that term.
>
> -- Wikipedia

## Purpose

Each of the SQL DBMSs has their own type system. Thanks to SQL standard, they
are very similar, but have key differences. For example, SQLite does not have a
type for date or time or timestamps, but it has functions for handling date and
time that take ISO8603 strings or ints representing Unix timestamps. So it does
support most of what is possible to do with dates, it just stores a different
data format and uses different functions to achieve that.

PRQL's task is to define common description of _data formats_, just as how it
already defines _data transformations_.

I believe this should best be done in two steps:

1. Define PRQL's Type System (PTS), following principles we think a relational
   language should have (and not focus on what existing SQL DBMSs have).

2. Define a mapping from the SQL's Type System (STS) into PTS, for each of the
   DBMSs. Ideally we'd want that to be a bijection, so each type in PTS would be
   represented by a single type in STS and vice-versa. Unfortunately this is not
   extirely possible, as shown below.

In practical terms, we want for a user to be able to:

- ... express types of their database with PRQL. In some cases, we can allow to
  say "your database is not representable with PRQL, change it or use only a
  subset of it". An example of what we don't want to support are arrays with
  arbitrary indexes in Postgres (i.e. 2-based index for arrays). This task could
  be automated by LSP server, via introspecting user's database and producing
  PRQL source.

- ... express their SQL queries in PRQL.

- ... use any PRQL feature in their database. For example, translate PRQL's
  datetime operations to use TEXT in SQLite.

Example of the mapping between PTS and two STSs:

| PTS       | STS Postgres | STS SQLite |
| --------- | ------------ | ---------- |
| int32     | integer      | INTEGER    |
| int64     | bigint       | INTEGER    |
| timestamp | timestamp    | TEXT       |

## Principles

**Algebraic types** - have a way of expressing sum and product types. In Rust,
sum would be an enum and product would be tuple or a struct. In SQL, product
would be a row, since it can contain different types, all at once. Sum would be
harder to express, see 
(this post)[https://www.parsonsmatt.org/2019/03/19/sum_types_in_sql.html].

The value proposition here is that algebraic types give a lot modeling
flexibility, all while being conceptually simple.

**Composable** - as with transformation, we'd want types to compose together.

Using Python, JavaScript, C++ or Rust, one could define many different a data
structure that would correspond to our idea of "relation". Most of them would be
an object/struct that has column names and types and then a generic array of
arrays for rows.

PRQL's type system should also be able to express relation as composed from
primitive types, but have only one idiomatic way of doing so.

In practice this means that builtin types include only primitives (int, text,
bool), struct (for product), enum (for sum) and list (for repeating).

An SQL row would translate to struct, and a relation would translate to a list
of structs.

I would also strive for the type system to be minimal - don't differentiate
between tuples, objects and structs. Choose one and stick to it.

**Type constraints** - constrain a type with a predicate. For example, have a
type of int64 that are equal or greater than 10. Postgres
[does support this](https://news.ycombinator.com/item?id=34835063). The primary
value of using constrained types would not be validation (as it is used in
linked article), but when matching the type.

Say, for example, that we have a pipeline like this:

```
derive color = switch [x => 'red', true => 'green']
derive is_red = switch [color == 'red' => true, color == 'green' => false]
```

It should be possible to infer that `color` is of type `text`, but only when
equal to `'red'` or `'green'`. This means that the second switch covers all
possible cases and `is_red` cannot be `null`.

## Syntax

```
# built-in types
type int
type float
type bool
type text
type char
type null

# users-defined types
type my_int = int

# sum type (union or enum)
type number = int | float
type scalar = number | bool | str | char

# by default types are not nullable
type my_nullable_int = int | null

# user-defined enum
type open
type pending
type closed
type status = open | pending | closed

# product type
type my_struct = [id = int_nullable, name = str]
type my_timestamp_with_timezone = [int, text]

# list type (I'm not sure about this syntax)
type list = {T}

# relations are list of structs
type my_relation = {[
	id = int,
	title = text,
	age = int
]}
```

## Built-in primitives

I've mentioned having `int32` and `int64` as distinct types, but initial
implementation I don't see a need to do that. Instead we can start with a
built-in `int`, which can later be replaced by `type int = int32 | int64`.

The general rule for "when to make a distinction between types" would be "as
soon as we find an operation that would be expressed differently". In this
example, that would require some operation on `int32` to have different syntax
than same operation over `int64`.

We can have such relaxed rule because PRQL is not aiming to be a Data Definition
Language and does not have to bother with exact bit layout of types.

## Appendix

### Non-bijection cases between PTS and STS

There are cases where a PTS construct has multiple possible and valid
representations in some STSs.

For such cases, we'd want to have something similar to Rust's `#[repr(X)]` which
says "data in this type is represented as X" (we'd probably want a different
syntax).

This is needed because translation from PRQL operation to SQL may depend on the
representation.

Using SQLite as an example again, users may have some data stored as INTEGER and
some as TEXT, but would want to define both of them as PTS `timestamp`. They
would attach `#[repr(INTEGER)]` or `#[repr(TEXT)]` to the type. This would
effect how `timestamp - timestamp` is translated into SQL. INTEGER can use
normal int subtraction, but TEXT must apply `unixepoch` first.

A similar example is a "string array type" in PTS that could be represented by
an `text[]` (if DBMS supports arrays) or `json` or it's variant `jsonb` in
Postgres. Again, the representation would affect operators: in Postgres arrays
can be access with `my_array[1]` and json uses `my_json_array -> 1`. This
example may not be applicable, if we decide that we want a separate JSON type in
PST.
