This directory contains the "core" crates that enable the PRQL compiler (that is
the conversion from PRQL to SQL). The bindings to call the compiler from other
programming languages can be found in [../bindings/](../bindings/).

`prqlc` is the CLI for the compiler.

Other related crates that live in separate repositories are:

- [prql-query] — another CLI that can also query .csv, .parquet and .json files

[prql-query]: https://github.com/PRQL/prql-query
