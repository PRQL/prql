# dbt-prql

> Original docs at <https://github.com/prql/dbt-prql>

[dbt-prql](https://github.com/prql/dbt-prql) allows writing PRQL in dbt models.
This combines the benefits of PRQL's power & simplicity _within_ queries, with
dbt's version control, lineage & testing _across_ queries.

Once `dbt-prql` in installed, dbt commands compile PRQL between `{% prql %}` &
`{% endprql %}` jinja tags to SQL as part of dbt's compilation. No additional
config is required.

## Examples

### Simple example

```prql_no_test
{% prql %}
from employees
filter (age | in 20..30)
{% endprql %}
```

...would appear to dbt as:

```sql
SELECT
  employees.*
FROM
  employees
WHERE
  age BETWEEN 20
  AND 30
```

### Less simple example

```prql_no_test
{% prql %}
from {{ source('salesforce', 'in_process') }}
derive expected_sales = probability * value
join {{ ref('team', 'team_sales') }} [name]
group name (
  aggregate (expected_sales)
)
{% endprql %}
```

...would appear to dbt as:

```sql
SELECT
  name,
  {{ source('salesforce', 'in_process') }}.probability * {{ source('salesforce', 'in_process') }}.value AS expected_sales
FROM
  {{ source('salesforce', 'in_process') }}
  JOIN {{ ref('team', 'team_sales') }} USING(name)
GROUP BY
  name
```

...and then dbt will compile the `source` and `ref`s to a full SQL query.

### Replacing macros

dbt's use of macros has saved many of us many lines of code, and even saved some
people some time. But imperatively programming text generation with code like
`if not loop.last` is not our highest calling. It's the "necessary" part rather
than beautiful part of dbt.

Here's the canonical example of macros in the [dbt
documentation](https://docs.getdbt.com/docs/get-started/learning-more/using-jinja):

```sql
{%- set payment_methods = ["bank_transfer", "credit_card", "gift_card"] -%}

select
order_id,
{%- for payment_method in payment_methods %}
sum(case when payment_method = '{{payment_method}}' then amount end) as {{payment_method}}_amount
{%- if not loop.last %},{% endif -%}
{% endfor %}
from {{ ref('raw_payments') }}
group by 1
```

Here's that model using PRQL[^1], including the prql jinja tags.

```prql_no_test
{% prql %}
func filter_amount method -> s"sum(case when payment_method = '{method}' then amount end) as {method}_amount"

from {{ ref('raw_payments') }}
group order_id (
  aggregate [
    filter_amount bank_transfer,
    filter_amount credit_card,
    filter_amount gift_card,
  ]
)
{% endprql %}
```

As well the query being simpler in its final form, writing in PRQL also gives us
live feedback around any errors, on every keystroke. Though there's much more to
come, check out the current version on [PRQL
Playground](https://prql-lang.org/playground/).

## What it does

When dbt compiles models to SQL queries:

- Any text in a dbt model between `{% prql %}` and `{% endprql %}` tags is
  compiled from PRQL to SQL before being passed to dbt.
- The PRQL complier passes text that's containing `{{` & `}}` through to dbt
  without modification, which allows us to embed jinja expressions in PRQL.
  (This was added to PRQL specifically for this use-case.)
- dbt will then compile the resulting model into its final form of raw SQL, and
  dispatch it to the database, as per usual.

There's no config needed in the dbt project; this works automatically on any dbt
command (e.g. `dbt run`) assuming `dbt-prql` is installed.

## Installation

```sh
pip install dbt-prql
```

## Current state

Currently this is new, but fairly feature-complete. It's enthusiastically
supported — if there are any problems, please open an issue.
