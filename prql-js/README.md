# prql-js

JavaScript bindings for [`prql-compiler`](https://github.com/prql/prql/). Check out <https://prql-lang.org> for more
context.

## Installation

```sh
npm install prql-js
```

## Usage

Currently these functions are exported

```javascript
function to_sql(prql_string); // returns SQL string
function to_json(prql_string); // returns AST as JSON
function compile(prql_string); // returns CompileResult, with error source lines and cols
```

### From NodeJS

```javascript
const prql = require("prql-js/dist/node/prql_js.js");

const compileResult = prql.compile("from employees | select first_name");
console.log(compileResult);

const sql = prql.to_sql("from employees | select first_name");
console.log(sql);

// Whats returned from to_json is a JSON string, so parse it here.
const json = JSON.parse(prql.to_json("from employees | select first_name"));
console.log(json);
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
        const sql = to_sql("from employees | select first_name");

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

const sql = to_sql(`from employees | select first_name`);
console.log(sql);
```

## Notes

This uses
[`wasm-pack`](https://rustwasm.github.io/docs/wasm-pack/tutorials/npm-browser-packages/index.html)
to generate bindings[^1].

[^1]:
    though we would be very open to other approaches, and used `trunk`
    successfully in a rust-driven approach to this, RIP `prql-web`.
