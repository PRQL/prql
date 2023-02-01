# PRQL test-databases

Test PRQL queries against various SQL RDBMS.

## Data

Database chinook.db was downloaded from
<https://www.sqlitetutorial.net/sqlite-sample-database>

Columns are renamed to snake_case, so Postgres and DuckDb don't struggle with
them.

For optimal accessibility, portability between databases and file size, all
tables are stored as CSV files. Their current size is 432kB, it could be gzip-ed
to 112kB, but that would require a preprocessing step before running
`cargo test`.

## RDMBS

### SQLite

Can be run as part of `cargo test`. Uses bundled sqlite, compiled as part of
cargo build.

### DuckDb

Can be run as part of `cargo test`. Uses bundled DuckDb, compiled as part of
cargo build.

### Postgres

If passed environmental variable `POSTGRES_HOST` this crate will requests
postgres server that should already have data loaded in.

This will not run as a part of `cargo test`, but can be run with docker-compose.

## Docker compose

There is also a proof on concept for testing done against Postgres, which can be
run by running `docker-compose up`. This will:

- build a docker image for Postgres (with data already loaded in)
- build a docker image for this crate (+data), compiled with --tests
- run the two images, executing the tests.

Steps to run the tests:

1. Compile the integration test:

   ```
   $ cargo build --test integration
   ```

2. Copy the test file to tests/integration dir:

   ```
   $ cp target/debug/deps/integration-xxxxx prql-compiler/tests/integration
   ```

3. Run docker compose (that will also build the image):
   ```
   $ cd prql-compiler/tests/integration
   $ docker-compose up
   ```

## Test organization

We follow the advice in
<https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html>.
