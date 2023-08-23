# Transforms

Transforms are functions that take a relation and produce a relation.

Usually they are chained together into a pipeline, which resembles an SQL query.

Transforms were designed with a focus on modularity, so each of them is
fulfilling a specific purpose and has defined invariants (properties of the
relation that are left unaffected). That's often referred to as "orthogonality"
and its goal is to keep transform functions composable by minimizing
interference of their effects. Additionally, it also keeps the number of
transforms low.

For example, `select` and `derive` will not change the number of rows, while
`filter` and `take` will not change the number of columns.

In SQL, we can see this lack of invariant when an aggregation function is used
in the `SELECT` clause. Before, the number of rows was kept constant, but
introduction of an aggregation function caused the whole statement to produce
only one row (per group).

These are the currently available transforms:

| Transform   | Purpose                                                                         | SQL Equivalent              |
| ----------- | ------------------------------------------------------------------------------- | --------------------------- |
| `from`      | [Start from a table](./from.md)                                                 | `FROM`                      |
| `derive`    | [Compute new columns](./derive.md)                                              | `SELECT *, ... AS ...`      |
| `select`    | [Pick & compute columns](./select.md)                                           | `SELECT ... AS ...`         |
| `filter`    | [Pick rows based on their values](./filter.md)                                  | `WHERE`, `HAVING`,`QUALIFY` |
| `sort`      | [Order rows based on the values of columns](./sort.md)                          | `ORDER BY`                  |
| `join`      | [Add columns from another table, matching rows based on a condition](./join.md) | `JOIN`                      |
| `take`      | [Pick rows based on their position](./take.md)                                  | `TOP`, `LIMIT`, `OFFSET`    |
| `group`     | [Partition rows into groups and applies a pipeline to each of them](./group.md) | `GROUP BY`, `PARTITION BY`  |
| `aggregate` | [Summarize many rows into one row](./aggregate.md)                              | `SELECT foo(...)`           |
| `window`    | [Apply a pipeline to overlapping segments of rows](./window.md)                 | `OVER`, `ROWS`, `RANGE`     |
| `loop`      | [Iteratively apply a function to a relation until it's empty](./loop.md)        | `WITH RECURSIVE ...`        |
