// Syntax highlighting for PRQL.

// Inspired by [Pest's book](https://github.com/pest-parser/book)

// mdBook exposes a minified version of highlight.js, so the language
// definition objects below have abbreviated property names:
//     "b"  => begin
//     "e"  => end
//     "c"  => contains
//     "k"  => keywords
//     "cN" => className

hljs.registerLanguage("prql", function(hljs) {
    return {
        case_insensitive: false,
        keywords: {
            keyword: 'from select derive filter take sort join aggregate func',
            literal: 'false true null',
        },
        contains: [
            hljs.COMMENT('#', '$'),

            { // named arg
                className: 'params',
                begin: '\\w+:(?!\\s)',
                end: '',
                relevance: 10
            },
            { // s-string
                className: 'attribute',
                begin: 's"', end: '"',
                relevance: 10
            },
            { // normal string
                className: 'string',
                begin: '"', end: '"',
                relevance: 10
            },

        ]
    };

});

// This file is inserted after the default highlight.js invocation, which tags
// unknown-language blocks with CSS classes but doesn't highlight them.
Array.from(document.querySelectorAll("code.language-prql"))
    .forEach((a) => console.log(a) || hljs.highlightBlock(a));
