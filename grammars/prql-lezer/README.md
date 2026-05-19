# prql-lezer

A Lezer / CodeMirror grammar for PRQL. It's largely fully-functioning, with a
few small TODOs in the [grammar file](src/prql.grammar).

CodeMirror grammars are required by some downstream tools, including
[Jupyter syntax highlighting](https://github.com/PRQL/pyprql/issues/45).

We don't yet have the JS machinery around it, and it's not published to any
package managers. We can add that shortly. Possibly it'll go into its own repo.

## Developing

Tests are in the `test/` directory. The Lezer playground can also be useful for
interactive development:

- Opening <https://lezer-playground.vercel.app/>
- Pasting an example query
- Pasting the current grammar
- Fixing any issues in the grammar
- Copying the grammar back into the repo

## Instructions

Install dependencies:

    npm install

Build:

    npm run build

Test:

    npm run test
