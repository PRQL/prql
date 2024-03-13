# S-strings

An s-string inserts SQL directly, as an escape hatch when there's something that
PRQL doesn't yet implement. For example, there's a `version()` function in
PostgreSQL that returns the PostgreSQL version, so if we want to use that, we
use an s-string:

```prql
from db.my_table
select db_version = s"version()"
```

Embed a column name in an s-string using braces. For example, PRQL's standard
library defines the `average` function as:

```prql no-eval
let average = column -> s"AVG({column})"
```

So this compiles using the function:

```prql
from db.employees
aggregate {average salary}
```

```admonish note
Because S-string contents are SQL, double-quotes (`"`) will denote a _column name_.
To avoid that, use single-quotes (`'`) around the SQL string, and
adjust the quotes of the S-string. For example, instead of `s'CONCAT("hello", "world")'` use `s"CONCAT('hello', 'world')"`
```

Here's an example of a more involved use of an s-string:

```prql
from db.dept_emp
select {de = this}
join (db.salaries | select {s = this}) side:left (s.emp_no == de.emp_no && s"""
  ({s.from_date}, {s.to_date})
  OVERLAPS
  ({de.from_date}, {de.to_date})
""")
```

For those who have used Python, s-strings are similar to Python's f-strings, but
the result is SQL code, rather than a string literal. For example, a Python
f-string of `f"average({col})"` would produce `"average(salary)"`, with quotes;
while in PRQL, `s"average({col})"` produces `average(salary)`, without quotes.

Note that interpolations can only contain plain variable names and not whole
expression like Python.

We can also use s-strings to produce a full table:

```prql
s"SELECT DISTINCT ON first_name, id, age FROM employees ORDER BY age ASC"
join s"SELECT * FROM salaries" (==id)
```

```admonish note
S-strings in user code are intended as an escape hatch for an unimplemented
feature. If we often need s-strings to express something, that's a sign we
should implement it in PRQL or PRQL's stdlib. If you often require an s-string,
[submit an issue with your use case](https://github.com/PRQL/prql/issues/new/choose).
```

## Braces

To output braces from an s-string, use double braces:

```prql
from db.employees
derive {
  has_valid_title = s"regexp_contains(title, '([a-z0-9]*-){{2,}}')"
}
```

## Precedence within s-strings

Variables in s-strings are inserted into the SQL source as-is, which means we
may get surprising behavior when the variable has multiple terms and the
s-string isn't parenthesized.

In this toy example, the expression `salary + benefits / 365` gets precedence
wrong. The generated SQL code is as if we had written
`salary + (benefits / 365)`.

```prql
from db.employees
derive {
  gross_salary = salary + benefits,
  daily_rate = s"{gross_salary} / 365"
}
```

Instead, the numerator `{gross_salary}` must be encased in parentheses:

```prql
from db.employees
derive {
  gross_salary = salary + benefits,
  daily_rate = s"({gross_salary}) / 365"
}
```
