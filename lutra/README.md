# Lutra

Query runner for PRQL.

Status: early experimental development.

As prqlc provides conversion from PRQL source to SQL source, Lutra aims to
provide conversion from PRQL source to relational data.

For this to happen, PRQL source needs to include additional annotations (i.e.
`@lutra.sqlite`) that define data sources that contain the source data and can
execute SQL queries.

Lutra can be used as CLI (the binary in `lutra` crate) or as a library.
