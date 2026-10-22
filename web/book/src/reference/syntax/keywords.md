# Identifiers & keywords

Identifiers can contain alphanumeric characters and `_` and must not start with
a number. They can be chained together with the `.` lookup operator, used to
retrieve a tuple from a field or a variable from a module.

```prql no-eval
hello

_h3llo

hello.world
```

## `this` & `that`

`this` refers to the current relation:

```prql
from invoices
aggregate (
    count this
)
```

Within a [`join`](../stdlib/transforms/join.md), `that` refers to the other
table:

```prql
from invoices
join tracks (this.track_id==that.id)
```

`this` can also be used to remove any column ambiguity. For example, currently
using a bare `time` as a column name will fail, because it's also a type:

```prql error no-fmt
from invoices
derive t = time
```

But with `this.time`, we can remove the ambiguity:

```prql
from invoices
derive t = this.time
```

## Wildcards (`.*`)

A relation name followed by `.*` refers to all of that relation's columns:

```prql
from employees
select {employees.*}
```

If the compiler knows a relation's columns — for example, after a `select` — a
wildcard expands to those columns. Otherwise, such as for a table read directly
with `from`, it compiles to `*` (or `alias.*` for an aliased relation).

Wildcards can be used wherever a tuple of columns is expected, including in
[`select`](../stdlib/transforms/select.md),
[`select !{...}`](../stdlib/transforms/select.md#excluding-columns),
[`group`](../stdlib/transforms/group.md) and the
[tuple functions](../stdlib/tuple.md).

A wildcard is useful after a join, to keep every column from one side:

```prql
from e = employees
join d = departments (==department_id)
select {e.name, d.*}
```

Wildcards also work with [`this`](#this--that): `this.*` refers to all columns
of the current relation, including columns computed earlier in the pipeline:

```prql
from employees
select {first_name, salary}
derive bonus = salary * 0.1
select (tuple_reverse this.*)
```

Here, `bonus` is included even though it's derived after the `select`.

<!-- prettier-ignore -->
> [!NOTE]
> `that.*` can't be used to select columns, because `that` only exists
> inside a `join` condition. After a join, refer to the other relation by
> name, as in `d.*` above.

## Quoting

To use characters that would be otherwise invalid, identifiers can be surrounded
with backticks.

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

Note that all of the following identifiers will be treated as separate table
definitions: `tracks`, `public.tracks`, `my_database.public.tracks`.

## Keywords

PRQL uses following keywords:

- **`prql`** - query header [_more..._](../../project/target.md)
- **`let`** - variable definition [_more..._](../declarations/variables.md)
- **`into`** - variable definition [_more..._](../declarations/variables.md)
- **`case`** - flow control [_more..._](../syntax/case.md)
- **`type`** - type declaration
- **`enum`** - enumeration type declaration
- **`func`** - explicit function declaration
  [_more..._](../declarations/functions.md)
- **`module`** - used internally
- **`import`** - used internally
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
std.from my_table
std.select {from = my_table.a, take = my_table.b}
std.take 3
```
