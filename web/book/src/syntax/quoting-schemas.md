# Quoting schemas

```admonish note
This is currently not great and we are working on improving it; see
https://github.com/PRQL/prql/issues/1535 for progress.
```

If supplying a schema without a column — for example in a `from` or `join`
transform, that also needs to be a quoted identifier:

```prql
from `music.albums`
```
