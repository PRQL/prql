# Date & time format specifiers

PRQL specifiers for date and time formatting is a subset of specifiers used by
[`chrono`](https://docs.rs/chrono/latest/chrono/format/strftime/index.html).

Here is the list of the specifiers currently supported:

| Spec. | Example                       | Description                                                                                                                    |
| ----- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
|       |                               |                                                                                                                                |
|       |                               | **DATE SPECIFIERS:**                                                                                                           |
| `%Y`  | `2001`                        | Year number, zero-padded to 4 digits                                                                                           |
| `%y`  | `01`                          | Year number, zero-padded to 2 digits                                                                                           |
| `%m`  | `07`                          | Month number (01–12), zero-padded to 2 digits                                                                                  |
| `%-m` | `7`                           | Month number (1-12)                                                                                                            |
| `%b`  | `Jul`                         | Abbreviated month name. Always 3 letters.                                                                                      |
| `%B`  | `July`                        | Full month name                                                                                                                |
| `%d`  | `08`                          | Day number (01-31), zero-padded to 2 digits                                                                                    |
| `%-d` | ` 8`                          | Day number (1-31)                                                                                                              |
| `%a`  | `Sun`                         | Abbreviated weekday name. Always 3 letters                                                                                     |
| `%A`  | `Sunday`                      | Full weekday name                                                                                                              |
| `%D`  | `07/08/01`                    | Month-day-year format. Same as `%m/%d/%y`                                                                                      |
| `%x`  | `07/08/01`                    | Locale's date representation                                                                                                   |
| `%F`  | `2001-07-08`                  | Year-month-day format (ISO 8601). Same as `%Y-%m-%d`                                                                           |
|       |                               |                                                                                                                                |
|       |                               | **TIME SPECIFIERS:**                                                                                                           |
| `%H`  | `00`                          | Hour number (00-23)                                                                                                            |
| `%k`  | ` 0`                          | Same as `%H` but space-padded. Same as `%_H`.                                                                                  |
| `%I`  | `12`                          | Hour number in 12-hour clocks (01--12), zero-padded to 2 digits.                                                               |
| `%p`  | `AM`                          | `AM` or `PM` in 12-hour clocks.                                                                                                |
| `%M`  | `34`                          | Minute number (00-59), zero-padded to 2 digits.                                                                                |
| `%S`  | `60`                          | Second number (00-59), zero-padded to 2 digits.                                                                                |
| `%f`  | `264900`                      | Number of microseconds since last whole second{footnote: this is different from chrono, for which `%f` represents nanoseconds} |
| `%R`  | `00:34`                       | Hour-minute format. Same as `%H:%M`.                                                                                           |
| `%T`  | `00:34:60`                    | Hour-minute-second format. Same as `%H:%M:%S`.                                                                                 |
| `%X`  | `00:34:60`                    | Locale's time representation (e.g., 23:13:48).                                                                                 |
| `%r`  | `12:34:60 AM`                 | Locale's 12 hour clock time. (e.g., 11:11:04 PM)                                                                               |
|       |                               |                                                                                                                                |
|       |                               | **DATE & TIME SPECIFIERS:**                                                                                                    |
| `%+`  | `2001-07-08T00:34:60.026490Z` | ISO 8601 / RFC 3339 date & time format.                                                                                        |
|       |                               |                                                                                                                                |
|       |                               | **SPECIAL SPECIFIERS:**                                                                                                        |
| `%t`  |                               | Literal tab (`\t`).                                                                                                            |
| `%n`  |                               | Literal newline (`\n`).                                                                                                        |
| `%%`  |                               | Literal percent sign.                                                                                                          |
