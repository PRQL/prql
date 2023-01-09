---
title: Functional databases
date: 2023-01-07
layout: article
---

I believe that many of the ideas from functional programming are very suitable
for a query language. To make a case for it, I'll briefly explain a few
features, adapt their syntax and show how they are used in PRQL.

## Fancy functional features

Functional programming is an old school programming paradigm, that is slowly
gaining traction within a few common languages that are designed with
procedural paradigm in mind.

The many aspects of paradigm have been discussed extensively, so I'll try to be
brief. If you've already read then all, I suggest you skip to the next section,
but if you haven't, I think you'll be glad that I decided to showcase the
features using pseudo code that looks like JavaScript. I know it has problem,
but it's something most of us can read.

### Pure functions

This is the core aspect that allows us to do most of the magic later. When we
say "pure function", we just mean the mathematical function and not "a
procedure" or "a method". To be more precise:

- Pure functions have no side effects. This means that no global state is
  altered and nothing is logged or printed. The function result is the only this
  output that the function produces.
- Pure functions produce the same output given the same args. The output does
  not depend on any external information, so no random can be called, no user
  input requested and no system calls made.

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
we can skip calling `my_function` for the second and the third time, because we
(or even better: the compiler) already know what the output will be.

This may seem like a minor point, but we just getting started.

### First class citizens

Nearly a buzz word, the saying "functions are first class citizens" means that
they are treated the same as any other value.

They can be stored in a variable, they can be passed to another function as an
argument or even be used in binary operations.

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

function add_one(y) { return add(1, y) }

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

In conventional programming languages[1], there is a difference between using
`my_function()` and `my_var`. The first one evaluates the expression at the call
site, while the second one evaluates it at the declaration site. It is also
possible to express just `my_function`, a reference to a function that can be
called without arguments.

But if all your functions are pure, we generalize by saying that all three cases
are equivalent.

To make this work, let's say that `my_function` is "implicitly invoked" just as
it would have been expressed as `my_function()`.

Also, if we declare all functions to be pure it doesn't matter exactly _when_
they are evaluated. So let's give the compiler the authority to make an informed
guess about _when_ and make `my_var` and `my_function()` semantically
equivalent.

[1]: Let's say that conventional languages are first 15 from this list:
https://survey.stackoverflow.co/2022/#most-popular-technologies-language-prof

## Syntax, surly shinier

First of all, lets change the function call to this:

```
(my_function arg1 arg2 arg3)
```

It may look strange because function name is within the parenthesis and there is
no commas between arguments. But trust me, there are benefits to this syntax.

Firstly, when the call has no arguments, the parenthesis can be omitted:

```
(my_function) == my_function
```

which feels very natural because of the similar behavior with expressions. As
intended, function call with no argument and a plain reference to the function
are expressed with the same syntax.

Let's extend this behavior and allow allow bare function calls in a few places
where they cannot become ambiguous:

```
# a list:
[my_function arg1 arg2 arg3, my_function arg4 arg5 arg6]
# a declaration:
let res = my_function arg1 arg2 arg3
```

Secondly, I'd argue that currying looks natural.

```
let curry = my_function arg1 arg2
let res = curry arg3

# or inline:
let res = (my_function arg1 arg2) arg3
```

Now let's go one step further and introduce a "pipe" operator. It applies left
operand as an argument to the right operand:

```
arg3 | my_function arg1 arg2
```

This is especially useful for chaining function calls. If we declare `div`,
`floor` and `mul` as common arithmetic functions, the function call syntax
really starts to make sense:

```
12 | div 5 | floor | mul 5
```

## Running real relations

Up to this point, we were talking language design without a clear justification.
Language being cool is fine, but it is much more important that the language is
actually the right tool for the job.

The job, in the case of PRQL, is querying databases.

Core unit of data here is a relation: an ordered set of rows, each of which
contains entries for each of the columns.

Queries operate in a static environment that contains references to database
tables and a library of standard functions.

```
mod default_db {
    let albums = ...
    let artists = ...
    let tracks = ...
}

mod std {
    func from tbl -> ...
    func select tbl -> ...
    func take range tbl -> ...

    func sum col -> ...
    func average col -> ...
}
```

The exact structure and naming may change, but the important part is that we
have global immutable variables that can be referenced from pure functions.

PRQL currently supports most of the features described above, focused around the
unconventional function call syntax.

A basic operation on a relation would be:

```
(take 3 default_db.albums)
# of with a pipeline
(default_db.albums | take 3 )
```

Now, because specifying `default_db` in table names is not beginner friendly, we
have a function `from` that implicitly uses the `default_db` module. It doesn't
do any work on the relation itself, though:

```
(from albums | take 3)
```

To make querying easier, PRQL also has some neat name resolution rules that
allow function arguments to refer to each other. In practice, it allows
referring to columns of a relation in function calls:

```
(select [title, artist_id] default_db.albums)
# with a pipeline:
(from albums | select [title, artist_id])
```

All these queries can be simplified to an expression of relations and scalars.
In PRQL, we call such an expression a Relational Query. It is an intermediate
representation of prql-compiler and can be translated into SQL to be executed on
basically any relational database.

## Appendix

### PRQL support

PRQL is a work is progress. It does not yet support all the features presented
here, namely:

- `let` syntax for variable declarations,
- inline currying - function call must that with a function name.
- syntax for declaring tables.

We focused on the core features and all of these can be worked around.

Also, PRQL may not ever get all of these features, because ideas in this article
are only my own and not necessarily of the whole PRQL core team.

### Many functions in SQL are not pure

And we don't have a plan on how to deal with that.

### In math, function call syntax is ambiguous

If you think about it, the function call syntax from math is kind of ambiguous.
For example, what does this mean:

```
a(b + 1)
```

If could either be a call of function a, or it could be just multiplication
where we omitted _. This is not a problem in conventional programming languages
because they don't allow omitting the _.
