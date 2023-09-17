# prql-lezer

A Lezer / CodeMirror grammar for PRQL. It's largely fully-functioning, with a
few small TODOs in the [grammar file](src/prql.grammar).

CodeMirror grammars are required by some downstream tools, including
[Jupyter syntax highlighting](https://github.com/PRQL/pyprql/issues/45). As of
2022-12 none yet use it.

We don't yet have the JS machinery around it, and it's not published to any
package managers. We can add that shortly. Possibly it'll go into its own repo.

## Developing

Given there aren't yet tests, we've been developing this by:

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
