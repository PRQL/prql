# Annotated example

The [Playground](https://prql-lang.org/playground) defaults to showing a query
that demonstrates most of the transforms and capabilities of PRQL. This page
explains the details of each line in that example.

<!--
  This is the full query from the Playground. It needs to be explained line by line
  It is not currently linked into the SUMMARY.md page
-->

```prql no-eval
from invoices                        # A PRQL query begins with a table
                                     # Subsequent lines "transform" (modify) it
derive {                             # "derive" adds columns to the result
  transaction_fee = 0.8,             # "=" sets a column name
  income = total - transaction_fee   # Calculations can use other column names
}
# This is a comment; commenting out a line leaves a valid query
filter income > 5                    # "filter" replaces both of SQL's WHERE & HAVING
filter invoice_date >= @2010-01-16   # Clear date syntax
group customer_id (                  # "group" performs the pipeline in (...) on each group
  aggregate {                        # "aggregate" reduces each group to a single row
    sum_income = sum income,         # ... using SQL SUM(), COUNT(), etc. functions
    ct = count customer_id,          #
  }
)
join c=customers (==customer_id)     # join on "customer_id" from both tables
derive name = f"{c.last_name}, {c.first_name}" # F-strings like Python
derive db_version = s"version()"     # S-string offers escape hatch to SQL
select {                             # "select" passes along only the named columns
  c.customer_id, name, sum_income, ct, db_version,
}                                    # trailing commas always ignored
sort {-sum_income}                   # "sort" sorts the result; "-" is decreasing order
take 1..10                           # Limit to a range - could also be "take 10"
#
# The "output.sql" tab at right shows the SQL generated from this PRQL query
# The "output.arrow" tab shows the result of the query
```
