// !!keep consistent with
// https://github.com/PRQL/prql/blob/main/reference/highlight-prql.js
//
// TODO: can we import one from the other at build time?

// We should probably grab more from other languages at
// https://github.com/highlightjs/highlightjs-tsql/tree/main/src/languages.
// Possibly we can even import parts at runtime, simplifying this file?

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
  const BUILTIN_FUNCTIONS = ["case", "in", "as"];
  const KEYWORDS = ["let", "prql", "into"];
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
          /(>)|(<)|(==)|(\+)|(\-)|(\/)|(\*)|(!=)|(->)|(=>)|(<=)|(>=)|(&&)|(\|\|)/,
        relevance: 10,
      },
      {
        scope: "number",
        // Regex explanation:
        // 1. `\b`: asserts a word boundary. This ensures that the pattern matches numbers that are distinct words or at the boundaries of words.
        // 2. `(\d[_\d]*(e|E)\d[_\d]*)`: This is the first alternative in the main group and matches numbers in scientific notation:
        //     - `\d`: matches a digit (0-9).
        //     - `[_\d]*`: matches zero or more underscores or digits, representing the numbers before the `e` in scientific notation.
        //     - `(e|E)`: matches the letter 'e' or 'E' for scientific notation.
        //     - `\d`: matches a digit (0-9), the beginning of the exponent.
        //     - `[_\d]*`: matches zero or more underscores or digits, representing the numbers after the `e` in scientific notation.
        // 3. `(\d[_\d]*|(\d\.[\d_]*\d))`: This is the second alternative in the main group and matches standard numbers without the scientific notation:
        //     - `\d[_\d]*`: matches a sequence starting with a digit and followed by zero or more digits or underscores.
        //     - `|`: OR
        //     - `(\d\.[\d_]*\d)`: matches numbers with a decimal point:
        //         - `\d`: matches the digit(s) before the decimal point.
        //         - `\.`: matches the decimal point.
        //         - `[\d_]*\d`: matches digits after the decimal point, ensuring the sequence ends in a digit and not a trailing underscore.
        // 4. `(\.[\d_]+)`: This is the third alternative in the main group:
        //     - `\.`: matches a literal dot, so this alternative captures numbers that begin with a decimal point.
        //     - `[\d_]+`: matches one or more digits or underscores, for the sequence after the initial dot.
        match:
          /\b((\d[_\d]*(e|E)\d[_\d]*)|(\d[_\d]*|(\d\.[\d_]*\d))|(\.[\d_]+))/,
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

// This line should only exists in the website, not the book.

hljs.highlightAll();
