# Relations

PRQL is designed on top of _relational algebra_, which is the established data
model used by modern SQL databases.
A _relation_ has a rigid mathematical definition,
which can be simplified to "a table of data".
For example, the `invoices` table from the Chinook database[^1] looks like this:

| invoice_id | customer_id | billing_city | _other columns_ | total |
| ---------- | ------------ | ------------ | :-----------: | ----- |
| 1        |  2 | Stuttgart | ...  | 1.98 |
| 2        |  4 | Oslo      | ...        | 3.96 |
| 3        |  8 | Brussels  | ...        | 5.94 |
| 4        | 14 | Edmonton  | ...         | 8.91 |
| 5        | 23 | Boston    | ...         | 13.86 |
| 6        | 37 | Frankfurt | ...         | 0.99 |

A table (relation) is composed of columns, each of which has an unique name and a designated data type.
Every table has zero to many rows, each containing the same set of columns.
The table above has `invoice_id` and `customer_id` columns with a data type of "integer number",
a `billing_city` column with a data type of "text",
a number of other columns, and
and a `total` column that contains floating-point number. [^2]

## Queries

PRQL is designed to build queries that combine and select data from relations such as the `invoices` table above. Here is the most basic query:

```
from invoices
```

_Note: You can try each of these examples here in the [Playground.](https://prql-lang.org/playground/)
Enter the query on the left-hand side,
and click **output.arrow** in the right-hand side to see the result._

The result of the query above is not terribly interesting, it's just the same table as before.
But PRQL can do more...

**derive:** PRQL can also _add_ columns to a table with the `derive` statement.
Let's define a new column for Value Added Tax, set at 19% of the invoice total.
_(Try it in the [Playground.](https://prql-lang.org/playground/))_

```
from invoices
derive { total * 0.19 }
```

<!-- todo: make sure that the new column is unnamed -->

The value of the new column can be a constant (such as a number or a string),
or can be computed from the value of an existing column.
Note that the name of the new column is the expression that defined it.
To give that column an interesting name,
assign the expression to a name `value_added_tax`.

```
from invoices
derive { value_added_tax = total * 0.19 }
```

**select:** PRQL can also _remove_ columns from a table.
The `select` statement passes along only the columns named in a list
and discards all others.
Formally, that list is a _tuple_ of comma-separated expressions wrapped in `{ ... }`.
You can write the items in the tuple on one or several lines:
trailing commas are ignored.
Finally, you can assign any of the expressions to a _variable_
that becomes the name of the resulting column in the SQL output.

Suppose you only need the `order_id`, `total`, and `value_added_tax`.
Use `select` to choose the columns to pass through:

```
from invoices
derive { value_added_tax = total * 0.19 }
select {
  OrderID = invoice_id,
  Total = total,
  VAT = value_added_tax,
}
```

Once you `select` certain columns, subsequent transforms will have access only to those columns named in the tuple.

**join:** The `join` transform also _adds_ columns to a table.
This transform combines rows from two tables "side by side" to make them into one table.
To determine which rows from each table should be joined, `join` has match criteria in `( ... )`. [^3]

```
from invoices
join customers ( ==customer_id )
```

This example "connects" the customer information (from the `customers` table) with the information from the `invoices` table, using the `customer_id` column from each table to match the rows.

**Summary:** PRQL deals with tables (relations) of data.
You can just add new lines at the end of the query
to transform the table produced by the statement above.
Appending new transforms to a query extends the pipeline.

The `derive`, `select`, and `join` transforms change the number of columns in a table.
The first two never affect the number of rows in a table.
`join` may change the number of rows, depending on the variation chosen. (See footnote 3 below)

[^1]: Chinook is sample database with (fake) data in tables and interesting relations between them. There are many versions of the Chinook database on the web, for example, [https://github.com/lerocha/chinook-database](https://github.com/lerocha/chinook-database). PRQL uses the data from... _Where does our data come from? Do we use some canonical version?_

[^2]: _Is the following true? Is this the best place to discuss it?_ Most ~~definitions~~ databases use unordered relations, which cannot have duplicate rows. PRQL defines relations to have an order and therefore can contain duplicate rows.

[^3]: There are a number of variations for the `join` transform that guide how the coumns are matched: see the discussion of SQL Inner, Outer, Left, Right, and Cross joins, and ??? for how PRQL handles them.
