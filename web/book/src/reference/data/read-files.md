# Reading files

There are a few functions mainly designed for DuckDB to read from files:

```prql
prql target:sql.duckdb

from a = (read_parquet "artists.parquet")
join b = (read_csv "albums.csv") (a.artist_id == b.artist_id)
join c = (read_json "metadata.json") (a.artist_id == c.artist_id)
```

> [!NOTE] These don't currently have all the DuckDB options. If those would be
> helpful, please log an issue and it's a fairly easy addition.

> [!NOTE] We may be able to reduce the boilerplate
> `WITH table_x AS SELECT * FROM...` in future versions.

When specifying file names directly in the `FROM` clause without using
functions, which is allowed in DuckDB, enclose the file names in backticks
` `` ` as follows:

```prql
from `artists.parquet`
```

## See also

- [Target and Version](../../project/target.md)
- [Ad-hoc data](./relation-literals.md)
