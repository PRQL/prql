# Compiler architecture

Compiler works in the following stages:

1. Lexing & parsing - split PRQL text into tokens, build parse tree and convert
   into our AST (Abstract Syntax Tree, see `ast` module). Parsing is done using
   PEST parser (`prql.pest`), AST is constructed in `parser.rs`.

2. Semantic analysis - resolves names (identifiers), extracts declarations,
   determines frames (columns of the table in each step). It declares `Context`
   that contains root module (mapping from accessible names to their
   declarations).

   Resolving includes following operations:

   - Assign an id to each node (`Expr` and `Stmt`).
   - Extract function declarations and variable def into appropriate `Module`,
     accessible from `Context::root_mod`
   - Lookup identifiers in module and find associated declaration. Ident is
     replaced with fully qualified name that guarantees unique name in
     `root_mod`. Sometimes, `Expr::target` is also set.
   - Function calls to transforms (`from`, `derive`, `filter`) are converted
     from `FuncCall` into `TransformCall`, which is more convenient for later
     processing.
   - Determine type of expr. If expr is a reference to a table use the frame of
     the table as the type. If it is a `TransformCall`, apply the transform to
     the input frame to obtain resulting type. For simple expressions, try to
     infer from `ExprKind`.

3. Lowering - converts PL into RQ that is more strictly typed, contains less
   information but is convenient for translating into SQL or some other backend.

4. SQL backend - converts RQ into SQL. It converts each of the relations into a
   SQL query. Pipelines are analyzed and split at appropriate positions into
   "AtomicPipelines" which can be represented by a single SELECT statement.

   Splitting is done back-to-front. First, we start with list all output columns
   we want. Then we traverse the pipeline backwards and split when we encounter
   a transform that is incompatible with transforms already present in the
   pipeline. Split can also be triggered by encountering an expression that
   cannot be materialized where it is used (window function is WHERE for
   example).

   This process is also called anchoring, because it anchors a column definition
   to a specific location in the output query.

   During this process, `sql::context` keeps track of:

   - table instances in the query (to prevent mixing up two instances of the
     same table)
   - column definitions, whether computed or a reference to a table column,
   - column names, as defined in RQ or generated
