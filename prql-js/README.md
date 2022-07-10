# prql-js

JavaScript bindings for [`prql-compiler`](https://github.com/prql/prql/). Check out <https://prql-lang.org> for more
context.

## Installation

```sh
npm install prql-js
```

## Usage

Currently these functions are exposed

```javascript
function compile(prql_string) # returns CompileResult
function to_sql(prql_string) # returns SQL string
function to_json(prql_string) # returns JSON string ( needs JSON.parse() to get the json)
```

### From NodeJS

```javascript
const prql = require("prql-js/dist/node/prql_js.js");

const { sql, error } = compile(`from employees | select first_name`);
console.log(sql);
```

### From a Browser

```html
<html>
  <head>
    <script src="./node_modules/prql-js/dist/web/prql_js.js"></script>
    <script>
      const { compile } = wasm_bindgen;

      async function run() {
        await wasm_bindgen("./node_modules/prql-js/dist/web/prql_js_bg.wasm");
        const sql = compile("from employees | select first_name").sql;

        console.log(sql);
      }

      run();
    </script>
  </head>

  <body></body>
</html>
```

### From a Framework or a Bundler

```typescript
import compile from "prql-js/dist/bundler/prql_js";

const sql = compile(`from employees | select first_name`).sql;
console.log(sql);
```

## Notes

This uses
[`wasm-pack`](https://rustwasm.github.io/docs/wasm-pack/tutorials/npm-browser-packages/index.html)
to generate bindings[^1].

[^1]:

though we would be very open to other approaches, and used `trunk`
successfully in a rust-driven approach to this, RIP `prql-web`.

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
