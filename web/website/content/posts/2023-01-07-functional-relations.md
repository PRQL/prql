---
title: A functional approach to relational queries
date: 2023-01-07
authors: ["Aljaž Mur Eržen"]
layout: article
toc: true
url: functional-relations
---

I believe that many of the ideas from functional programming are very suitable
for a query language. To make a case for it, I'll briefly explain a few
features, adapt their syntax, and show how they are used in PRQL.

## Fancy functional features

Functional programming is an old-school programming paradigm that is gradually
gaining traction within a few common languages that are designed with procedural
paradigm in mind.

The many aspects of the paradigm have been discussed extensively, so I'll try to
be brief. If you've already read them all, I suggest you skip to the next
section.

### Pure functions

This is the core aspect that allows us to do most of the magic later. When we
say "pure function", we just mean the mathematical function and not "a
procedure" or "a method". To be more precise:

- Pure functions have no side effects. This means that no global state is
  altered. The function result is the only output that the function produces.
- Pure functions produce the same output given the same args. The output does
  not depend on any external information; for example the current time or a
  random number generator.

The most obvious effect is that for a pure function `my_function`, this:

```js
res1 = my_function(arg);
res2 = my_function(arg);
res3 = my_function(arg);
```

... is equivalent to:

```js
res1 = my_function(arg);
res2 = res1;
res3 = res2;
```

Because `my_function` is guaranteed to return the same result when given `arg`,
we can skip calling `my_function` for the second and the third time because we
already know what the output will be.

This may seem like a minor point, but we're just getting started.

### First-class citizens

Functions being "first-class citizens" means that they are treated the same as
any other value; they can be stored in a variable, they can be passed to another
function as an argument or even be used in binary operations.

```js
function double(x) {
  return 2 * x;
}

let preprocess = double;

my_array = [4, 5, 6];

my_array.map(preprocess);
// yields [8, 10, 12]
```

Here, we've stored function `double` in variable `preprocess` and then passed
that to method `Array.map`, which applies a given function to each of the array
elements.

### Currying

Named after Haskell Curry, currying is an implicit act of converting a function
call with missing arguments into a new function.

```js
function add(x, y) {
  return x + y;
}
let add_one = add(1);
```

Because we didn't specify `y` parameter of `add` function, the result is a new
function that is still waiting for the last argument. It is equivalent to
defining add_one like this:

```js
function add_one(y) {
  return add(1, y);
}
```

### Implicit function call

Let's suppose that we have a pure function that doesn't take any arguments:

```js
function my_function() {
  return 42;
}
```

Because it is a pure function, it cannot depend on anything other than its
non-existing arguments. Another way to phrase it, this is a constant expression.

And as such, it might as well be defined as one:

```js
let my_var = 42;
```

In conventional programming languages[^1], there is a difference between using
`my_function()` and `my_var`. The first one evaluates the expression at the call
site, while the second one evaluates it at the declaration site. It is also
possible to express just `my_function`, a reference to a function that can be
called without arguments.

But if all functions are pure, we generalize by saying that all three cases are
equivalent.

To make this work, let's say that `my_function` is "implicitly invoked" just as
it would have been expressed as `my_function()`.

Also, if we declare all functions to be pure it doesn't matter exactly _when_
they are evaluated. So let's give the compiler the authority to make an informed
guess about _when_ and make `my_var` and `my_function()` semantically
equivalent.

[^1]:
    Let's say that conventional languages are the first 15 from this list:
    <https://survey.stackoverflow.co/2022/#most-popular-technologies-language-prof>

## Syntax, surly shinier

First of all, let's change the function call to this:

```prql
(my_function arg1 arg2 arg3)
```

It may look strange because the function name is within the parenthesis and
there are no commas between arguments. But there are benefits to this syntax.

Firstly, when the call has no arguments, the parenthesis can be omitted:

```prql
(my_function) == my_function
```

which feels very natural because of the similar behavior with expressions. As
intended, a function call with no argument and a plain reference to the function
are expressed with the same syntax.

Let's extend this behavior and allow bare function calls in a few places where
they won't become ambiguous:

