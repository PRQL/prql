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
  "concat",
  "union",
];
const BUILTIN_FUNCTIONS = ["switch"]; // "in", "as"
const KEYWORDS = ["func", "table", "prql"];
const LITERALS = ["null", "true", "false"];
const OPERATORS = ["and", "or"]; // "not"

const def = {
  // Set defaultToken to invalid to see what you do not tokenize yet
  // defaultToken: 'invalid',

  keywords: [
    ...TRANSFORMS,
    ...BUILTIN_FUNCTIONS,
    ...KEYWORDS,
    ...LITERALS,
    ...OPERATORS,
  ],

  operators: [
    "-",
    "*",
    "/",
    "%",
    "+",
    "-",
    "==",
    "!=",
    ">",
    "<",
    ">=",
    "<=",
    "??",
  ],

  // The main tokenizer for our languages
  tokenizer: {
    root: [
      // comments
      { include: "@comment" },

      // named-args
      [/(\w+)\s*:/, { cases: { $1: "key" } }],

      // identifiers and keywords
      [
        /[a-z_$][\w$]*/,
        { cases: { "@keywords": "keyword", "@default": "identifier" } },
      ],

      // whitespace
      { include: "@whitespace" },

      // delimiters
      [/[()[\]]/, "@brackets"],

      // numbers
      // Slightly modified from https://stackoverflow.com/a/23872060/3064736;
      // it requires a number after a decimal point, so ranges appear as
      // ranges.
      // We disallow a leading word character, so that we don't highlight
      // a number in `foo_1`,
      // We allow underscores, a bit more liberally than PRQL, which doesn't
      // allow them at the start or end (but that's difficult to express with
      // regex; contributions welcome).
      [/[+-]?[^\w](([\d_]+(\.[\d_]+])?)|(\.[\d_]+))/, "number"],

      // strings
      [/"([^"\\]|\\.)*$/, "string.invalid"], // non-terminated string
      [/"/, { token: "string.quote", bracket: "@open", next: "@string" }],

      // characters
      [/'[^\\']'/, "string"],
    ],

    comment: [[/#.*/, "comment"]],

    string: [
      [/[^\\"]+/, "string"],
      [/"/, { token: "string.quote", bracket: "@close", next: "@pop" }],
    ],

    whitespace: [
      [/[ \t\r\n]+/, "white"],
      [/\/\*/, "comment", "@comment"],
      [/\/\/.*$/, "comment"],
    ],
  },
};
export default def;
