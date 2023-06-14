# Internals

This chapter explains PRQL's semantics: how expressions are interpreted and
their meaning. It's intended for advanced users and compiler contributors.

It's also worth checking out the
[`prql-compiler` docs](https://docs.rs/prql-compiler/latest/prql_compiler/) for
more details on its API.

## Terminology

[_Relation_](<https://en.wikipedia.org/wiki/Relation_(database)>): Standard
definition of a relation in context of databases:

- An ordered set of tuples of form `(d_0, d_1, d_2, ...)`.
- Set of all `d_x` is called an attribute or a column. It has a name and a type
  domain `D_x`.

[_Table_](<https://en.wikipedia.org/wiki/Table_(database)#Tables_versus_relations>):
persistently stored relation. Some uses of this term actually mean to say
"relation".
