# Transforms

PRQL queries are a pipeline of transformations ("transforms"), where each
transform takes the previous result and adjusts it in some way, before passing
it onto to the next transform.

Because PRQL focuses on modularity, we have far fewer transforms than SQL, each
one fulfilling a specific purpose. That's often referred to as "orthogonality".

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
