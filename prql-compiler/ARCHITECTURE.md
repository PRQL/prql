# Architecture of PRQL compiler

Compiler works in the following stages:

1. Lexing & parsing - split PRQL text into tokens, build parse tree and convert into our AST (Abstract Syntax Tree, see `ast` module).
   Parsing is done using PEST parser (`prql.pest`), AST is constructed in `parser.rs`.

2. Semantic analysis - resolves names (identifiers), extracts declarations, determines frames (columns of the table in each step).
   It declares `Context` that contains current scope (list of accessible names) and declarations (values of identifiers).
   When an AST is resolved:

   - function declarations and other definitions are extracted into `Context::declarations`,
   - identifiers and function calls produce a lookup in `Context::scope` that finds associated declaration and saves the reference in `Node::declared_at`,
   - function calls to transforms (`from`, `derive`, `filter`) are converted from `FuncCall` into `Transform`, which is more convenient for later processing.

3. SQL Translator - converts resolved AST into SQL.
   It extracts all tables and pipeline and splits them into "atomic pipeline"s, which can be expressed with a single SELECT statement.
   Each atomic pipeline is then:
   - materialized (where needed, identifiers are converted into their declarations),
   - translated into SQL AST,
   - concatenated into one large SQL AST.
