# Strings

Strings in PRQL can use either single or double quotes:

```prql
derive x: "hello world"
```

```prql
derive x: 'hello world'
```

To quote a string containing quotes, either use the "other" type of quote, or
use three-or-more quotes, and close with the same number.

```prql
derive x: '"hello world"'
```

```prql
derive x: """I said "hello world"!"""
```

Currently PRQL does not adjust escape characters during the compilation process.

Currently PRQL allows multiline strings with either a single character or
multiple character quotes. This may change for strings using a single character
quote in future versions.
