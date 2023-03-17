# From text

It's often useful to make a small table inline, for example when exploring how a
database will evaluate an expression, or to have a small lookup table inline.
This can be quite verbose in SQL.

PRQL uses `from_text` for this.

It accepts a few formats:

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
derive [
    d = b + c,
    answer = 20 * 2 + 2,
]
```

An example of adding a small lookup table:

```prql
let temp_format_lookup = from_text format:csv """
country_code,format
uk,C
us,F
lr,F
de,C
"""

from temperatures
join temp_format_lookup [==country_code]
```

And JSON:

```prql
let x = from_text format:json """{
    "columns": ["a", "b", "c"],
    "data": [
        [1, "x", false],
        [4, "y", null]
    ]
}"""

let y = from_text format:json """
    [
        {"a": 1, "m": "5"},
        {"a": 4, "n": "6"}
    ]
"""

from x | join y [==a]
```
