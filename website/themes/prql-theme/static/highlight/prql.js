// !!keep consistent with https://github.com/prql/prql/blob/main/reference/highlight-prql.js
hljs.registerLanguage('prql', function (hljs) {
    return {
        case_insensitive: false,
        keywords: {
            keyword: 'from select derive filter take sort join aggregate func group window',
            literal: 'false true null',
        },
        contains: [
            hljs.COMMENT('#', '$'),

            { // named arg
                className: 'params',
                begin: '\\w+\\s*:',
                end: '',
                relevance: 10
            },
            { // assign
                className: 'variable',
                begin: '\\w+\\s*=(?!=)',
                end: '',
                relevance: 10
            },
            { // date
                className: 'string',
                begin: '@',
                end: ' ',
                relevance: 10
            },
            { // interpolation string
                className: 'attribute',
                begin: '(s|f)"', end: '"',
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

hljs.highlightAll();