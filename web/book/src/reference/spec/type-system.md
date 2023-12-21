# Type system

> The type system determines the allowed values of a term.
>
> -- Wikipedia

## Purpose

Each of the SQL DBMSs has their own type system. Thanks to SQL standard, they
are very similar, but have key differences. For example, SQLite does not have a
type for date or time or timestamps, but it has functions for handling date and
time that take ISO 8601 strings or integers that represent Unix timestamps. So
it does support most of what is possible to do with dates in other dialects,
even though it stores data with a different physical layout and uses different
functions to achieve that.

PRQL's task is to define common description of _data formats_, just as how it
already defines common _data transformations_.

We believe this should best be done in two steps:

1. Define PRQL's Type System (PTS), following principles we think a relational
   language should have (and not focus on what existing SQL DBMSs have).

2. Define a mapping between SQL Type System (STS) and PTS, for each of the
   DBMSs. Ideally we'd want that to be a bijection, so each type in PTS would be
   represented by a single type in STS and vice-versa. Unfortunately this is not
   entirely possible, as shown below.

In practical terms, we want for a user to be able to:

- ... express types of their database with PRQL (map their STS into PTS). In
  some cases, we can allow to say "your database is not representable with PRQL,
  change it or use only a subset of it". An example of what we don't want to
  support are arrays with arbitrary indexes in Postgres (i.e. 2-based index for
  arrays).

  This task of mapping to PTS could be automated by LSP server, by introspecting
  user's SQL database and generating PRQL source.

- ... express their SQL queries in PRQL. Again, using mapping from STS to PTS,
  one should be able to express any SQL operation in PRQL.

  For example, translate MSSQL `DATEDIFF` to subtraction operator `-` in PRQL.

  For now, this mapping is manual, but should be documented and may be
  automated.

- ... use any PRQL feature in their database. Here we are mapping back from PTS
  into STS. Note that STS may have changed to a different dialect.

  For example, translate PRQL's datetime operations to use TEXT in SQLite.

  As of now, prql-compiler already does a good job of automatically doing this
  mapping.

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
harder to express, see (this
post)[https://www.parsonsmatt.org/2019/03/19/sum_types_in_sql.html ].

The value proposition here is that algebraic types give a lot modeling
flexibility, all while being conceptually simple.

**Composable** - as with transformation, we'd want types to compose together.

Using Python, JavaScript, C++ or Rust, one could define many different data
structures that would correspond to our idea of "relation". Most of them would
be an object/struct that has column names and types and then a generic array of
arrays for rows.

PRQL's type system should also be able to express relations as composed from
primitive types, but have only one idiomatic way of doing so.

In practice this means that builtin types include only primitives (int, text,
bool, float), tuple (for product), enum (for sum) and array (for repeating).

An SQL row would translate to tuple, and a relation would translate to an array
of tuples.

I would also strive for the type system to be minimal - don't differentiate
between tuples, objects and structs. Choose one and stick to it.

**Type constraints** - constrain a type with a predicate. For example, have a
type of `int64`s that are equal or greater than 10. Postgres
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

## Theory

> For any undefined terms used in this section, refer to set theory and
> mathematical definitions in general.

A "type of a variable" is a "set of all possible values of that variable". This
means that terms "type" and "set" are equivalent in this context.

Types (sets) can be expressions. For example, a union of two types is a type
itself. This means a type expression is equivalent to any other expression whose
type is a "set of sets".

So let's introduce a "set" as a PRQL expression construct (alongside existing
idents, literals, ranges and so on). For now, it does not need any special
syntax. Because sets are normal expressions, existing syntax can be repurposed
to define operations on sets:

- Binary operation `or` of two sets represents a union of those two sets:

  ```
  let number = int or float
  ```

  With algebraic types, this is named "a sum type".

- Literals can be coerced into a singleton set (i.e. `false` is converted into a
  set with only one element `false`):

  ```
  let int_or_null = int or null
  ```

- A list of set expressions can be coerced into a set of tuples, where entries
  of the tuples correspond to elements of the set expressions in the list:

  ```
  let my_row = [id = int, bool, name = str]
  ```

- An array of set expressions with exactly one entry can be coerced into a set
  of arrays of that set expression:

  ```
  let array_of_int = {int} # proposed syntax for arrays
  ```

