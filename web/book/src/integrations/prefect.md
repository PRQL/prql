# Prefect

Because [Prefect](https://www.prefect.io/) is in native Python, it's extremely
easy to integrate with PRQL.

With a Postgres Task, replace:

```python
PostgresExecute.run(..., query=sql)
```

...with...

```python
PostgresExecute.run(..., query=prql_python.compile(prql))
```

We're big fans of Prefect, and if there is anything that would make the
integration easier, please open an issue.
