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

```prql_no_test
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
join d = department [==dept_no]
select [e.name, gross_salary, d.name]
```

## See also

- [Transforms](../transforms/README.md) - PRQL Transforms
- [Syntax](../syntax.md) - Notation for PRQL queries
