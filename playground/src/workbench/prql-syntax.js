const def = {
  // Set defaultToken to invalid to see what you do not tokenize yet
  // defaultToken: 'invalid',

  keywords: [
    "from",
    "select",
    "derive",
    "filter",
    "take",
    "sort",
    "join",
    "aggregate",
    "group",
    "func",
    "table",
    "and",
    "or",
    "not",
    "null",
    "true",
    "false",
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
      [/\d*\.\d+([eE][-+]?\d+)?/, "number.float"],
      [/\d+/, "number"],

      // strings
      [/"([^"\\]|\\.)*$/, "string.invalid"], // non-teminated string
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
