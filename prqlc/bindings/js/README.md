# prql-js

JavaScript bindings for [`prqlc`](https://github.com/PRQL/prql/).

## Installation

```sh
npm install prql-js
```

## Usage

Currently these functions are exposed

```typescript
function compile(prql_query: string, options?: CompileOptions): string;

function prql_to_pl(prql_query: string): string;

function pl_to_rq(pl_json: string): string;

function rq_to_sql(rq_json: string): string;
```

### From Node.js

Direct usage

```javascript
const prqljs = require("prql-js");

const sql = prqljs.compile(`from db.employees | select first_name`);
console.log(sql);
```

Options

```javascript
const opts = new prql.CompileOptions();
opts.target = "sql.mssql";
opts.format = false;
opts.signature_comment = false;

const sql = prqljs.compile(`from db.employees | take 10`, opts);
console.log(sql);
```

Template literal

```javascript
const prqljs = require("prql-js");
const prql = (string) => prqljs.compile(string[0] || "");

const sql = prql`from db.employees | select first_name`;
console.log(sql);
```

Template literal with newlines

```javascript
const prqljs = require("prql-js");
const prql = (string) => prqljs.compile(string[0] || "");

const sql = prql`
    from db.employees
    select first_name
`;
console.log(sql);
```

### From a browser

```html
<html>
  <head>
    <script type="module">
      import init, { compile } from './dist/web/prql_js.js';
      await init();

      const sql = compile("from db.employees | select first_name");
      console.log(sql);
    </script>
  </head>

  <body></body>
</html>
```

### From a framework or a bundler

```typescript
import compile from "prql-js/dist/bundler";

const sql = compile(`from db.employees | select first_name`);
console.log(sql);
```

## Errors

Errors are returned as following object, serialized as a JSON array:

```typescript
interface ErrorMessage {
  /// Message kind. Currently only Error is implemented.
  kind: "Error" | "Warning" | "Lint";
  /// Machine-readable identifier of the error
  code: string | null;
  /// Plain text of the error
  reason: string;
  /// A list of suggestions of how to fix the error
  hint: string | null;
  /// Character offset of error origin within a source file
  span: [number, number] | null;

  /// Annotated code, containing cause and hints.
  display: string | null;
  /// Line and column number of error origin within a source file
  location: SourceLocation | null;
}

/// Location within the source file.
/// Tuples contain:
/// - line number (0-based),
/// - column number within that line (0-based),
interface SourceLocation {
  start: [number, number];

  end: [number, number];
}
```

These errors can be caught as such:

```javascript
try {
  const sql = prqlJs.compile(`from db.employees | foo first_name`);
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

By default the `wasm` binaries are optimized on each run, even if the underlying
code hasn't changed, which can be slow. For a lower-latency dev loop, pass
`--profile=dev` to `npm install` for a faster, less optimized build.

```sh
npm install prql-js --profile=dev
```

## Notes

- This uses [`wasm-pack`](https://rustwasm.github.io/docs/wasm-pack/) to
  generate bindings[^1].
- We've added an `npm` layer on top of the usual approach of just using
  `wasm-pack`, so we can distribute a single package with targets of `node`,
  `bundler` and `no-modules` — somewhat inverting the approach recommended by
  `wasm-pack`. The build instruction goes in a `build` script, rather than a
  `pack` script.

[^1]:
    Though we would be very open to other approaches, given wasm-pack does not
    seem maintained, and we're eliding many of its features to build for three
    targets. See <https://github.com/PRQL/prql/issues/1836> for more details.
