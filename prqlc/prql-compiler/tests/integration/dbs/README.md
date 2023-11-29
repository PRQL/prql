# PRQL test-databases

Test PRQL queries against various SQL RDBMS.

## In-process DBs

To run tests against DuckDB & SQLite, no additional setup is required; simply
run:

```sh
cargo test --features=test-dbs
```

## External DBs

To run tests against external databases — currently Postgres, MySQL, SQL Server
and ClickHouse are tested — we use `docker compose`:

1. Run `docker compose up` (may take a while on the first time):

   ```sh
   cd prql-compiler/tests/integration
   docker compose up
   ```

2. Run the tests:

   ```sh
   cargo test --features=test-dbs-external
   ```

3. After you're done, stop the containers and remove local images and volumes:

   ```sh
   docker compose down -v --rmi local
   ```

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
