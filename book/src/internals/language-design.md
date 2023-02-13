# Language design

In a way PRQL is just a transpiler to SQL, this causes its language design to
gravitate toward thinking about PRQL features in terms of how they translate to
SQL.

```
PRQL feature -> SQL feature -> relational result
```

This is flawed because:

- it does not model interactions between features well,
- SQL behavior can sometimes be misleading (the order of a subquery will not
  persist in the parent query) or even differs between dialects (set
  operations).

Instead, we should think of PRQL features in terms of how they affect PRQL
expressions, which in most cases means relation.

```
PRQL feature -> relation
                   |
                   v
PRQL feature -> relation
                   |
                   v
PRQL feature -> relation
                   |
                   v
            relational result
```

Thinking about SQL comes in only at the last step when relation (or rather
relational expression) is translated to an SQL expression.
