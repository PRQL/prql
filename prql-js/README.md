# prql-js

JavaScript bindings for [`prql-compiler`](https://github.com/prql/prql).

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
const prql = require("prql-js");

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
        const { sql, error } = compile("from employees | select first_name");

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
import compile from "prql-js/dist/bundler";

const { sql, error } = compile(`from employees | select first_name`);
console.log(sql);
```

## Development

Build:

```sh
npm run build
```

This builds Node, bundler and web packages in the `dist` path.

Test:

```sh
npm test
```

## Notes

- This uses
  [`wasm-pack`](https://rustwasm.github.io/docs/wasm-pack/)
  to generate bindings.
- We've added an `npm` layer on top of the usual approach of just using
  `wasm-pack`, so we can distribute a single package with targets of `node`,
  `bundler` and `no-modules` — somewhat inverting the approach recommended by
  `wasm-pack`. The build instruction goes in a `build` script, rather than a
  `pack` script. We're open to alternatives!
