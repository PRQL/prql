# S-Strings

An s-string inserts SQL directly, as an escape hatch when there's something that
PRQL doesn't yet implement. For example, there's no `version()` function in SQL
that returns the Postgres version, so if we want to use that, we use an
s-string:

```prql
from my_table
select db_version = s"version()"
```

Embed a column name in an s-string using braces. For example, PRQL's standard
library defines the `average` function as:

```prql_no_test
func average column -> s"AVG({column})"
```

So this compiles using the function:

```prql
from employees
aggregate [average salary]
```

Here's an example of a more involved use of an s-string:

```prql
from de=dept_emp
join s=salaries side:left [
  (s.emp_no == de.emp_no),
  s"""({s.from_date}, {s.to_date})
  OVERLAPS
  ({de.from_date}, {de.to_date})"""
]
```

For those who have used python, s-strings are similar to python's f-strings, but
the result is SQL code, rather than a string literal. For example, a python
f-string of `f"average{col}"` would produce `"average(salary)"`, with quotes;
while in PRQL, `s"average{col}"` produces `average(salary)`, without quotes.

We can also use s-strings to produce a full table:

```prql
from s"SELECT DISTINCT ON first_name, id, age FROM employees ORDER BY age ASC"
join s = s"SELECT * FROM salaries" [==id]
```

```admonish note
S-strings in user code are intended as an escape-hatch for an unimplemented
feature. If we often need s-strings to express something, that's a sign we
should implement it in PRQL or PRQL's stdlib.
```

## Braces

To output braces from an s-string, use double braces:

```prql
from employees
derive [
  has_valid_title = s"regexp_contains(title, '([a-z0-9]*-){{2,}}')"
]
```

## Precedence

The PRQL compiler simply places a literal copy of each variable into the
resulting string, which means we may get surprising behavior when the variable
is has multiple terms and the s-string isn't parenthesized.

In this toy example, the `salary + benefits / 365` gets precedence wrong:

```prql
from employees
derive [
  gross_salary = salary + benefits,
  daily_rate = s"{gross_salary} / 365"
]
```

Instead, put the denominator `{gross_salary}` in parentheses:

```prql
from employees
derive [
  gross_salary = salary + benefits,
  daily_rate = s"({gross_salary}) / 365"
]
```
