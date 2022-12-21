# PRQL compiler

`prql-compiler` contains the implementation of PRQL's compiler, written in rust.

For more on PRQL, check out the [PRQL website](https://prql-lang.org) or the
[PRQL repo](https://github.com/prql/prql).

## Installation

`prql-compiler` can be installed with `cargo`:

```sh
cargo install prql-compiler
```

...or built from source:

```sh
# from prql/prql-compiler
cargo install --path .
```

It can be installed via brew too:

```sh
brew install prql/prql/prql-compiler
```

## Usage

```sh
$ echo "from employees | filter has_dog | select salary" | prql-compiler compile

SELECT
  *
FROM
  employees
WHERE
  has_dog
```

## Terminology

Relation = Standard definition of a relation in context of databases:

- An ordered set of tuples of form `(d_0, d_1, d_2, ...)`.
- Set of all `d_x` is called an attribute or a column. It has a name and a type
  domain `D_x`.

Frame = descriptor of a relation. Contains list of columns (with names and
types). Does not contain data.

Table = persistently stored relation. Some uses of this term actually mean to
say "relation".
