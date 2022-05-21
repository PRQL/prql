# prql-js

JavaScript bindings for [`prql-compiler`](../prql-compiler/README.md). This uses
[`wasm-pack`](https://rustwasm.github.io/docs/wasm-pack/tutorials/npm-browser-packages/index.html)
to generate bindings[^1].

[^1]: though we would be very open to other approaches, and used `trunk`
successfully in a rust-driven approach to this, RIP `prql-web`.

## Installation

To install the currently published version:

```sh
npm install prql-js
```

This package is built to target a bundler (i.e. webpack). To use it with Node.js
or import it directly in a browser as an ES module, [build it using a suitable
`--target`](https://rustwasm.github.io/docs/wasm-pack/commands/build.html).

## Usage

```js
import compile from 'prql-js';

const sql = compile(`from employees | select first_name`);
console.log(sql);
```

Prints:

```sql
SELECT
  first_name
FROM
  employees
```

For more information about the language, see [reference book](https://prql-lang.org/reference).

## Development

Build:

```sh
wasm-pack build
```

This builds a node package in the `pkg` path. An example of including that as a
dependency is in [`playground`](../playground/package.json).

Test:

```sh
wasm-pack test --firefox
```
