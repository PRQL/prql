# Relations

PRQL is designed on top of _relational algebra_, which is the established data
model used by modern SQL databases.
A _relation_ has a rigid mathematical definition,
which can be simplified to "a table of data".
For example, the `invoices` table from the Chinook database ([https://github.com/lerocha/chinook-database](https://github.com/lerocha/chinook-database)) looks like this:

| invoice_id | customer_id | billing_city | _other columns_ | total |
| ---------- | ------------ | ------------ | :-----------: | ----- |
| 1        |  2 | Stuttgart | ...  | 1.98 |
| 2        |  4 | Oslo      | ...        | 3.96 |
| 3        |  8 | Brussels  | ...        | 5.94 |
| 4        | 14 | Edmonton  | ...         | 8.91 |
| 5        | 23 | Boston    | ...         | 13.86 |
| 6        | 37 | Frankfurt | ...         | 0.99 |

A table (relation) is composed of columns, each of which has an unique name and a designated data type.
Every table has zero to several rows, each containing the same set of columns.
The table above has
`invoice_id`, `customer_id`, and `artist_id` columns with a data type of "integer number",
a `billing_city` column with a data type of "text",
a number of other columns, and
and a `total` column that contains floating-point numbers. [^2]

## Queries

PRQL is designed to build queries that combine and select data from relations such as the `invoices` table above. Here is the most basic query:

```
from invoices
```

_Note: Try each of these examples here in the [Playground.](https://prql-lang.org/playground/)
Enter the query on the left-hand side,
and click **output.arrow** in the right-hand side to see the result._

The result of the query above is not terribly interesting, it's just the same table as before.
But PRQL can do more...

### `select` transform

The PRQL `select` statement picks the columns to pass through based on a list
and discards all others.
Formally, that list is a _tuple_ of comma-separated expressions wrapped in `{ ... }`.

Suppose we only need the `order_id`, `total` columns.
Use `select` to choose the columns to pass through.
_(Try it in the [Playground.](https://prql-lang.org/playground/))_

```
from invoices
select { order_id, total }
```

We can write the items in the tuple on one or several lines:
trailing commas are ignored.
In addition, we can assign any of the expressions to a _variable_
that becomes the name of the resulting column in the SQL output.

```
from invoices
select {
  OrderID = invoice_id,
  Total = total,
}
```
This is the same query as above, rewritten on multiple lines,
and assigning `OrderID` and `Total` names to the columns.

Once we `select` certain columns, subsequent transforms will have access only to those columns named in the tuple.

### `derive` transform

PRQL can also add columns to a table with the `derive` statement.
Let's define a new column for Value Added Tax, set at 19% of the invoice total.

```
from invoices
derive { VAT = total * 0.19 }
```

<!-- todo: make sure that the new column is unnamed -->

The value of the new column can be a constant (such as a number or a string),
or can be computed from the value of an existing column.
Note that the value of the new column is assigned the name `VAT`.

### `join` transform

The `join` transform also adds columns to a table by combining the
rows from two tables "side by side".
To determine which rows from each table should be joined, `join` has match criteria, written in `( ... )`. [^3]

```
from invoices
join customers ( ==customer_id )
```

This example "connects" the customer information from the `customers` table with the information from the `invoices` table, using identical values of the `customer_id` column from each table to match the rows.

It is frequently useful to assign an alias to both tables being joined together
so that each table's columns can be referred to uniquely.

```
from inv=invoices
join cust=customers ( ==customer_id )
```

In the example above, the alias `inv` represents the `invoices` table and `cust` represents the `customers` table. It then becomes possible to refer to `inv.billing_city` and `cust.last_name` unambiguously.

### Summary
PRQL manipulates tables (relations) of data.
The `derive`, `select`, and `join` transforms change the number of columns in a table.
The first two never affect the number of rows in a table.
`join` may change the number of rows, depending on the variation chosen. (See footnote 3 below)

This final example combines the above into a single query.
It illustrates _a pipeline_ &mdash; the fundamental basis of PRQL.
We simply add new lines (transforms) at the end of the query.
Each transform modifies the table produced by the statement above
to produce the desired result.

```
from inv=invoices
join cust=customers (==customer_id)
derive { VAT = inv.total * 0.19 }
select {
  OrderID = inv.invoice_id,
  CustomerName = cust.last_name,
  Total = inv.total,
  VAT,
}
```


### Move to Reference?

_The following items might be better moved to the Reference section_

[^1]: Chinook is sample database with (fake) data in tables and interesting relations between them. There are many versions of the Chinook database on the web, for example, [https://github.com/lerocha/chinook-database](https://github.com/lerocha/chinook-database). PRQL uses the data from... _Where does our data come from? Do we use some canonical version?_

[^2]: _Is the following true? Is this the best place to discuss it?_ Most ~~definitions~~ databases use unordered relations, which cannot have duplicate rows. PRQL defines relations to have an order and therefore can contain duplicate rows.

[^3]: There are a number of variations for the `join` transform that guide how the coumns are matched: see the discussion of SQL Inner, Outer, Left, Right, and Cross joins, and ??? for how PRQL handles them.
