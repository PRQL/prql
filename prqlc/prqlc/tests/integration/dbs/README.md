# PRQL test-databases

Test PRQL queries against various SQL RDBMS.

## In-process DBs

To run tests against DuckDB & SQLite, no additional setup is required; simply
run:

```sh
cargo test --features=test-dbs
```

## External DBs

To run tests against external databases — currently Postgres, MySQL, SQL Server,
ClickHouse and GlareDB are tested — we use `docker compose`:

1. Run `docker compose up` (may take a while on the first time):

   ```sh
   cd prql-compiler/tests/integration
   docker compose up
   ```

2. Run the tests:

   ```sh
   cargo test --features=test-dbs-external --no-capture
   ```

   The `--no-capture` option is definitely not required but is practical to see
   all the dialects tested per query.

3. After you're done, stop the containers and remove local images and volumes:

   ```sh
   docker compose down -v --rmi local
   ```

Note: if you're on an M1 and your MSSQL docker container doesn't run, refer to
[this comment](https://github.com/microsoft/mssql-docker/issues/668#issuecomment-1436802153)

## Tested databases

Tests are by default run on all the DBs with `SupportLevel::Supported`.

If you also want to test on a DB that is not yet at this support level like
`MSSQL`, simply add `# mssql:test` on top of your query.\
If you want to ignore one of the supported DBs like `sqlite`, simply add `# sqlite:skip`
on top of your query.

## Data

Columns are renamed to `snake_case`, so Postgres and DuckDb don't struggle with
them.

For optimal accessibility, portability between databases and file size, all
tables are stored as CSV files. Their current size is 432kB, it could be gzip-ed
to 112kB, but that would require a preprocessing step before running
`cargo test`.

## Queries

For databases like ClickHouse, where the order of results is ambiguous, please
use `sort` for test queries to to guarantee the order of rows across DBs.

For example, instead of the following query:

```elm
from albums
```

Use a query including `sort`:

```elm
from albums
sort album_id
```

## Test organization

We follow the advice in
<https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html>.
