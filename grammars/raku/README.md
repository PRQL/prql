# Raku grammar

PRQL grammar for Raku.

## Instructions

```raku
use lib '.';
use prql;

say PRQL.parse('from employees');
say PRQL.parsefile('employees.prql');
```

## Installation

To install from source run:

    zef install .

## Tests

Tests can be run individually by specifying the test filename on the command
line:

    raku t/arithmetics.rakutest

To run all tests in the directory you have to install `prove6` using `zef`:

    zef install App::Prove6
    prove6 --lib t/

Each test asserts the shape of the tree the grammar builds, not only that the
query parses:

    parses-to 'filter 10 * 10', 'statement(pipeline-statement(pipeline(call-expression(identifier,test(test-inner(binary-test(expression(number(integer)),arith-op,expression(number(integer)))))))))';

`parses-to` comes from `t/lib/PRQLTest.rakumod`, which renders a `Match` as
`rule(child,child)` over the grammar's named captures. To see the shape a query
produces before writing the assertion:

    raku -I lib -I t/lib -e 'use PRQLTest; use prql; say shape(PRQL.parse("filter 1 + 1"))'

## Documentation

- https://docs.raku.org/language/grammar_tutorial
- https://docs.raku.org/language/grammars
