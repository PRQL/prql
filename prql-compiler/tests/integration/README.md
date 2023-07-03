# PRQL test-databases

Test PRQL queries against various SQL RDBMS.

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

## RDMBS

### SQLite

Can be run as part of `cargo test`. Uses bundled sqlite, compiled as part of
cargo build.

### DuckDb

Can be run as part of `cargo test`. Uses bundled DuckDb, compiled as part of
cargo build.

### External DBs

These will not run as a part of `cargo test`. Use
`cargo test --features=test-dbs-external` instead. Make sure to start docker
compose before (see below).

Currently Postgres, MySQL, SQL Server and ClickHouse are tested.

## Docker compose

To test the external databases, docker needs to be installed.

Steps to run the tests:

1. Run docker compose (may take a while on the first time):

   ```sh
   cd prql-compiler/tests/integration
   docker compose up
   ```

2. Run the tests:

   ```sh
   cargo test --features=test-dbs-external
   ```

## Test organization

We follow the advice in
<https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html>.
