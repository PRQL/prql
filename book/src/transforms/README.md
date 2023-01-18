# Transforms

PRQL queries are a pipeline of transformations ("transforms"), where each
transform takes the previous result and adjusts it in some way, before passing
it onto to the next transform.

Because PRQL focuses on modularity, we have far fewer transforms than SQL, each
one fulfilling a specific purpose. That's often referred to as "orthogonality".

These are the currently available transforms:

| Transform   | Purpose                                                                                     | SQL Equivalent              |
| ----------- | ------------------------------------------------------------------------------------------- | --------------------------- |
| `from`      | [Starts from a table](./transforms/from.md)                                                 | `FROM`                      |
| `derive`    | [Computes new columns](./transforms/derive.md)                                              | `SELECT *, ... AS ...`      |
| `select`    | [Picks & computes columns](./transforms/select.md)                                          | `SELECT ... AS ...`         |
| `filter`    | [Picks rows based on their values](./transforms/filter.md)                                  | `WHERE`, `HAVING`,`QUALIFY` |
| `sort`      | [Orders rows based on the values of columns](./transforms/sort.md)                          | `ORDER BY`                  |
| `join`      | [Adds columns from another table, matching rows based on a condition](./transforms/join.md) | `JOIN`                      |
| `take`      | [Picks rows based on their position](./transforms/take.md)                                  | `TOP`, `LIMIT`, `OFFSET`    |
| `group`     | [Partitions rows into groups and applies a pipeline to each of them](./transforms/group.md) | `GROUP BY`, `PARTITION BY`  |
| `aggregate` | [Summarizes many rows into one row](./transforms/aggregate.md)                              | `SELECT foo(...)`           |
| `window`    | [Applies a pipeline to overlapping segments of rows](./transforms/window.md)                | `OVER`, `ROWS`, `RANGE`     |
