# How do I: create ad-hoc relations?

It's often useful to make a small inline relation, for example when exploring
how a database will evaluate an expression, or for a small lookup table. This
can be quite verbose in SQL.

PRQL offers two approaches — array literals, and a `from_text` transform.

## Array literals

Because relations (aka a table) in PRQL are just arrays of tuples, they can be
expressed with array and tuple syntax:

```prql
from [
  {a=5, b=false},
  {a=6, b=true},
]
filter b == true
select a
```

```prql
let my_artists = [
  {artist="Miles Davis"},
  {artist="Marvin Gaye"},
  {artist="James Brown"},
]

from db.artists
join my_artists (==artist)
join db.albums (==artist_id)
select {artists.artist_id, albums.title}
```

## `from_text`

`from_text` takes a string in a common format, and converts it to table. It
accepts a few formats:

- `format:csv` parses CSV (default),

- `format:json` parses either:

  - an array of objects each of which represents a row, or

  - an object with fields `columns` & `data`, where `columns` take an array of
    column names and `data` takes an array of arrays.

```prql
from_text """
a,b,c
1,2,3
4,5,6
"""
derive {
    d = b + c,
    answer = 20 * 2 + 2,
}
```

```prql
from_text format:json """
[
    {"a": 1, "m": "5"},
    {"a": 4, "n": "6"}
]
"""
```

```prql
from_text format:json """
{
    "columns": ["a", "b", "c"],
    "data": [
        [1, "x", false],
        [4, "y", null]
    ]
}
"""
```

## See also

- [Read files?](./read-files.md)
