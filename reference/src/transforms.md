# Transforms

PRQL queries are a pipeline of transformations ("transforms"), where each transform takes
the previous result and adjusts it in some way, before passing it onto to the
next transform.

Because PRQL focuses on modularity, we have far fewer transforms than SQL, each
one fulfilling a specific purpose. That's often referred to as "orthogonality".

These are the currently available transforms:
<!-- Copied from `SUMMARY.md` -->
| Transform                   | Purpose                                                             |
| --------------------------- | ------------------------------------------------------------------- |
| [Derive](./derive.md)       | Computes new columns                                                |
| [Select](./select.md)       | Picks & computes columns                                            |
| [Filter](./filter.md)       | Picks rows based on their values                                    |
| [Sort](./sort.md)           | Orders rows based on the values of columns                          |
| [Join](./join.md)           | Adds columns from another table, matching rows based on a condition |
| [Take](./take.md)           | Picks rows based on their position                                  |
| [Group](./group.md)         | Partitions rows into groups and applies a pipeline to each of them  |
| [Aggregate](./aggregate.md) | Summarizes many rows into one row                                   |
| [Window](./window.md)       | Applies a pipeline to overlapping segments of rows                  |
