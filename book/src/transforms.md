# Transforms

PRQL queries are a pipeline of transformations ("transforms"), where each
transform takes the previous result and adjusts it in some way, before passing
it onto to the next transform.

Because PRQL focuses on modularity, we have far fewer transforms than SQL, each
one fulfilling a specific purpose. That's often referred to as "orthogonality".

These are the currently available transforms:

| Transform                                    | Purpose                                                             | SQL Equivalent              |
| -------------------------------------------- | ------------------------------------------------------------------- | --------------------------- |
| [**`from`**](./transforms/from.md)           | Starts from a table                                                 | `FROM`                      |
| [**`derive`**](./transforms/derive.md)       | Computes new columns                                                | `SELECT *, ... AS ...`      |
| [**`select`**](./transforms/select.md)       | Picks & computes columns                                            | `SELECT ... AS ...`         |
| [**`filter`**](./transforms/filter.md)       | Picks rows based on their values                                    | `WHERE`, `HAVING`,`QUALIFY` |
| [**`sort`**](./transforms/sort.md)           | Orders rows based on the values of columns                          | `ORDER BY`                  |
| [**`join`**](./transforms/join.md)           | Adds columns from another table, matching rows based on a condition | `JOIN`                      |
| [**`take`**](./transforms/take.md)           | Picks rows based on their position                                  | `TOP`, `LIMIT`, `OFFSET`    |
| [**`group`**](./transforms/group.md)         | Partitions rows into groups and applies a pipeline to each of them  | `GROUP BY`, `PARTITION BY`  |
| [**`aggregate`**](./transforms/aggregate.md) | Summarizes many rows into one row                                   | `SELECT foo(...)`           |
| [**`window`**](./transforms/window.md)       | Applies a pipeline to overlapping segments of rows                  | `OVER`, `ROWS`, `RANGE`     |
