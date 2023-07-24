# Aggregation

A key feature of analytics is reducing many values down to some summary. This
act is called aggregation and always includes an aggregation function, for
example `average` or `sum`:

<!-- aggregating ids does not make much sense, should we switch to some other table? -->

```
from albums
aggregate {sum_of_ids = sum album_id}
```

`aggregate` is quite similar to `derive` and `select` as it is creating a new
column, with the option to assign it a name. Just as select, it will discard all
columns that are not present in the tuple, but with a key difference: it will
always produce a single row.

Just as `select` (and actually `derive` too), `aggregate` takes a tuple of
aggregation expressions, which means that you can produce multiple summaries at
once:

```
from albums
aggregate {
    sum_of_ids = sum album_id,
    mean_id = average album_id,
}
```

Here we've also split the tuple over multiple lines to make it more readable.
Obviously, you can do this with `select` and `derive` too.

## Grouping

Say that now we want to produce summaries of albums _for each artist
separately_. We cannot do this, as `aggregate` will always produce a single row.

What we can do, is separate the relation into groups corresponding to individual
artists, and apply `aggregate` to each one of them:

```
from albums
filter artist_id == 1
aggregate {sum_of_ids = sum album_id}
```

```
from albums
filter artist_id == 2
aggregate {sum_of_ids = sum album_id}
```

```
from albums
filter artist_id == 3
aggregate {sum_of_ids = sum album_id}
```

... and so on. Of course this is not the way to go, because it is way too
repetitive and we need to type all of the artist ids by hand. Luckily, `group`
will do exactly what we want: separate relation into groups, apply a function to
each of the groups and combine the results back together:

```
from albums
group artist_id (
    aggregate {sum_of_ids = sum album_id}
)
```

People who know how this is done in SQL would probably noticed that we just
decoupled aggregation from grouping. These two very connected operations benefit
immensely from each being a standalone function. Firstly, this allows each of
them to have some invariants that the query engine can leverage to produce more
efficient query planes. Additionally, they can be used with other transform
functions:

```
from albums
group artist_id (
    take 2
)
```

We can derive what this code chunk is doing using the same logic use used
before: `group` splits the relation into chunks each of which is corresponding
to some `artist_id`. Then it applies function `take 2`, which will take first
two rows of the chunk, and at the end it combines the rows back together.

SQL needed to replicate this behavior includes window functions and multiple
sub-queries. Some dialects (PostgreSQL, DuckDB, Google BigQuery) have a special
syntax to improve performance and reduce the query complexity with
`DISTINCT ON`. This approach only works when we have `take 1` instead of
`take 2`, so it's not a general case solution.
