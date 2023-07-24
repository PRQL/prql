# Filtering rows

In previous page we learned how `derive` and `select` can used to create new
columns and remove unwanted ones.

Now we will look into how we can manipulate rows using `filter` and `take`.

> Side note: each of the transform functions has at least some invariant: select
> and derive will not change the number of rows, filter and take will not change
> the number of columns. We call this separation of effects "transform
> orthogonality". It's goal is to keep transform functions composable by
> minimizing interference of their effects.
>
> In SQL, you can see this lack of invariant when an aggregation function is
> used in the `SELECT` clause. Before, the number of rows was kept constant, but
> introduction of an aggregation function caused the whole statement to produce
> only one row (per group).

Filter pick rows based on their values:

```
from albums
filter artist_id == 2
```

Here we are looking for rows corresponding to albums which were produced by
artist with id 2.

In SQL, `filter` would be expressed with either `WHERE` or `HAVING`, depending
on where in the pipeline it is used.

---

Take picks rows based on their position within the relation. The position can be
specified in two ways:

- a plain number `x`, which will pick first `x` rows, or
- an inclusive range of rows `start..end`.

```
from albums
take 4
```

```
from albums
take 4..7
```

Of course, it is possible combine all of what we learned into a single pipeline:

```
from albums

# add a new column
derive my_column = artist_id + 1

# retain only rows that have my_column equal to 91
filter my_column == 91

# skip first 10 rows and take only the next 10
take 11..20

# take only first 5 rows
take 5
```

We did something strange at the end here: first we've taken rows `11..20` and
then taken first 5 rows within that range. This could be done in a single
transform `take 11..15`. When compiled to SQL this two ways of expressing the
same thing actually produce an identical query. This is a nice example of how
PRQL allows fast data exploration, optimizing your queries so you have the
freedom to stack transforms on top of each other, without worrying about
interactions of a new transform with the previous query.
