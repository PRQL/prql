# DuckDB

There's a [DuckDB](https://duckdb.org/) community extension by
**[@ywelsch](https://github.com/ywelsch)** at
the DuckDB Community Extension Repository.

```sql
INSTALL prql FROM community;
LOAD prql;
-- Once the extension is loaded, you can write PRQL queries
from (read_csv 'https://raw.githubusercontent.com/PRQL/prql/0.8.0/prql-compiler/tests/integration/data/chinook/invoices.csv')
filter invoice_date >= @2009-02-01
take 5;
```

Check out the [extension's documentation](https://community-extensions.duckdb.org/extensions/prql.html) for more details.
