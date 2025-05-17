# QStudio IDE

[QStudio](https://www.timestored.com/qstudio/prql-ide) is a SQL GUI that lets
you browse tables, run SQL scripts, and chart and export the results. QStudio
runs on Windows, macOS and Linux, and works with every popular database
including mysql, postgresql, mssql, kdb....

```admonish note
QStudio relies on the PRQL compiler. You must ensure that `prqlc` is in your path. See the [installation instructions](https://prql-lang.org/book/project/integrations/prqlc-cli.html#installation) for `prqlc` in the PRQL reference guide for details.
```

QStudio calls `prqlc` (the PRQL compiler) to generate SQL code from PRQL queries
(.prql files) then runs the SQL against the selected database to display the
results. For more details, check out:

- [QStudio site](https://www.timestored.com/qstudio/prql-ide)
- [QStudio-PRQL Quick Start](https://github.com/richb-hanover/qStudio-PRQL_Quick_Start)
- There is a
  [double-clickable macOS app](https://randomneuronsfiring.com/wp-content/uploads/QStudio.zip)
  that bundles QStudio and the `prqlc` compiler.
