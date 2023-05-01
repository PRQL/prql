# Regex expressions

```admonish note
This is currently experimental
```

To perform a regex search, use the `~=` operator. This generally compiles to
`REGEXP`, though it's heavily dialect-dependent.

```prql
from artists
filter (name ~= "Martin")
```

```prql
prql target:sql.duckdb

from artists
filter (name ~= "Martin")
```

```prql
prql target:sql.postgres

from artists
filter (name ~= "Martin")
```