- A function that takes set as params and returns a set is converted into a set
  of functions.

  ```
  let floor_signature = (float -> int)
  # using a proposed syntax for lambda functions
  ```

Module `std` defines built-in sets `int`, `float`, `bool`, `text` and `set`.
Other built-in sets will be added in the future.

## Type annotations

Let's extend the syntax for declaration of variable `a`, whose value can be
computed by evaluating `x`, with a type annotation:

```
let a <t> = x
```

This extended syntax applies following assertions:

- `t` can be evaluated statically (at compile time),
- `t` can be coerced into a set,
- value of `x` (and `a`) must be an element of `t`. This assertion must be
  possible to evaluate statically.

Similar rules apply to type annotations of return types of functions and
function parameter definitions.

## Type definitions

As shown, types can be defined by defining expressions and coercing them to set
expressions by using `< >`.

But similar to how both `func` and `let` can be used to define functions (when
we introduce lambda function syntax), let's also introduce syntactic sugar for
type definitions:

```
# these two are equivalent
let my_type <set> = set_expr
type my_type = set_expr
```

## Container types

> Terminology is under discussion

**Tuple** is the only product type in PTS. It contains n ordered fields, where n
is known at compile-time. Each field has a type itself and an optional name.
Fields are not necessarily of the same type.

In other languages, similar constructs are named record, struct, tuple, named
tuple or (data)class.

**Array** is a container type that contains n ordered fields, where n is not
known at compile-time. All fields are of the same type and cannot be named.

**Relation** is an array of tuples.

The first argument of transforms `select` and `derive` contains a known number
of entries, which can be of different types. Thus, it is a tuple.

```
select [1.4, false, "foo"]
```

### Normalization

Say we have a type expression E, composed of possibly many operators. To
minimize this expression, we employ process of normalization, which is meant to
take advantage of the following equalities:

```
A & A = A

A & B = (), where:
 - A and B are primitive, singleton, tuple or array
 - and A != B

A - (A | B) = (), where

A | () = A
```

We can see that intersection and difference are the main simplifying operations,
which is why the normalization 'sinks them into the expression', to maximize the
number of their interactions.

Rules for normalization during intersection:

```
(A | B)      & C            = (A & C) | (B & C)
A            & (B | C)      = (A & B) | (A & C)

(A - B)      & C            = (A & C) - B
A            & (B - C)      = (A & B) - C

{A, B}       & {C, D}       = {A & C, B & D}
[A]          & [B]          = [A & B]

A            & A            = A
A            & B            = never
```

Rules for normalization during difference:

```
(A | B)      - C            = (A - C) | (B - C)
(A - B)      - C            = A - (B | C)

{A, B}       - {C, D}       = {A - C, B - D}
{A, B}       - C            = {A, B}
A            - {B, C}       = A

[A]          - [B]          = [A - B]
[A]          - B            = [A]
A            - [B]          = A

A            - (B - C)      = A & not (B & not C) = A & (not B | C) = (A & not B) | (A & C) = (A - B) | (A & C)
A            - (A | B)      = ()
```

## Physical layout

_Logical type_ is user-facing the notion of a type that is the building block of
the type system.

_Physical layout_ is the underlying memory layout of the data represented by a
variable.

In many programming languages, physical layout of a logical type is dependent on
the target platform. Similarly, physical layout of a PRQL logical type is
dependent on representation of that type in the target STS.

```
PTS logical type  --->  STS logical type  ---> STS physical layout
```

Note that STS types do not have a single physical layout. Postgres has a logical
(pseudo)type `anyelement`, which is a super type of any data type. It can be
used as a function parameter type, but does not have a single physical layout so
it cannot be used in a column declaration.

For now, PRQL does not define physical layouts of any type. It is not needed
since PRQL is not used for DDL (see section "Built-in primitives") or does not
support raw access to underlying memory.

As a consequence, results of a PRQL query cannot be robustly compared across
DBMSs, since the physical layout of the result will vary.

In the future, PRQL may define a common physical layout of types, probably using
Apache Arrow.

<!-- ## Enums

```
# user-defined enum
type open
type pending
type closed
type status = open or pending or closed
``` -->

## Examples

```
type my_relation = {[
	id = int,
	title = text,
	age = int
]}

type invoices = {[
    invoice_id = int64,
    issued_at = timestamp,
    labels = {text}

    #[repr(json)]
    items = [{
        article_id = int64,
        count = int16 where x -> x >= 1,
    }],
    paid_by_user_id = int64 or null,
    status = status,
]}
```

