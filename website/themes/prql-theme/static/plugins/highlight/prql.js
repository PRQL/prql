// !!keep consistent with
// https://github.com/PRQL/prql/blob/main/reference/highlight-prql.js
//
// TODO: can we import one from the other at build time?

formatting = function (hljs) {
  const TRANSFORMS = [
    "from",
    "select",
    "derive",
    "filter",
    "take",
    "sort",
    "join",
    "aggregate",
    "group",
    "window",
    "append",
    "union",
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
            // scope: "title.function",
            scope: "variable",
            begin: "f",
            end: '"',
            // excludesEnd: true,
          },
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
      {
        // number
        scope: "number",
        // Slightly modified from https://stackoverflow.com/a/23872060/3064736;
        // it requires a number after a decimal point, so ranges appear as
        // ranges.
        // We disallow a leading word character, so that we don't highlight
        // a number in `foo_1`,
        // We allow underscores, a bit more liberally than PRQL, which doesn't
        // allow them at the start or end (but that's difficult to express with
        // regex; contributions welcome).
        match: /[+-]?[^\w](([\d_]+(\.[\d_]+])?)|(\.[\d_]+))/,
        relevance: 10,
      },
      {
        // range
        scope: "symbol",
        match: /\.{2}/,
        relevance: 10,
      },
      {
        // operator
        scope: "operator",
        match:
          /(>)|(<)|(==)|(\+)|(\-)|(!=)|(<=)|(>=)|(\?\?)|(\band\b)|(\bor\b)/,
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

// This line should only exists in the website, not the book.

hljs.highlightAll();
