# Syntax highlighting

PRQL contains multiple grammar definitions to enable tools to highlight PRQL
code. These are all intended to provide as good an experience as the grammar
supports. Please raise any shortcomings in a GitHub issue.

The definitions are somewhat scattered around the codebase; this page serves as
an index.

- [Lezer](https://lezer.codemirror.net/) — used by CodeMirror editors. The PRQL
  file is at
  [`prql-lezer/README.me`](https://github.com/PRQL/prql/tree/main/prql-lezer/README.md).

- [Handlebars](https://handlebarsjs.com/) — currently duplicated:

  - The book:
    [`book/highlight-prql.js`](https://github.com/PRQL/prql/blob/main/book/highlight-prql.js)
  - The website (outside of the book & playground):
    [`website/themes/prql-theme/static/plugins/highlight/prql.js`](https://github.com/PRQL/prql/blob/main/book/highlight-prql.js)

- [Textmate](https://macromates.com/manual/en/language_grammars) — used by the
  VSCode Extension. It's in the `prql-vscode` repo in
  [`prql-vscode/syntaxes/prql.tmLanguage.json`](https://github.com/PRQL/prql-vscode/blob/main/syntaxes/prql.tmLanguage.json).

- [Monarch](https://microsoft.github.io/monaco-editor/monarch.html) — used by
  the Monaco editor, which we use for the Playground. The grammar is at
  [`playground/src/workbench/prql-syntax.js`](https://github.com/PRQL/prql/blob/main/playground/src/workbench/prql-syntax.js).

While the [pest](https://pest.rs/) grammar at
[`prql-compiler/src/parser/prql.pest`](https://github.com/PRQL/prql/blob/main/prql-compiler/src/parser/prql.pest)
isn't used for syntax highlighting, it's the arbiter of truth given it currently
powers the PRQL compiler.
