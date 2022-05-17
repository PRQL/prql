# PRQL reference compiler

## Installation

PRQL can be installed with `cargo`, or built from source.

```sh
cargo install prql-compiler
```

## Usage

```sh
$ echo "from employees | filter has_dog" | prql compile

SELECT
  *
FROM
  employees
WHERE
  has_dog
```
