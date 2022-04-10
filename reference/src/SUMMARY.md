# Summary

- [Introduction](./introduction.md)
- [Queries](./queries/README.md)
  - [Pipelines](./queries/pipelines.md)
- [Verbs](./verbs.md)
- [Language features](./language-features/README.md)

  <!--   - f-strings — `CONCAT(foo, " ", bar)` -> `f"{foo} {bar}"`? -->
  - [String concat]()
    <!-- - Ranges — `BETWEEN 1 AND 3` -> `in 1..3`? -->
  - [Ranges]()
    <!-- - - Dates — `"2021-01-01"` -> `@2021-01-01`? And `DATE_TRUNC(foo_date, YEAR)` -> `foo_date.year`? Or -> `foo_date | as year`? Or `foo_date | to year`? -->
    <!-- - Offsets — `DATE_ADD(DATE "2008-12-25", INTERVAL 5 DAY)` -> `@2008-12-25 - 5day`? -->
  - [Dates]()
    <!--   - Regex — `REGEX_MATCH(foo, "\\w{3}")` -> `foo ~ r"\w{3}"`? Or -> `regex foo r"\w{3}"`? -->
  - [Regex]()
  - [S-Strings](./language-features/s-strings.md)

- [Transforms](./transforms.md)
- [Syntax](./syntax.md)
- [Functions](./functions.md)
- [Stdlib](./stdlib.md)
- [Live Editor](./editor.md)
