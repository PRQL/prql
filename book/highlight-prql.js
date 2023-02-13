// Syntax highlighting for PRQL.

// Keep consistent with
// https://github.com/PRQL/prql/blob/main/website/themes/prql-theme/static/highlight/prql.js
// TODO: can we import one from the other at build time?

// Inspired by [Pest's book](https://github.com/pest-parser/book)

// mdBook exposes a minified version of highlight.js, so the language
// definition objects below have abbreviated property names:
//     "b"  => begin
//     "e"  => end
//     "c"  => contains
//     "k"  => keywords
//     "cN" => className

// TODO:
// - Can we represent strings with the actual rule of >= 3 quotes?
// - Aliases seem a bit strong?
// - Can we represent the inner s & f string items?

formatting = function (hljs) {
  const TRANSFORMS = [
    "aggregate",
    "append",
    "derive",
    "filter",
    "from_text",
    "from",
    "group",
    "join",
    "select",
    "sort",
    "take",
    "union",
    "window",
  ];
  const BUILTIN_FUNCTIONS = ["switch", "in", "as"];
  const KEYWORDS = ["func", "let", "prql"];
  return {
    name: "PRQL",
    case_insensitive: true,
    keywords: {
      keyword: [...TRANSFORMS, ...BUILTIN_FUNCTIONS, ...KEYWORDS],
      literal: "false true null ",
    },
    contains: [
      hljs.COMMENT("#", "$"),
      {
        // named arg
        scope: "params",
        begin: /\w+\s*:/,
        end: "",
        relevance: 10,
      },
      // This seems much too strong at the moment, so disabling. I think ideally
      // we'd have it for aliases but not assigns.
      // {
      //   // assign
      //   scope: { 1: "variable" },
      //   match: [/\w+\s*/, /=[^=]/],
      //   relevance: 10,
      // },
      {
        // date
        scope: "string",
        match: /@(\d*|-|\.\d|:)+/,
        relevance: 10,
      },
      {
        // interval
        scope: "string",
        // Add more as needed
        match: /\d+(days|hours|minutes|seconds|milliseconds)/,
        relevance: 10,
      },
      {
        // interpolation strings: s-strings are variables and f-strings are
        // strings? (Though possibly that's too cute, open to adjusting)
        //
        scope: "variable",
        relevance: 10,
        variants: [
          {
            begin: '(s)"""',
            end: '"""',
          },
          {
            begin: '(s)"',
            end: '"',
          },
        ],
        contains: [
          // I tried having the `f` / `s` be marked differently, but I don't
          // think it's possible to have a subscope within the begin / end.
          {
            // I think `variable` is the right scope rather than defaulting to
            // white, but not 100% sure; using `subst` is suggested in the docs.
            scope: "variable",
            begin: /\{/,
            end: /\}/,
          },
        ],
      },
      {
        scope: "string",
        relevance: 10,
        variants: [
          {
            begin: '(f)"""',
            end: '"""',
          },
          {
            begin: '(f)"',
            end: '"',
          },
        ],
        contains: [
          {
            scope: "variable",
            begin: "f",
            end: '"',
            // excludesEnd: true,
          },
          // TODO: would be nice to have this be a different color, but I don't
          // think it's possible to have a subscope within the begin / end.
          // {
          //   scope: "punctuation",
          //   match: /{|}/,
          // },
          {
            scope: "variable",
            begin: /\{/,
            end: /\}/,
          },
        ],
      },
      {
        // normal string
        scope: "string",
        relevance: 10,
        variants: [
          // TODO: is there a way of encoding the actual rule here? Otherwise
          // we're just adding the variants we use...
          {
            begin: '"""""',
            end: '"""""',
          },
          {
            begin: '"""',
            end: '"""',
          },
          {
            begin: '"',
            end: '"',
          },
          {
            begin: "'",
            end: "'",
          },
        ],
      },
      { scope: "punctuation", match: /[\[\]{}(),]/ },
      {
        scope: "operator",
        match:
          /(>)|(<)|(==)|(\+)|(\-)|(\/)|(\*)|(!=)|(<=)|(>=)|(\band\b)|(\bor\b)/,
        relevance: 10,
      },
      {
        // number
        scope: "number",
        // Slightly modified from https://stackoverflow.com/a/23872060/3064736;
        // it requires a number after a decimal point, so ranges appear as
        // ranges.
        // We allow underscores, a bit more liberally than PRQL, which doesn't
        // allow them at the end (but that's difficult to express with
        // regex; contributions welcome).
        // We force a leading break, so that we don't highlight a
        // number in `foo_1`.
        match: /\b((\d[\d_]*(\.[\d_]+])?)|(\.[\d_]+))/,
        relevance: 10,
      },
      {
        // range
        scope: "symbol",
        match: /\.{2}/,
        relevance: 10,
      },
      // Unfortunately this just overrides any keywords. It's also not
      // complete — it only handles functions at the beginning of a line.
      // I spent several hours trying to get hljs to handle this, but
      // because there's no recursion, I'm not sure it's possible.
      // Possibly we could hook into `on:begin` and implement it
      // ourselves, but this would be a lot of overhead.
      // { // function
      //     keywords: TRANSFORMS.join(' '),
      //     beginScope: { 1: 'title.function' },
      //     begin: [/^\s*[a-zA-Z]+/, /(\s+[a-zA-Z]+)+/],
      //     relevance: 10
      // },
    ],
  };
};

hljs.registerLanguage("prql", formatting);
hljs.registerLanguage("prql_no_test", formatting);
hljs.registerLanguage("elm", formatting);

// These lines should only exists in the book, not the website.

// This file is inserted after the default highlight.js invocation, which tags
// unknown-language blocks with CSS classes but doesn't highlight them.
Array.from(document.querySelectorAll("code.language-prql")).forEach(
  (a) => console.log(a) || hljs.highlightBlock(a)
);

Array.from(document.querySelectorAll("code.language-prql_no_test")).forEach(
  (a) => console.log(a) || hljs.highlightBlock(a)
);

Array.from(document.querySelectorAll("code.language-elm")).forEach(
  (a) => console.log(a) || hljs.highlightBlock(a)
);
