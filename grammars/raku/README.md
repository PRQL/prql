# Raku grammar

PRQL grammar for Raku.

## Instructions

```raku
use lib '.';
use prql;

say PRQL.parse('from employees');
say PRQL.parsefile('employees.prql');
```

## Tests

Tests can be run individually by specifying the test filename on the command
line:

    raku t/arithmetics.rakutest

To run all tests in the directory you have to install `prove6` using `zef`:

    zef install App::Prove6
    prove6 --lib t/

## Documentation

- https://docs.raku.org/language/grammar_tutorial
- https://docs.raku.org/language/grammars
