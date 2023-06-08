# Reading files

We have a couple of functions named `read_*`, which ask the DB to read files,
designed for DuckDB:

```prql
from (read_parquet 'artists.parquet')
join (read_csv 'albums.csv') (==track_id)
```

```admonish note
These don't currently have all the DuckDB options. If those would be helpful,
please log an issue and it's a fairly easy addition.
```

```admonish info
We may be able to reduce the boilerplate `WITH table_x AS SELECT * FROM...` in future versions.
```

## See also

- [Relation literals](../relation-literals.md)
