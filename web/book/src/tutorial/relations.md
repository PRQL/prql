# Relations

PRQL is designed on top of relational algebra, which is the an established data
model used by modern databases. _Relation_ has a rigid mathematical definition,
which can be simplified down to "tabular data". See table of `albums` for
example:

| album_id | title               | artist_id |
| -------- | ------------------- | --------- |
| 1        | For Those About ... | 1         |
| 2        | Balls to the Wall   | 2         |
| 3        | Restless and Wild   | 2         |
| 4        | Let There Be Rock   | 1         |
| 5        | Big Ones            | 3         |
| 6        | Jagged Little Pill  | 4         |

It is composed of columns, each of which has an unique name and a designated
data type. `album_id` has a data type of "integer numbers" and `title` has data
type of "text".

> Side note: most definitions use unordered relations, which cannot have
> duplicate rows. PRQL defines relations to have an order and to be able to
> contain duplicate rows.

PRQL is designed to query relations such as the `albums` table above. The most
basic query would be:

```
from albums
```

This is not impressive, it is just the same table as we had before. But we can
add on to it:

```
from albums
derive artist_id + 1
```

<!-- todo: make sure that the new column is unnamed -->

`derive` adds a new column to the relation, computed from other columns. Note
that the new column is unnamed. If we wanted to have it named, we could say
something like:

```
from albums
derive my_column = artist_id + 1
```

Now let's say that we don't need the original `artist_id` column and would want
to remove it. We can use select:

```
from albums
derive my_column = artist_id + 1
select {album_id, title, my_column}
```

`select` will take a tuple of columns and discard all others. Note that it can
also contain expressions, similar to derive:

```
from albums
derive my_column = artist_id + 1
select {album_id, title, my_column}
select {album_id, title, col_2 = my_column + 1}
```

Notice how we assigned a name to the new column _within_ the select tuple.

We have just added new lines at the end of the query, which might look strange.
Each of the lines of the query is a transformation of the result given by the
query above. This means that we can just append new transforms at the bottom of
a query and continue the pipeline.

> Side note: if you `select` early in the pipeline, subsequent transforms will
> have access only to the `select`ed columns.
