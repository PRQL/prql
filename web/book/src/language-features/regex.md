# Regex expressions

```admonish note
This is currently experimental
```

To perform a case-sensitive regex search, use the `~=` operator. This generally
compiles to `REGEXP`, though differs by dialect more than most functions. A
regex search means that to match an exact value, the start and end need to be
anchored with `^foo$`.

```prql
from tracks
filter (name ~= "Love")
```

```prql
prql target:sql.duckdb

from artists
filter (name ~= "Love.*You")
```

```prql
prql target:sql.bigquery

from tracks
filter (name ~= "\\bLove\\b")
```

```prql
prql target:sql.postgres

from tracks
filter (name ~= "\\(I Can't Help\\) Falling")
```

```prql
prql target:sql.mysql

from tracks
filter (name ~= "With You")
```

```prql
prql target:sql.sqlite

from tracks
filter (name ~= "But Why Isn't Your Syntax More Similar\\?")
```
