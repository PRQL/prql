# How do I: read files?

There are a couple of functions designed for DuckDB:

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

- [How do I: create ad-hoc relations?](./relation-literals.md)
