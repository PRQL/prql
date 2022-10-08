# Prefect

Because Prefect is in native python, it's extremely easy to integrate with PRQL.

With a Postgres Task, replace:

```python
PostgresExecute.run(..., query=sql)
```

...with...

```python
PostgresExecute.run(..., query=pyprql.to_sql(prql))
```

We're big fans of Prefect, and if there is anything that would make the
integration easier, please open an issue.
