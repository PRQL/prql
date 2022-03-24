# Functions

- Functions can take two disjoint types of arguments:
  1. Positional arguments, which are required.
  2. Named arguments, which are optional and have a default value.
- So a function like:

  ```elm
  func lag col sort_col by_col=id = (
    window col
    by by_col
    sort sort_col
    lag 1
  )
  ```

  ...is called `lag`, takes three arguments `col`, `sort_col` & `by_col`, of
  which the first two much be supplied, the third can optionally be supplied
  with `by_col:sec_id`.
