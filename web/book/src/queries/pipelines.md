# Pipelines

PRQL queries are a sequence of lines (or _transforms_) that form a _pipeline_.
Each line transforms the data, and passes its result to the next.

**The simplest pipeline** is just:

```prql
from employees
```

## Adding transforms

As we add additional lines, each one transforms the result:

```prql
from employees
derive gross_salary = (salary + payroll_tax)
```

...and so on:

```prql no-eval
from employees
derive gross_salary = (salary + payroll_tax)
sort gross_salary
```

## Compiling to SQL

PRQL compiles the query to SQL. The PRQL compiler tries to represent as many
transforms as possible with a single `SELECT` statement. When necessary, the
compiler "overflows" and creates CTEs (common table expressions):

```prql
from e = employees
derive gross_salary = (salary + payroll_tax)
sort gross_salary
take 10
join d = department (==dept_no)
select {e.name, gross_salary, d.name}
```

## See also

<!-- markdown-link-check-disable -->
<!-- we're linking to README.md files as index.html to work around https://github.com/rust-lang/mdBook/issues/984 -->

- [Transforms](../transforms/index.html) - PRQL Transforms
- [Syntax](../syntax/index.html) - Notation for PRQL queries
