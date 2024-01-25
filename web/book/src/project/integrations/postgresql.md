# PostgreSQL

PL/PRQL is a PostgreSQL extension that lets you write functions with PRQL. 

PL/PRQL functions serve as intermediaries, compiling the user's PRQL code into SQL statements that PostgreSQL executes. The extension is based on the [pgrx](https://github.com/pgcentralfoundation/pgrx) for developing PostgreSQL extensions in Rust. This framework manages the interaction with PostgreSQL's internal APIs, type conversions, and other function hooks necessary to integrate PRQL with PostgreSQL.


## Examples
PL/PRQL functions are defined using the `plprql` language specifier:
```sql
create function match_stats(int) returns table(player text, kd_ratio float) as $$
  from matches
  filter match_id == $1
  group player (
    aggregate {
      total_kills = sum kills,
      total_deaths = sum deaths
    }
  )
  filter total_deaths > 0
  derive kd_ratio = total_kills / total_deaths
  select { player, kd_ratio }
$$ language plprql;

select * from match_stats(1001)
    
 player  | kd_ratio 
---------+----------
 Player1 |    0.625
 Player2 |      1.6
(2 rows)
```

You can also run PRQL directly with the `prql` function which is useful for custom SQL in ORMs:
 
```sql
select prql('from matches | filter player == ''Player1''', 'player1_cursor');

fetch 2 from player1_cursor;

 id | match_id | round | player  | kills | deaths 
----+----------+-------+---------+-------+--------
  1 |     1001 |     1 | Player1 |     4 |      1
  3 |     1001 |     2 | Player1 |     1 |      7
(2 rows)
```

## Getting Started
For installation instructions and more information on the extension, see the [PL/PRQL repository](https://github.com/kaspermarstal/plprql).
