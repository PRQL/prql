# dbt-prql

> Original docs at <https://github.com/prql/dbt-prql>

```note admonish
As of Feb 2022, we're again considering how to best integrate with
dbt more closely. Ideally a file with a `.prql` extension will just work™.

If you're interested in this, subscribe or 👍 to
<https://github.com/dbt-labs/dbt-core/pull/5982>.

The original plugin is hosted here, but only works with a minority of
dialects, and isn't a focus of development at the moment.
```

dbt-prql allows writing PRQL in [dbt](https://www.getdbt.com/) models. This
combines the benefits of PRQL's power & simplicity _within_ queries, with dbt's
version control, lineage & testing _across_ queries.

Once `dbt-prql` in installed, dbt commands compile PRQL between `{% prql %}` &
`{% endprql %}` Jinja tags to SQL as part of dbt's compilation. No additional
config is required.

## Examples

### Simple example

```elm
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

```elm
{% prql %}
from in_process = {{ source('salesforce', 'in_process') }}
derive expected_sales = probability * value
join {{ ref('team', 'team_sales') }} [name]
group name (
  aggregate (sum expected_sales)
)
{% endprql %}
```

...would appear to dbt as:

```sql
SELECT
  name,
  sum(in_process.probability * in_process.value) AS expected_sales
FROM
  {{ source('salesforce', 'in_process') }} AS in_process
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

Here's the canonical example of macros in the
[dbt documentation](https://docs.getdbt.com/docs/build/jinja-macros):

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

Here's that model using PRQL[^1], including the prql Jinja tags.

```elm
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
come, check out the current version on
[PRQL Playground](https://prql-lang.org/playground/).

[^1]:
    Note that when <https://github.com/prql/prql/issues/82> is implemented, we
    can dispense with the s-string, and optionally dispense with the function.

    ```elm
    from {{ ref('raw_payments') }}
    group order_id (
      aggregate [
        bank_transfer_amount = amount | filter payment_method == 'bank'        | sum,
        credit_card_amount = amount   | filter payment_method == 'credit_card' | sum,
        gift_amount = amount          | filter payment_method == 'gift_card'   | sum,
      ]
    )
    ```

    or

    ```elm
    func filter_amount method -> amount | filter payment_method == method | sum

    from {{ ref('raw_payments') }}
    group order_id (
      aggregate [
        bank_transfer_amount = filter_amount 'bank'
        credit_card_amount   = filter_amount 'credit_card'
        gift_amount          = filter_amount 'gift_card'
      ]
    )
    ```

## What it does

When dbt compiles models to SQL queries:

- Any text in a dbt model between `{% prql %}` and `{% endprql %}` tags is
  compiled from PRQL to SQL before being passed to dbt.
- The PRQL compiler passes text that's containing `{{` & `}}` through to dbt
  without modification, which allows us to embed Jinja expressions in PRQL.
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

## How does it work?

It's some dark magic, unfortunately.

dbt doesn't allow adding behavior beyond the database adapters (e.g.
`dbt-bigquery`) or Jinja-only plugins (e.g. `dbt-utils`). So this library hacks
the Python import system to monkeypatch dbt's Jinja environment with an
additional Jinja extension on Python's startup[^2].

[^2]:
    Thanks to
    [mtkennerly/poetry-dynamic-versioning](https://github.com/mtkennerly/poetry-dynamic-versioning)
    for the technique.

This approach was discussed with the dbt team
[here](https://github.com/prql/prql/issues/375) and
[here](https://github.com/prql/prql/issues/13).

This isn't stable between dbt versions, since it relies on internal dbt APIs.
The technique is also normatively bad — it runs a few lines of code every time
the Python interpreter starts — whose errors could lead to very confusing bugs
beyond the domain of the problem (though in the case of this library, it's small
and well-constructed™).

If there's ever any concern that the library might be causing a problem, just
set an environment variable `DBT_PRQL_DISABLE=1`, and this library won't
monkeypatch anything. It's also fully uninstallable with
`pip uninstall dbt-prql`.

## Roadmap

Open to ideas; at the moment it's fairly feature-complete. If we were
unconstrained in dbt functionality:

- If dbt allowed for external plugins, we'd enthusiastically move to that.
- We'd love to have this work on `.prql` files without the `{% prql %}` tags;
  but with the current approach that would require quite invasive
  monkeypatching.
- If we could add the dialect in automatically (i.e. `prql dialect:snowflake`),
  that would save a line per model.
- If we could upstream this into dbt-core, that would be awesome. It may be on
  PRQL to demonstrate its staying power before that, though.

We may move this library to the <https://github.com/prql/PyPrql> or
<https://github.com/prql/prql> repos. We'd prefer to keep it as its own package
given the hackery above, but there's no need for it to be its own repo.
