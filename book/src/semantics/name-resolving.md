# Name resolving

Because PRQL primarily handles relational data, it has specialized scoping rules for referencing columns.

## Scopes

In PRQL's compiler, a scope is the collection of all names one can reference from a specific point in the program.

In PRQL, names in the scope are composed from namespace and variable name which are separated by a dot, similar to SQL.
Namespaces can contain many dots, but variable names cannot.

```admonish example
Name `my_table.some_column` is a variable `some_column` from namespace `my_table`.

Name `foo.bar.baz` is a variable `baz` from namespace `foo.bar`.
```

When processing a query, a scope is maintained and updated for each point in the query.

It start with only namespace `std`, which is the standard library.
It contains common functions like `sum` or `count`,
along with all transform functions such as `derive` and `group`.

In pipelines (or rather in transform functions),
scope is also injected with namespaces of tables which may have been referenced with `from` or `join` transforms.
These namespaces contain simply all the columns of the table and possibly a wildcard variable,
which matches any variable (see the algorithm below).
Within transforms, there is also a special namespace that does not have a name.
It is called a _"frame"_ and it contains columns of the current table the transform is operating on.

## Resolving

For each ident we want to resolve, we search the scope's items in order. One of three things can happen:

- Scope contains an exact match, e.g. a name that matches in namespace and the variable name.

- Scope does not contain an exact match,
  but the ident did not specify a namespace, so we can match a namespace that contains a `*` wildcard.
  If there's a single namespace, the matched namespace is also updated to contain this new variable name.

  In the case that there are multiple namespaces with a wildcard,
  we don't match with neither of the namespaces, but match as `*.*` name.
  This is a special feature that allows a column name which may reside in two different tables
  not to be associated with any of them.
  Instead, it is translated into the column name only, so database can determine which table it belongs to.
  Note that this may lead to PRQL passing on ambigous queries to the database, instead of resulting error early.

- Otherwise, the nothing is matched and an error is raised.

## Translating to SQL

When translating into a SQL statement which references only one table,
there is no need to reference column names with table prefix.

```prql
from employees
select first_name
```

But when there are multiple tables and we don't have complete knowledge of all table columns,
a column without a prefix (i.e. `first_name`) may actually reside in multiple tables.
Because of this, we have to use table prefixes for all column names.

```prql
from employees
derive [first_name, dept_id]
join d=departments [dept_id]
select [first_name, d.title, created_at]
```

As you can see, `employees.first_name` now needs table prefix, to prevent conficts with potential column with the same name in `departments` table.
Similarly, `d.title` needs the table prefix.

But `created_at` has triggered the special rule and matched `*.*`,
because it may reside in any of the two tables.
This means that PRQL does not associate it with any table and the column is translated as is.
