# prql-js

JavaScript bindings for [`prql-compiler`](https://github.com/PRQL/prql/). Check
out <https://prql-lang.org> for more context.

## Installation

```sh
npm install prql-js
```

## Usage

Currently these functions are exposed

```javascript
function compile(prql_query: string, options?: CompileOptions): string;

function prql_to_pl(prql_query: string): string;

function pl_to_rq(pl_json: string): string;

function rq_to_sql(rq_json: string): string;
```

### From NodeJS

Direct usage

```javascript
const prqljs = require("prql-js");

const sql = prqljs.compile(`from employees | select first_name`);
console.log(sql.sql);
```

Template literal

```javascript
const prqljs = require("prql-js");
const prql = (string) => prqljs.compile(string[0] || "");

const sql = prql`from employees | select first_name`;
console.log(sql.sql);
```

Template literal with newlines

```javascript
const prqljs = require("prql-js");
const prql = (string) => prqljs.compile(string[0] || "");

const sql = prql`
    from employees
    select first_name
`;
console.log(sql.sql);
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
        const sql = compile("from employees | select first_name");

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

const sql = compile(`from employees | select first_name`);
console.log(sql);
```

## Errors

Errors are returned as following object, serialized as a JSON array:

```ts
interface ErrorMessage {
  /// Plain text of the error
  reason: String;
  /// A list of suggestions of how to fix the error
  hint: String | null;
  /// Character offset of error origin within a source file
  span: Span | null;

  /// Annotated code, containing cause and hints.
  display: String | null;
  /// Line and column number of error origin within a source file
  location: SourceLocation | null;
}

/// Location within the source file.
/// Tuples contain:
/// - line number (1-based),
/// - column number within that line (1-based),
interface SourceLocation {
  start: [number, number];

  end: [number, number];
}
```

These errors can be caught as such:

```javascript
try {
  const sql = prqlJs.compile(`from employees | foo first_name`);
} catch (error) {
  const errorMessages = JSON.parse(error.message).inner;

  console.log(errorMessages[0].display);
  console.log(errorMessages[0].location);
}
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

- This uses [`wasm-pack`](https://rustwasm.github.io/docs/wasm-pack/) to
  generate bindings.
- We've added an `npm` layer on top of the usual approach of just using
  `wasm-pack`, so we can distribute a single package with targets of `node`,
  `bundler` and `no-modules` — somewhat inverting the approach recommended by
  `wasm-pack`. The build instruction goes in a `build` script, rather than a
  `pack` script. We're open to alternatives!
