# PRQL

**P**oint-f**R**ee **Q**uery **L**anguage, pronounced "Prequel".

PRQL is a language for transforming data. Like SQL, it's readable, explicit and
declarative. Unlike SQL, it forms a logical pipeline of transformations, and
supports abstractions such as variables and functions. It can be used with any
database that uses SQL, since it transpiles to SQL.

## Variables

Here's a fairly simple SQL query:

```sql
SELECT TOP 20
    title,
    country,
    AVG(salary) AS average_salary,
    SUM(salary) AS sum_salary,
    AVG(salary + payroll_tax) AS average_gross_salary,
    SUM(salary + payroll_tax) AS sum_gross_salary,
    AVG(salary + payroll_tax + healthcare_cost) AS average_gross_cost,
    SUM(salary + payroll_tax + healthcare_cost) AS sum_gross_cost,
    COUNT(*)
FROM employees
WHERE salary + payroll_tax + healthcare_cost > 0 AND country = 'USA'
GROUP BY title, country
ORDER BY sum_gross_cost
```

Even a simple query demonstrates some of the problems with SQL's lack of
abstractions — we needlessly repeat the calculation for each measure multiple
times, when each derives from the previous measure — and again in the `WHERE`
clause. The syntax is also awkward — when developing the query, commenting out
the final line of the `SELECT` list causes a syntax error, and we need to repeat
the columns in the `GROUP BY` clause in the `SELECT` list.

Here's the same query with PRQL:

```prql
employees
filter country = 'USA'                         # Each line transforms the previous result.
gross_salary = salary + payroll_tax            # This _adds_ a column to the result with a variable.
gross_cost   = gross_salary + healthcare_cost  # Variable can use other variables.
filter gross_cost > 0
group_by [title, country] [
    average salary,
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum     gross_cost,
    count,
]
sort sum_gross_cost                            # Uses the auto-generated column name.
head 20
```

As well as using variables to reduce unnecessary repetition, the query is also
more readable — it flows from top to bottom, each line representing a
transformation of the previous line's result. For example, `TOP 20` and `head
20` both modify the final result — but only PRQL represents it as the final
transformation.

## Functions

Here's another SQL query, which calculates returns from prices on days with
valid prices.

```sql
SELECT
  date,
  -- Can't use a `WHERE` clause, as it would affect the row that the `LAG` function referenced.
  IF(is_valid_price, price_adjusted / LAG(price_adjusted, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) AS return_total,
  IF(is_valid_price, price_adjusted_usd / LAG(price_adjusted_usd, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) AS return_usd,
  IF(is_valid_price, price_adjusted / LAG(price_adjusted, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) 
    - interest_rate / 252 AS return_excess,
  IF(is_valid_price, price_adjusted_usd / LAG(price_adjusted_usd, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) 
    - interest_rate / 252 AS return_usd_excess
FROM prices
```

> This might seem like a convoluted example, but it's taken from a real query.
> Indeed, it's also simpler and smaller than the full logic — note that it
> starts from `price_adjusted`, whose logic had to be split into a previous
> query to avoid the SQL becoming even less readable.

Here's the same query with PRQL:

```prql

func lag x = window x group_by:sec_id sort:date lag:1
func ret x = x / (x | lag) - 1 + cash_dividend_return
func excess x = (x - interest_rate) / 252    
func if_valid x = is_valid_price ? x : null

prices
return_total       = prices_adj   | ret | if_valid    # `|` can be used rather than newlines.
return_usd         = prices_usd   | ret | if_valid
return_excess      = return_total | excess
return_usd_excess  = return_usd   | excess
select [
  date,
  sec_id,
  return_total,
  return_usd,
  return_excess,
  return_usd_excess,
]
```

Because we define the functions once rather than copying & pasting the code, we
get all the benefits of encapsulation and extensibility — we can have reliable &
tested functions, whose purpose is explicit, which we can share across queries
and colleagues.

## TODOs

- Write a basic parser
  - Currently writing it using `nom`.
- Write a basic complier
  - This should be fairly easy since it's just generating SQL.
- Demonstrate some more complicated examples — e.g. most of the examples in
  <https://github.com/dbt-labs/dbt-utils> could all be covered much better by
  this.
- Show how this can build arbitrarily nested data, using the `map` & tabs a bit
  like in our Julia macros (but without the `do` & `end`); this could also be a
  clearer syntax for more substantial `jq`-like transformations.:

  ```julia
  @p begin
    text
    strip
    split(__, "\n")
    map() do __
        collect
        map() do __
          __ == chars[begin] ? 1 : 0
        end
    end
    hcat(__...)'
  end
  ```

## Thinking about

- Partials — potentially we don't need the `col` in `lag`?

  ```
  func lag col = window col group_by:sec_id sort:date lag:1
  ```

- Potentially `lag:1` should instead be passed as a function rather than an
  optional arg, like (even though in SQL you can't pass any function as a window
  func):

  ```
  func lag col = window col group_by:sec_id sort:date '(lag 1)
  ```

- Lists
  - Currently lists require brackets; there's no implicit list like:

    ```
    employees
    select salary  # fails, would require `select [salary]`
    ```

  - For some functions where we're only expecting a single arg, like `select`,
    we could accept that.

- Line breaks
  - Currently a line break always creates a piped transformation outside of a list.
    For example:

    ```
    tbl
    select [
      col1,
      col2,
    ]
    filter col1 = col2
    ```

    ...is equivalent to:

    ```
    tbl | select [col1, col2] | filter col1 = col2
    ```

- Functions' final argument is the result of the previous function; i.e.
  `group_by` would be like:

  ```
  group_by grouping_cols calc_cols X
  ```

- CTE syntax — something like `table =`?
- Raw syntax — I think we should have backticks represent raw SQL; i.e. `UPPER`
  could be defined as:

  ```prql
  func upper col = `UPPER(`col`)`
  # or with f-string-like syntax
  func upper col = `UPPER({col})`
  # or with " rather than `
  func upper col = "UPPER({col})"
  ```

## References

- <https://github.com/tobymao/sqlglot>
- Lots of SQL parsers exist, but we need a SQL _writer_
- <https://github.com/sqlparser-rs/sqlparser-rs>
