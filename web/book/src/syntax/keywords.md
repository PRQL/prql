# Keywords

At the moment, PRQL uses only four keywords:

- `let`
- `into`
- `case`
- `prql`
- `type`
- `module`

To use these names as columns or relations, use backticks: `` `case` ``.

It may seem that transforms are also keywords, but they are normal function
within std namespace:

```prql
std.from my_table
std.select [from = my_table.a, take = my_table.b]
std.take 3
```
