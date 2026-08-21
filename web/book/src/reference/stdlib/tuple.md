# Tuple functions

The standard library defines the following functions for manipulating tuples:

### `tuple_reduce`

Applies a two-parameter function cumulatively across a tuple, in order to reduce
the tuple to a single value.

```prql
from invoices
derive {
  cleared = tuple_reduce std.and {
    received, processed, packed, shipped
  }
}
```

When the `initial:` named parameter is provided, its value will be used to
initialize the reduction operation, and will be the default value if the tuple
is empty.

```prql
from test
derive {
  mysum = tuple_reduce initial:0 add {1, 2, 3}
}
```

If `initial` is not provided, when the tuple has exactly one entry, its value
will be returned; when the tuple is empty, an error will be raised.

### `tuple_map`

Applies a function to each entry in a tuple, returning a tuple. Aliases defined
in the tuple will be passed through to the output.

```prql
prql target:sql.duckdb

from invoices
select (
  tuple_map (date.to_text "%d/%m/%Y") {
    rcv_txt = received_on,
    prc_txt = processed_on,
  }
)
```

### `tuple_zip`

Combines two tuples into one tuple by aligning them in parallel and creating
tuples out of each aligned pair. This can be used in conjunction with
`tuple_map` and `tuple_reduce` in various ways:

```prql
from invoices
derive (
  tuple_zip {x, y} {u, v}
  tuple_map (tuple_reduce (func a b -> a + b))
)
```

### `tuple_uniq`

Iteratively deduplicates a tuple by alias (or, when an alias is not defined, by
its referenced column name). This can help to have more control over situations
when a column may be being overwritten.

For example, the following includes all columns from both `invoices` and
`shipments` in the final result, even those that have overlapping names:

```prql
let shipments = (from shipments | select {id, invoice_id, date_of, shipped_on})

from invoices
select {id, date_of, processed}
join shipments (this.id == that.invoice_id)
```

Adding `select (tuple_uniq take:late {invoices.*, shipments.*})` will allow the
columns from `shipments` to appear in the output taking precedence over those
from `invoices`.

```prql
let shipments = (from shipments | select {id, invoice_id, date_of, shipped_on})

from invoices
select {id, date_of, processed}
join shipments (this.id == that.invoice_id)
select (tuple_uniq take:late {invoices.*, shipments.*})
```

Using `take:early` rather than `take:late` flips the priority.

```prql
let shipments = (from shipments | select {id, invoice_id, date_of, shipped_on})

from invoices
select {id, date_of, processed}
join shipments (this.id == that.invoice_id)
select (tuple_uniq take:early {invoices.*, shipments.*})
```

Items in a tuple without a name or an alias will be dropped.

```prql
from test
select (tuple_uniq {x, 5, y})
```

### `tuple_reverse`

Reverses the order of a tuple.

```prql
from test
select (tuple_reverse {x, y, z})
```
