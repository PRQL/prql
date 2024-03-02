# Reading files

There are a couple of functions mainly designed for DuckDB to read from files:

```prql
prql target:sql.duckdb

from (read_parquet "artists.parquet")
join (read_csv "albums.csv") (==track_id)
```

```admonish note
These don't currently have all the DuckDB options. If those would be helpful,
please log an issue and it's a fairly easy addition.
```

```admonish info
We may be able to reduce the boilerplate `WITH table_x AS SELECT * FROM...` in future versions.
```

When specifying file names directly in the `FROM` clause without using
functions, which is allowed in DuckDB, enclose the file names in backticks
` `` ` as follows:

```prql
from `artists.parquet`
```

## See also

- [Target and Version](../../project/target.md)
- [Ad-hoc data](./relation-literals.md)
