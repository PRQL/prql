# PRQL-js

> JavaScript wrapper library around prql Rust crate

**P**ipelined **R**elational **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a modern language for transforming data — a simpler and more powerful
SQL. Like SQL, it's readable, explicit and declarative. Unlike SQL, it forms a
logical pipeline of transformations, and supports abstractions such as variables
and functions. It can be used with any database that uses SQL, since it
transpiles to SQL.

Example:

```prql
from employees
filter country = "USA"                       # Each line transforms the previous result.
derive [                                     # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [              # `by` are the columns to group by.
  average salary,                            # These are aggregation calcs run on each group.
  sum     salary,
  average gross_salary,
  sum     gross_salary,
  average gross_cost,
  sum_gross_cost: sum gross_cost,
  ct: count,
]
sort sum_gross_cost
filter ct > 200
take 20
```

## Installation

```
npm install prql-js
```

This package is built to target a bundler (i.e. webpack). If you want to use it with Node.js or import it directly in a browser as an ES module, you will have to [build it yourself using a suitable `--target`](https://rustwasm.github.io/docs/wasm-pack/commands/build.html).

## Usage

```js
import compile from 'prql-js';

const sql = compile(`from employees | select first_name`);
console.log(sql);
```
Prints:
```
SELECT
  first_name
FROM
  employees
```

For more information about the language, see [reference book](https://prql-lang.org/reference) or [examples on GitHub](https://github.com/prql/prql/tree/main/examples).

## Development

Generated with [wasm-pack](https://rustwasm.github.io/docs/wasm-pack/tutorials/npm-browser-packages/index.html).

Build:

    wasm-pack build

Test:

    wasm-pack test --firefox
