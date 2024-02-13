# Lutra

Query runner for PRQL (Pipelined Relational Query Language).

## Installation

`pip install lutra`

## Usage

```prql
# Project.prql

@(lutra.sqlite {file="chinook.db"})
module my_database {
    let artists <[{artist_id = int, name = text}]>
}

from.my_database.artists
select {artist_id, text}
into main
```

```python
import lutra
import pyarrow

result: pyarrow.RecordBatch = lutra.execute_one('.', 'main')
print(result)
```
