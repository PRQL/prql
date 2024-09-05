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
ClickHouse and GlareDB are tested using `docker compose` to create the
databases.

The steps are all covered by `task test-rust-external-dbs`; to run them
manually:

1. Run `docker compose up` (may take a while on the first time):

   ```sh
   cd prqlc/prqlc/tests/integration/dbs && docker compose up -d
   ```

2. Run the tests:

   ```sh
   cargo test --features=test-dbs-external -- --nocapture
   ```

   (The `--no-capture` option isn't required, but shows all the dialects tested
   per query.)

3. After it's done, remove the containers:

   ```sh
   cd prqlc/prqlc/tests/integration/dbs && docker compose down
   ```

Note: on an M1, if the MSSQL docker container doesn't run, refer to
[this comment](https://github.com/microsoft/mssql-docker/issues/668#issuecomment-1436802153)

## Tested databases

Tests are by default run on all the DBs with `SupportLevel::Supported`.

To test on a DB that is not yet at this support level like `MSSQL`, simply add
`# mssql:test` on top of the query. To ignore one of the supported DBs like
`sqlite`, simply add `# sqlite:skip` on top of the query.

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
