# prqlc

## Installation

`prqlc` can be installed with `cargo`:

```sh
cargo install prqlc
```

...or built from source:

```sh
cargo install --path prql-compiler/prqlc
```

<!-- It can be installed via brew too:

```sh
brew install prql/prql/prql-compiler
``` -->

## Usage

```sh
$ echo "from employees | filter has_dog | select salary" | prqlc compile

SELECT
  *
FROM
  employees
WHERE
  has_dog
```
