# Transforms

Transforms are functions that take a relation and produce a relation.

Usually they are chained together into a pipeline, which resembles an SQL query.

Transforms were designed with a focus on modularity, so each of them is
fulfilling a specific purpose and has defined invariants (properties of the
relation that are left unaffected). That's often referred to as "orthogonality"
and is key to keep the number of transforms low.

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
