# prqlc Architecture

The PRQL compiler operates in the following stages:

| stage    | sub-stage    | Abstract Syntax Tree (AST) Type used  |
| -------- | ------------ | ------------------------------------- |
| parse    | lexer        | string -> _LR — Lexer Representation_ |
| parse    | parser       | LR -> _PR — Parser Representation_    |
| semantic | ast_expand   | PR -> _PL — Pipelined Language_       |
| semantic | resolver     | PL                                    |
| semantic | flatten      | PL                                    |
| semantic | lowering     | PL -> _RQ — Resolved Query_           |
| sql      | preprocess   | RQ                                    |
| sql      | pq-compiler  | RQ -> _PQ — Partitioned Query_        |
| sql      | postprocess  | PQ                                    |
| sql      | sql-compiler | PQ -> `sqlparser::ast`                |
| sql      | codegen      | `sqlparser::ast` -> string            |

1. **Lexing & Parsing**: PRQL source text is split into tokens with the Chumsky
   parser named "lexer". The stream of tokens, as Lexer Representation (LR), is
   then parsed into an Abstract Syntax Tree (AST) called Parser Representation
   (PR).

2. **Semantic Analysis**: This stage resolves names (identifiers), extracts
   declarations, and determines frames (table columns in each step). A `Context`
   is declared containing the root module, which maps accessible names to their
   declarations.

   The resolving process involves the following operations:
   - Assign an ID to each node (`Expr` and `Stmt`).
   - Extract function declarations and variable definitions into the appropriate
     `Module`, accessible from `Context::root_mod`.
   - Look up identifiers in the module and find the associated declaration. The
     identifier is replaced with a fully qualified name that guarantees a unique
     name in `root_mod`. In some cases, `Expr::target` is also set.
   - Convert function calls to transforms (`from`, `derive`, `filter`) from
     `FuncCall` to `TransformCall`, which is more convenient for later
     processing.
   - Determine the type of expressions. If an expression is a reference to a
     table, use the frame of the table as the type. If it is a `TransformCall`,
     apply the transform to the input frame to obtain the resulting type. For
     simple expressions, try to infer from `ExprKind`.
   - Lowering: This stage converts the PL into RQ, which is more strictly typed
     and contains less information but is convenient for translating into SQL or
     other backends.

3. **SQL Backend**: This stage converts RQ into PQ, an intermediate AST, before
   finally converting to SQL. Each relation is transformed into an SQL query.
   Pipelines are analyzed and split into "AtomicPipelines" at appropriate
   positions, which can be represented by a single SELECT statement.

   Splitting is performed back-to-front. First, a list of all output columns is
   created. The pipeline is then traversed backwards, and splitting occurs when
   an incompatible transform with those already present in the pipeline is
   encountered. Splitting can also be triggered by encountering an expression
   that cannot be materialized where it is used (e.g., a window function in a
   WHERE clause).

   This process is also called anchoring, as it anchors a column definition to a
   specific location in the output query.

   During this process, `sql::context` keeps track of:
   - Table instances in the query (to prevent mixing up multiple instances of
     the same table)
   - Column definitions, whether computed or a reference to a table column
   - Column names, as defined in RQ or generated
