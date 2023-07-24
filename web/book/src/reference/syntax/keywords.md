# Identifiers & keywords

Identifiers can contain alphanumeric characters and `_` and must not start with
a number. They can be chained together with the `.` indirection operator, used
to retrieve a tuple from a field or a variable from a module. .

```prql no-eval
hello

_h3llo

hello.world
```

## Quoting

To use characters that would be otherwise invalid, identifiers can be surrounded
by with backticks.

When compiling to SQL, these identifiers will use dialect-specific quotes and
quoting rules.

```prql
prql target:sql.mysql
from employees
select `first name`
```

```prql
prql target:sql.postgres
from employees
select `first name`
```

```prql
prql target:sql.bigquery

from `project-foo.dataset.table`
join `project-bar.dataset.table` (==col_bax)
```

## Schemas & database names

Identifiers of database tables can be prefixed with schema and databases names.

```prql
from my_database.chinook.albums
```

Note that all of following identifiers will be treated as separate table
definitions: `tracks`, `public.tracks`, `my_database.public.tracks`.

## Keywords

PRQL uses following keywords:

- `prql` - query header
- `let` - variable definition
- `into` - variable definition
- `case` - flow control
- `type` - type declaration
- `func` - function declaration
- `module` - used internally
- `internal` - used internally
- `true` - [literal](./literals.md)
- `false` - [literal](./literals.md)
- `null` - [literal](./literals.md)

Keywords can be used as identifiers (of columns or variables) when encased in
backticks: `` `case` ``.

It may seem that transforms are also keywords, but they are normal functions
within std namespace:

```prql
std.from my_table
std.select {from = my_table.a, take = my_table.b}
std.take 3
```
