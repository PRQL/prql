# PRQL compiler

`prql-compiler` contains the implementation of PRQL's compiler, written in rust.

For more on PRQL, check out the [PRQL website](https://prql-lang.org) or the
[PRQL repo](https://github.com/PRQL/prql).

## Terminology

Relation = Standard definition of a relation in context of databases:

- An ordered set of tuples of form `(d_0, d_1, d_2, ...)`.
- Set of all `d_x` is called an attribute or a column. It has a name and a type
  domain `D_x`.

Frame = descriptor of a relation. Contains list of columns (with names and
types). Does not contain data.

Table = persistently stored relation. Some uses of this term actually mean to
say "relation".