```prql
# a list:
[my_function arg1 arg2 arg3, my_function arg4 arg5 arg6]
# a declaration:
let res = my_function arg1 arg2 arg3
```

Secondly, I'd argue that currying looks natural.

```prql
let curry = my_function arg1 arg2
let res = curry arg3

# or inline:
let res = (my_function arg1 arg2) arg3
```

Now let's go one step further and introduce a "pipe" operator. It applies its
left operand as an argument to the right operand:

```prql
arg3 | my_function arg1 arg2
```

This is especially useful for chaining function calls. If we declare `div`,
`floor` and `mul` as common arithmetic functions, the function call syntax
starts to make sense:

```prql
12 | div 5 | floor | mul 5
```

## Running real relations

Up to this point, we've discussed language design in the abstract, without
knowing what it will be used for. Designing for the sake of "language being
cool" is fine, but we do have a use for it, which means that the language must
be designed to be the right tool for the job.

The job, in the case of PRQL, is querying databases.

The core unit of data here is a "relation": an ordered set of rows, each of
which contains entries for each of the columns. (More people are likely to be
familiar with a "table", which is a relation stored in a DB.)

Queries operate in a static environment that contains references to database
tables and a library of standard functions. The important keyword here is
"static" by which I mean "global immutable variables" that can be referenced
from pure functions [^2].

PRQL currently supports most of the features described in previous sections,
focused on the innovative function call syntax.

