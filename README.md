# PRQL

Point-fRee Query Language, pronounced "Prequel".

PRQL is a language for transforming data. It can be used with any database that
uses SQL since it transpiles to SQL. Like SQL, it's readable, explicit and
declarative. Unlike SQL, it forms a logical pipeline of transformations, and
supports abstractions such as variables and functions.

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
WHERE salary + payroll_tax + healthcare_cost > 0
GROUP BY title, country
ORDER BY sum_gross_cost
```

Even a simple query demonstrates some of the problems with SQL's lack of
abstractions — we need to repeat the calculation for each measure multiple
times. The syntax is also awkward — when developing the query, it's not possible
to comment out the final line of the `SELECT` list, and we need to repeat the
columns in the `GROUP BY` clause.

Here's the same query with PRQL:

```prql
employees
gross_salary <- salary + payroll_tax            # Only needs to be defined once
gross_cost   <- gross_salary + healthcare_cost
filter gross_cost > 0
group_by [title, country] [
    average salary
    sum     salary
    average gross_salary
    sum     gross_salary
    average gross_cost
    sum     gross_cost
    count
]
sort sum_gross_cost
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
  IF(is_current_price, price_adjusted / LAG(price_adjusted, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) AS return_total,
  IF(is_current_price, price_adjusted_usd / LAG(price_adjusted_usd, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) AS return_usd,
  IF(is_current_price, price_adjusted / LAG(price_adjusted, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) 
    - interest_rate / 252 AS return_excess,
  IF(is_current_price, price_adjusted_usd / LAG(price_adjusted_usd, 1) OVER 
    (PARTITION BY sec_id ORDER BY date) - 1 + cash_dividend_return, NULL) 
    - interest_rate / 252 AS return_usd_excess
FROM prices
```

Here's the same query with PRQL:

```prql

lag col <- window col group_by:sec_id sort:date lag:1
ret col <- col / (col | lag) - 1 + cash_dividend_return
excess col <- col - interest_rate / 252
if_valid col <- if is_current_price col null

prices
return_total       <- prices_adjusted | ret | if_valid
return_usd         <- prices_usd      | ret | if_valid
return_excess      <- return_total | excess
return_usd_excess  <- return_usd   | excess
select [
  date
  sec_id
  return_total              
  return_usd                
  return_excess
  return_usd_excess
]
```

Because we define the functions once rather than copying & pasting the code,
we get all the benefits of encapsulation and extensibility — we can have
reliable, tested functions that we share across queries and colleagues.

## TODOs

- Write a basic parser
  - What should I use for this?
  - nom? pest? [lairpop](https://github.com/lalrpop/lalrpop)?
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

- Potentially we don't need the `col` in `lag`, since it would form a partial?
  But then maybe we need to declare functions with a keyword?

  ```
  lag col <- window col group_by:sec_id sort:date lag:1
  ```

- Potentially `lag:1` should instead be passed as a function rather than an
  optional arg, like (even though in SQL you can't pass any function as a window
  func):

  ```
  lag col <- window col group_by:sec_id sort:date '(lag 1)
  ```

- How would piping work with non-monadic functions? Probably by putting the
  implicit arg first or last; i.e `group_by` (shown above) would be defined as:

  ```
  group_by X grouping_cols calcs 
  ```

  or

  ```
  group_by grouping_cols calcs X
  ```
