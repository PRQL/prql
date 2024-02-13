# Identifiers & keywords

Identifiers can contain alphanumeric characters and `_` and must not start with
a number. They can be chained together with the `.` indirection operator, used
to retrieve a tuple from a field or a variable from a module.

```prql no-eval
hello

_h3llo

hello.world
```

## `this` & `that`

`this` refers to the current relation:

```prql
from.invoices
aggregate (
    count this
)
```

Within a [`join`](../stdlib/transforms/join.md), `that` refers to the other
table:

```prql
from.invoices
join from.tracks (this.track_id==that.id)
```

`this` can also be used to remove any column ambiguity. For example, currently
using a bare `time` as a column name will fail, because it's also a type:

```prql error no-fmt
from.invoices
derive t = time
```

But with `this.time`, we can remove the ambiguity:

```prql
from.invoices
derive t = this.time
```

## Quoting

To use characters that would be otherwise invalid, identifiers can be surrounded
by with backticks.

When compiling to SQL, these identifiers will use dialect-specific quotes and
quoting rules.

```prql
prql target:sql.mysql

from.employees
select `first name`
```

```prql
prql target:sql.postgres

from.employees
select `first name`
```

```prql
prql target:sql.bigquery

from.`project-foo.dataset.table`
join from.`project-bar.dataset.table` (==col_bax)
```

## Schemas & database names

Identifiers of database tables can be prefixed with schema and databases names.

```prql
from.my_database.chinook.albums
```

Note that all of following identifiers will be treated as separate table
definitions: `tracks`, `public.tracks`, `my_database.public.tracks`.

## Keywords

PRQL uses following keywords:

- **`prql`** - query header [_more..._](../../project/target.md)
- **`let`** - variable definition [_more..._](../declarations/variables.md)
- **`into`** - variable definition [_more..._](../declarations/variables.md)
- **`case`** - flow control [_more..._](../syntax/case.md)
- **`type`** - type declaration
- **`func`** - explicit function declaration
  [_more..._](../declarations/functions.md)
- **`module`** - used internally
- **`internal`** - used internally
- **`true`** - boolean [_more..._](./literals.md#booleans)
- **`false`** - boolean [_more..._](./literals.md#booleans)
- **`null`** - NULL [_more..._](./literals.md#null)

Keywords can be used as identifiers (of columns or variables) when encased in
backticks: `` `case` ``.

Transforms are normal functions within the `std` namespace, not keywords. That
is, `std.from` is the same function as `from`. In the example below, the
resulting query is the same as without the `std.` namespace:

```prql
from.my_table
std.select {from = my_table.a, take = my_table.b}
std.take 3
```