[^2]:
    Many functions in SQL are not pure, and we don't have a plan on
    [how to deal with that](https://github.com/PRQL/prql/issues/1111).

### Basics

A basic operation on a relation would be:

```prql
(take 3 albums)
# or with a pipeline:
(albums | take 3)
```

... where `take` is a function and `albums` is a static relation. This does
exactly what it sounds like: it limits the result to the first n rows. One could
say it selects the top n rows.

In actual PRQL queries, we have some rules regarding references to tables, which
I'll talk about it some other time. For now, let's just say that for referencing
tables (i.e. static relations) use the function `from`. It has the bonus that it
makes queries look a lot more like SQL:

```prql
(from albums | take 3)
```

To make querying easier, we have some neat name resolution rules that allow
function arguments to refer to each other. In practice, it allows referring to
columns of a relation in function calls:

```prql
(select [title, artist_id] default_db.albums)
# and with a pipeline:
(from albums | select {title, artist_id})
```

All these queries can be simplified to an expression of relations and scalars.
In PRQL, we call such expressions "Relational Queries" or RQ for short. It is an
intermediate representation of prqlc and can be translated to SQL and executed
on basically any relational database.

This is the gist of how to express SQL queries with a functional language. At
this stage a curious reader might ask "can PRQL express any SQL query?" to which
I'd say: almost. Another might be wondering "but is it less cumbersome and more
consistent?" and I'd reply "very much, yes", but it depends on whom you ask. And
someone might say "why functional?" to which I say "exactly the question I was
waiting for!".

There are a few quick benefits I want to get out of the way:

- Pipelines make the flow of the query "top-to-bottom". An earlier part of a the
  pipeline will always be a valid query, regardless of what follows.
- Functions on relations (transforms) are designed to be orthogonal. Each of
  them has as few effects as possible and can be used at any point in the
  pipeline (except for `from`).

But there is more.

### Aggregate

At this point, I have to introduce `aggregate`. It takes a relation and produces
a single row using some aggregation function:

```prql
(from albums | aggregate {n_albums = count})
```

What happens if we don't specify the leading relation?

```prql
(aggregate {n_albums = count})
```

Because `aggregate` is missing an argument, currying kicks in, and the whole
expression evaluates to a function that is still waiting for a relation.

In SQL it is common to use GROUP BY when aggregating. If you think about it, it
essentially separates the relation into groups using some criteria and then
applies `aggregate` to each of the groups. This is exactly how PRQL expressed
it:

```prql
from albums | group artist_id (aggregate {n_albums = count})
```

This is a lot for one line, so let's unveil new syntactic conveniences: a new
line is a pipe operator and the top-level pipeline does not need parenthesis.
I'll also add a new transform at the back, don't worry about it.

```prql
from albums
group artist_id (
    aggregate {n_albums = count}
)
filter n_albums > 3
```

Notice how `group` operates on the whole relation it gets from `from` and that
`aggregate` is passed to `group` as a function to be applied to each of the
groups. `aggregate` then converts each of the groups into a single row and
returns that to `group` that composes these rows back together. This can now be
passed on to `filter`, which will remove any rows that don't match its
condition.

In SQL, a similar expression would have these four parts:

- projection in SELECT,
- an aggregation function that implicitly triggers aggregation,
- GROUP BY clause,
- HAVING clause.

These parts are entangled syntactically and semantically into one feature we
understand as GROUPING.

The beautiful realization is that these are 3 different operations that are
happening and that when separated they don't need to be associated with each
other. For example `filter` is more commonly expressed as WHERE and `group` has
other uses than `aggregate`.

But to separate these core relational operations, we need a way to express
aggregate as a function. This is why the functional paradigm fits the relational
data model.

### Distinct

A common question when learning SQL is "how do I select the row where column x
is smallest?". It has many variations, but there are two ways of doing it:

```sql
-- option 1
SELECT x, y, z FROM tab ORDER BY x LIMIT 1

-- option 2
SELECT x, y, z FROM tab WHERE x = (SELECT min(x) FROM tab)
```

[A follow-up question](https://stackoverflow.com/questions/3800551/select-first-row-in-each-group-by-group)
would be "how do I select the row where column x is smallest, for each group
over y?". This seems like a similar problem but the solution in SQL is
surprisingly different:

```sql
-- option 1 (unsupported in some dialects)
SELECT DISTINCT ON (y) x, y, z FROM tab ORDER BY y, x, z;

-- option 2  (supported by most dialects)
WITH summary AS (
    SELECT x, y, z,
        ROW_NUMBER() OVER(PARTITION BY y ORDER BY x) AS rank
    FROM tab)
SELECT * FROM summary WHERE rank = 1
```

Now break the query down into core operations. Essentially we want to do the
same thing we did before, but performed in groups by `y`. Before we used SQL
that can be expressed as `sort x | take 1` (which evaluates to a function), so
now surely this should work:

```prql
from tab
group y (sort x | take 1)
```

And it does. You can go and test this out in the
[PRQL playground](https://prql-lang.org/playground/).

Another variation of the question would be "how do I select a row, for each
group over all columns?". If you phrase it differently "group by all columns and
then take one row from each group". Or another way: "select distinct values of
all columns".

```prql
from tab
group tab.* (take 1)
```

As you can see, our language has many shortcuts for expressing operations such
as DISTINCT. This is convenient for us humans, but it's not a good base for a
relational query language.

I hope my point is clear: relational query language benefits a lot by separating
operations into orthogonal transforms[^3]. These transforms are in most cases
pure functions that are easiest to express in a functional language.

<!-- SQL, on the other hand, uses many keywords and syntactic constructs to
express the most commonly used combinations of these transforms. This falls short
when trying to express uncommon operations while not providing significantly
shorter queries. -->

If you want to see more of what PRQL is capable of, come and check out
[the project](https://github.com/PRQL/prql). It may not have monads (yet), but
it's probably better than what you are forced to use now.

[^3]:
    Transforms in PRQL are not completely orthogonal. `select`, `derive`,
    `aggregate` and `join` all manipulate relation columns. So in a sense, they
    are much closer to each other than they are to `take`.

## Appendix

### PRQL support

PRQL is a work in progress. It does not yet support all the features presented
here, namely:

- `let` syntax for variable declarations,
- inline currying - function call must start with a function name,
- syntax for declaring tables.

We focused on the core features and left these out because they can be worked
around.

Also, PRQL may not ever get all of these features, because the ideas in this
article are only my own and not necessarily of the whole PRQL core team.

### In math, function call syntax is ambiguous

If you think about it, the function call syntax from math is kind of ambiguous.
For example, what does this mean:

```math
a(b + 1)
```

It could either be a call of function a, or it could be just multiplication
where we omitted `*`. This is not a problem in conventional programming
languages because they don't allow omitting the `*`.