## Appendix

### Built-in primitives

This document mentions `int32` and `int64` as distinct types, but there is no
need for that in the initial implementation. The built-in `int` can associate
with all operations on integers and translate PRQL to valid SQL regardless of
the size of the integer. Later, `int` cam be replaced by:

```
type int = int8 || int16 || int32 || int64
```

The general rule for "when to make a distinction between types" would be "as
soon as the types carry different information and we find an operation that
would be expressed differently". In this example, that would require some
operation on `int32` to have different syntax than same operation over `int64`.

We can have such relaxed rule because PRQL is not aiming to be a Data Definition
Language and does not have to bother with exact physical layout of types.

### Type representations

There are cases where a PTS type has multiple possible and valid representations
in some STSs.

For such cases, we'd want to support the use of alternative representations for
storing data, but also application of any function that is defined for the
original type.

Using SQLite as an example again, users may have some temporal data stored as
INTEGER unix timestamp and some as TEXT that contains ISO 8601 without timezone.
From the user's perspective, both of these types are `timestamp`s and should be
declared as such. But when compiling operations over these types to SQL, the
compiler should consider their different representations in STS. For example a
difference between two timestamps `timestamp - timestamp` can be translated to a
normal int subtraction for INTEGER repr, but must apply SQLite's function
`unixepoch` when dealing with TEXT repr.

Table declarations should therefore support annotations that give hints about
which representation is used:

```
table foo {
    #[repr(text)]
    created_at: timestamp,
}
```

A similar example is an "array of strings type" in PTS that could be represented
by a `text[]` (if DBMS supports arrays) or `json` or it's variant `jsonb` in
Postgres. Again, the representation would affect operators: in Postgres, arrays
would be accessed with `my_array[1]` and json arrays would use
`my_json_array -> 1`. This example may not be applicable, if we decide that we
want a separate JSON type in PST.

### RQ functions, targets and reprs

> This part is talks about technical implementations, not the language itself

#### Idea

RQ contains a single node kind for expressing operations and functions:
BuiltInFunction (may be renamed in the future).

It is a bottleneck that we can leverage when trying to affect how an operator or
a function interacts with different type representations on different targets.

Idea is to implement the BuiltInFunction multiple times and annotate it with it
intended target and parameter representation. Then we can teach the compiler to
pick the appropriate function implementation that suit current repr and
compilation target.

#### Specifics

RQ specification is an interface that contains functions, identified by name
(i.e. `std.int8.add`). These functions have typed parameters and a return value.
If an RQ function call does not match the function declaration in number or in
types of the parameters, this is considered an invalid RQ AST.

We provide multiple implementations for each RQ function. They are annotated
with a target (i.e. `#[target(sql.sqlite)]`) and have their params annotated
with type reprs (i.e. `#[repr(int)]`).

```
# using a made-up syntax

#[target(sql.sqlite)]
func std.int8.add
    #[repr(int8)] x
    #[repr(int8)] y
    -> s"{x} + {y}"
```

Each RQ type has one canonical repr that serves as the reference implementation
for other reprs and indicates the amount of contained data (i.e. 1 bit, 8 bits,
64 bits).

#### Example

Let's say for example, that we'd want to support 8bit integer arithmetic, and
that we'd want the result of `127 + 1` to be `-128` (ideally we'd handle this
better, but bear with me for the sake of the example). Because some RDBMSs don't
support 8bit numbers and do all their integer computation with 64bit numbers
(SQLite), we need to implement an alternative type representation for that
target.

The logical type `int8` could have the following two reprs:

- canonical `repr_int8` that contains 8 bits in two's complement, covering
  integer values in range -128 to 127 (inclusive),
- `repr_int64` that contains 64 bits of data, but is using only the values that
  are also covered by `repr_int8`.

Now we'd implement function `std.int8.add` for each of the reprs. Let's assume
that the `int8` implementation is straightforward and that databases don't just
change the data type when a number overflows. The impl for `int64` requires a
CASE statement that checks if the value would overflow and subtact 256 in that
case.

The goal here is that the results of the two impls are equivalent. To validate
that, we also need a way to convert between the reprs, or another `to_string`
function, implemented for both reprs.
