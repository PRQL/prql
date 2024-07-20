# Date functions

These are all the functions defined in the `date` module:

### `to_text`

Converts a date into a text.\
Since there are many possible date representations, `to_text` takes a `format` parameter
that describes thanks to [specifiers](#date--time-format-specifiers) how the date
or timestamp should be structured.

```admonish info
Since all RDBMS have different ways to format dates and times, PRQL **requires an explicit dialect** to be specified
```

```admonish info
For now the supported DBs are: Clickhouse, DuckDB, MySQL, MSSQL and Postgres.
```

```prql
prql target:sql.duckdb

from invoices
select (invoice_date | date.to_text "%d/%m/%Y")

```

```prql
prql target:sql.postgres

from invoices
select (invoice_date | date.to_text "%d/%m/%Y")

```

```prql
prql target:sql.mysql

from invoices
select (invoice_date | date.to_text "%d/%m/%Y")

```

### Date & time format specifiers

PRQL specifiers for date and time formatting is a subset of specifiers used by
[`chrono`](https://docs.rs/chrono/latest/chrono/format/strftime/index.html).

Here is the list of the specifiers currently supported:

| Spec. | Example                       | Description                                                      |
| ----- | ----------------------------- | ---------------------------------------------------------------- |
|       |                               |                                                                  |
|       |                               | **DATE SPECIFIERS:**                                             |
| `%Y`  | `2001`                        | Year number, zero-padded to 4 digits                             |
| `%y`  | `01`                          | Year number, zero-padded to 2 digits                             |
| `%m`  | `07`                          | Month number (01–12), zero-padded to 2 digits                    |
| `%-m` | `7`                           | Month number (1-12)                                              |
| `%b`  | `Jul`                         | Abbreviated month name. Always 3 letters.                        |
| `%B`  | `July`                        | Full month name                                                  |
| `%d`  | `08`                          | Day number (01-31), zero-padded to 2 digits                      |
| `%-d` | ` 8`                          | Day number (1-31)                                                |
| `%a`  | `Sun`                         | Abbreviated weekday name. Always 3 letters                       |
| `%A`  | `Sunday`                      | Full weekday name                                                |
| `%D`  | `07/08/01`                    | Month-day-year format. Same as `%m/%d/%y`                        |
| `%x`  | `07/08/01`                    | Locale's date representation                                     |
| `%F`  | `2001-07-08`                  | Year-month-day format (ISO 8601). Same as `%Y-%m-%d`             |
|       |                               |                                                                  |
|       |                               | **TIME SPECIFIERS:**                                             |
| `%H`  | `00`                          | Hour number (00-23)                                              |
| `%k`  | ` 0`                          | Same as `%H` but space-padded. Same as `%_H`.                    |
| `%I`  | `12`                          | Hour number in 12-hour clocks (01--12), zero-padded to 2 digits. |
| `%p`  | `AM`                          | `AM` or `PM` in 12-hour clocks.                                  |
| `%M`  | `34`                          | Minute number (00-59), zero-padded to 2 digits.                  |
| `%S`  | `60`                          | Second number (00-59), zero-padded to 2 digits.                  |
| `%f`  | `264900`                      | Number of microseconds[^1] since last whole second               |
| `%R`  | `00:34`                       | Hour-minute format. Same as `%H:%M`.                             |
| `%T`  | `00:34:60`                    | Hour-minute-second format. Same as `%H:%M:%S`.                   |
| `%X`  | `00:34:60`                    | Locale's time representation (e.g., 23:13:48).                   |
| `%r`  | `12:34:60 AM`                 | Locale's 12 hour clock time. (e.g., 11:11:04 PM)                 |
|       |                               |                                                                  |
|       |                               | **DATE & TIME SPECIFIERS:**                                      |
| `%+`  | `2001-07-08T00:34:60.026490Z` | ISO 8601 / RFC 3339 date & time format.                          |
|       |                               |                                                                  |
|       |                               | **SPECIAL SPECIFIERS:**                                          |
| `%t`  |                               | Literal tab (`\t`).                                              |
| `%n`  |                               | Literal newline (`\n`).                                          |
| `%%`  |                               | Literal percent sign.                                            |

[^1]: This is different from chrono, for which `%f` represents nanoseconds
