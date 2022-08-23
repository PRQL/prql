//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::semantic::{Context, Declaration, Declarations};

use anyhow::Result;
use itertools::Itertools;

use crate::semantic::split_var_name;

/// Replaces all resolved functions and variables with their declarations.
pub fn materialize(
    mut pipeline: Vec<Transform>,
    mut context: MaterializationContext,
    as_table: Option<usize>,
) -> Result<(Vec<Transform>, MaterializedFrame, MaterializationContext)> {
    context.frame.sort.clear();
    extract_frame(&mut pipeline, &mut context)?;

    let mut m = Materializer::new(context);

    // materialize the query
    let pipeline = m.fold_transforms(pipeline)?;
    let frame = m.context.frame.clone();

    // materialize each of the columns
    let columns = m.anchor_columns(frame.columns, as_table)?;

    let sort = m.materialize_sort(frame.sort)?;

    // rename tables for future pipelines
    if let Some(as_table) = as_table {
        m.replace_tables(frame.tables, as_table);
    }

    Ok((pipeline, MaterializedFrame { columns, sort }, m.context))
}

fn extract_frame(
    pipeline: &mut Vec<Transform>,
    context: &mut MaterializationContext,
) -> Result<()> {
    for transform in pipeline.iter_mut() {
        context.frame = transform.kind.apply_to(context.frame.clone())?;
    }

    pipeline.retain(|transform| {
        !matches!(
            transform.kind,
            TransformKind::Select(_) | TransformKind::Derive(_) | TransformKind::Sort(_)
        )
    });
    Ok(())
}
#[derive(Debug)]
pub struct MaterializedFrame {
    pub columns: Vec<Expr>,
    pub sort: Vec<ColumnSort>,
}

#[derive(Default)]
pub struct MaterializationContext {
    /// Map of all accessible names (for each namespace)
    pub(super) frame: Frame,

    /// All declarations, mostly from [semantic::Context]
    pub(super) declarations: Declarations,
}

impl MaterializationContext {
    pub fn declare_table(&mut self, table: &str) -> usize {
        let id = self
            .declarations
            .push(Declaration::Table(table.to_string()), None);

        self.frame.tables.push(id);
        id
    }
}

impl From<Context> for MaterializationContext {
    fn from(context: Context) -> Self {
        MaterializationContext {
            frame: Frame::default(),
            declarations: context.declarations,
        }
    }
}

/// Can fold (walk) over AST and replace function calls and variable references with their declarations.
#[derive(Debug)]
pub struct Materializer {
    pub context: MaterializationContext,
    pub remove_namespaces: bool,
}

impl Materializer {
    fn new(context: MaterializationContext) -> Self {
        Materializer {
            remove_namespaces: context.frame.tables.len() == 1,
            context,
        }
    }

    /// Looks up column declarations and replaces them with an identifiers.
    fn anchor_columns(
        &mut self,
        columns: Vec<FrameColumn>,
        as_table: Option<usize>,
    ) -> Result<Vec<Expr>> {
        let mut to_replace = Vec::new();

        let res = columns
            .into_iter()
            .map(|column| {
                Ok(match column {
                    FrameColumn::Named(name, id) => {
                        let expr_node = self.materialize_declaration(id)?;

                        let name = split_var_name(&name).1.to_string();

                        let decl = Declaration::ExternRef {
                            variable: name.clone(),
                            table: as_table,
                        };
                        to_replace.push((id, decl));

                        emit_column_with_name(expr_node, name)
                    }
                    FrameColumn::Unnamed(id) => {
                        // no need to replace declaration, since it cannot be referenced again
                        self.materialize_declaration(id)?
                    }
                    FrameColumn::All(namespace) => {
                        let decl = &self.context.declarations.get(namespace);
                        let table = decl.as_table().unwrap();
                        ExprKind::Ident(format!("{table}.*")).into()
                    }
                })
            })
            .collect::<Result<Vec<_>>>()?;

        for (id, decl) in to_replace {
            self.context.declarations.replace(id, decl);
        }

        Ok(res)
    }

    fn replace_tables(&mut self, tables: Vec<usize>, new_table: usize) {
        let new_table = self.context.declarations.get(new_table).clone();
        for id in tables {
            self.context.declarations.replace(id, new_table.clone());
        }
    }

    /// Folds the column and returns expression that can be used in select.
    /// Also returns column id and name if declaration should be replaced.
    fn materialize_sort(&mut self, sort: Vec<ColumnSort<usize>>) -> Result<Vec<ColumnSort>> {
        sort.into_iter()
            .map(|s| {
                Ok(ColumnSort {
                    column: self.materialize_declaration(s.column)?,
                    direction: s.direction,
                })
            })
            .try_collect()
    }

    fn materialize_declaration(&mut self, id: usize) -> Result<Expr> {
        let decl = self.context.declarations.get(id);

        let materialized = match decl.clone() {
            Declaration::Expression(inner) => self.fold_expr(*inner)?,
            Declaration::ExternRef { table, variable } => {
                let name = if let Some(table) = table {
                    let (_, var_name) = split_var_name(&variable);

                    if self.remove_namespaces {
                        var_name.to_string()
                    } else {
                        let table = &self.context.declarations.get(table);
                        let table = table.as_table().unwrap();
                        format!("{table}.{var_name}")
                    }
                } else {
                    variable
                };

                ExprKind::Ident(name).into()
            }
            Declaration::Function(func_call) => {
                // function without arguments (a global variable)

                let body = func_call.body;

                self.fold_expr(*body)?
            }
            Declaration::Table(table) => ExprKind::Ident(format!("{table}.*")).into(),
        };

        Ok(materialized)
    }
}

fn emit_column_with_name(mut expr_node: Expr, name: String) -> Expr {
    // is expr_node just an ident with same name?
    let expr_ident = expr_node.kind.as_ident().map(|n| split_var_name(n).1);

    if !expr_ident.map(|n| n == name).unwrap_or(false) {
        // set expr alias
        expr_node.alias = Some(name);
    }
    expr_node
}

impl AstFold for Materializer {
    fn fold_expr(&mut self, mut node: Expr) -> Result<Expr> {
        // We replace Items and also pass node to `inline_func_call`,
        // so we need to run this here rather than in `fold_func_call` or `fold_item`.

        Ok(match node.kind {
            ExprKind::Ident(_) => {
                if let Some(id) = node.declared_at {
                    self.materialize_declaration(id)?
                } else {
                    node
                }
            }

            _ => {
                node.kind = fold_expr_kind(self, node.kind)?;
                node
            }
        })
    }
}

impl std::fmt::Debug for MaterializationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:?}", self.declarations)
    }
}
#[cfg(test)]
mod test {

    use super::*;
    use crate::{parse, semantic::resolve, utils::diff};
    use insta::{assert_display_snapshot, assert_snapshot, assert_yaml_snapshot};
    use serde_yaml::to_string;

    fn resolve_and_materialize(stmts: Vec<Stmt>) -> Result<Vec<TransformKind>> {
        let (res, context) = resolve(stmts, None)?;

        let pipeline = res.main_pipeline;

        let (mat, _, _) = materialize(pipeline, context.into(), None)?;
        Ok(mat.into_iter().map(|t| t.kind).collect())
    }

    #[test]
    fn test_replace_variables_1() -> Result<()> {
        let query = parse(
            r#"from employees
    derive [                                         # This adds columns / variables.
      gross_salary = salary + payroll_tax,
      gross_cost =   gross_salary + benefits_cost     # Variables can use other variables.
    ]
    "#,
        )?;

        let (res, context) = resolve(query, None)?;

        let (mat, _, _) = materialize(res.main_pipeline.clone(), context.into(), None)?;

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(diff(
            &to_string(&res.main_pipeline)?,
            &to_string(&mat)?
        ),
        @r###"
        @@ -3,6 +3,3 @@
             name: employees
             alias: null
             declared_at: 79
        -- Transform: !Derive
        -  - Ident: gross_salary
        -  - Ident: gross_cost
        "###);

        Ok(())
    }

    #[test]
    fn test_replace_variables_2() -> Result<()> {
        let query = parse(
            r#"
from employees
filter country == "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
    aggregate [                  # `by` are the columns to group by.
        average salary,                              # These are aggregation calcs run on each group.
        sum     salary,
        average gross_salary,
        sum     gross_salary,
        average gross_cost,
        sum_gross_cost = sum gross_cost,
        ct = count,
    ]
)
sort sum_gross_cost
filter sum_gross_cost > 200
take 20
"#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(&mat);

        Ok(())
    }

    #[test]
    fn test_non_existent_function() -> Result<()> {
        let query = parse(r#"from mytable | filter (myfunc col1)"#)?;
        assert!(resolve(query, None).is_err());

        Ok(())
    }

    #[test]
    fn test_run_functions_args() -> Result<()> {
        let stmts = parse(
            r#"
        from employees
        aggregate [
            sum salary
        ]
        "#,
        )?;

        assert_yaml_snapshot!(stmts, @r###"
        ---
        - Pipeline:
            nodes:
              - FuncCall:
                  name:
                    Ident: from
                  args:
                    - Ident: employees
                  named_args: {}
              - FuncCall:
                  name:
                    Ident: aggregate
                  args:
                    - List:
                        - FuncCall:
                            name:
                              Ident: sum
                            args:
                              - Ident: salary
                            named_args: {}
                  named_args: {}
        "###);

        let (res, context) = resolve(stmts, None)?;

        let (mat, _, _) = materialize(res.main_pipeline.clone(), context.into(), None)?;

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        let diff = diff(&to_string(&res.main_pipeline)?, &to_string(&mat)?);
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -4,5 +4,9 @@
             declared_at: 79
         - Transform: !Aggregate
             assigns:
        -    - Ident: <unnamed>
        +    - SString:
        +      - !String SUM(
        +      - !Expr
        +        Ident: salary
        +      - !String )
             by: []
        "###);

        Ok(())
    }

    #[test]
    fn test_run_functions_nested() -> Result<()> {
        let stmts = parse(
            r#"
        func lag_day x ->  s"lag_day_todo({x})"
        func ret x dividend_return ->  x / (lag_day x) - 1 + dividend_return

        from a
        select (ret b c)
        "#,
        )?;

        assert_yaml_snapshot!(stmts[2], @r###"
        ---
        Pipeline:
          nodes:
            - FuncCall:
                name:
                  Ident: from
                args:
                  - Ident: a
                named_args: {}
            - FuncCall:
                name:
                  Ident: select
                args:
                  - FuncCall:
                      name:
                        Ident: ret
                      args:
                        - Ident: b
                        - Ident: c
                      named_args: {}
                named_args: {}
        "###);

        let mat = resolve_and_materialize(stmts).unwrap();
        assert_yaml_snapshot!(mat, @r###"
        ---
        - Transform:
            From:
              name: a
              alias: ~
              declared_at: 84
        "###);

        Ok(())
    }

    #[test]
    fn test_run_inline_pipelines() -> Result<()> {
        let query = parse(
            r#"
        from a
        aggregate [one = (foo | sum), two = (foo | sum)]
        "#,
        )?;

        let (res, context) = resolve(query, None)?;

        let (mat, _, _) = materialize(res.main_pipeline.clone(), context.into(), None)?;

        assert_snapshot!(diff(&to_string(&res.main_pipeline)?, &to_string(&mat)?), @r###"
        @@ -4,6 +4,14 @@
             declared_at: 79
         - Transform: !Aggregate
             assigns:
        -    - Ident: one
        -    - Ident: two
        +    - SString:
        +      - !String SUM(
        +      - !Expr
        +        Ident: foo
        +      - !String )
        +    - SString:
        +      - !String SUM(
        +      - !Expr
        +        Ident: foo
        +      - !String )
             by: []
        "###);

        // Test it'll run the `sum foo` function first.
        let query = parse(
            r#"
        func plus_one x ->  x + 1

        from a
        aggregate [a = (sum foo | plus_one)]
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();

        assert_yaml_snapshot!(mat, @r###"
        ---
        - Transform:
            From:
              name: a
              alias: ~
              declared_at: 81
        - Transform:
            Aggregate:
              assigns:
                - Binary:
                    left:
                      SString:
                        - String: SUM(
                        - Expr:
                            Ident: foo
                        - String: )
                    op: Add
                    right:
                      Literal:
                        Integer: 1
                      ty:
                        Literal: Integer
                  ty: Infer
              by: []
          ty:
            Function:
              named: {}
              args:
                - Infer
              return_ty:
                Literal: Table
        "###);

        Ok(())
    }

    #[test]
    fn test_named_args() -> Result<()> {
        let query = parse(
            r#"
        func add x to:1 ->  x + to

        from foo_table
        derive [
            added = add bar to:3,
            added_default = add bar
        ]
        "#,
        )?;
        let mat = resolve_and_materialize(query).unwrap();

        assert_yaml_snapshot!(mat, @r###"
        ---
        - Transform:
            From:
              name: foo_table
              alias: ~
              declared_at: 82
        "###);

        Ok(())
    }

    #[test]
    fn test_materialize_1() -> Result<()> {
        let query = parse(
            r#"
        from employees
        aggregate [
            sum salary
        ]
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat,
            @r###"
        ---
        - Transform:
            From:
              name: employees
              alias: ~
              declared_at: 79
        - Transform:
            Aggregate:
              assigns:
                - SString:
                    - String: SUM(
                    - Expr:
                        Ident: salary
                    - String: )
                  ty: Infer
              by: []
          ty:
            Function:
              named: {}
              args:
                - Infer
              return_ty:
                Literal: Table
        "###
        );
        Ok(())
    }

    #[test]
    fn test_materialize_2() -> Result<()> {
        let query = parse(
            r#"
from employees
filter country == "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost =   gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
    aggregate [                  # `by` are the columns to group by.
        average salary,                              # These are aggregation calcs run on each group.
        sum     salary,
        average gross_salary,
        sum     gross_salary,
        average gross_cost,
        sum_gross_cost = sum gross_cost,
        ct = count,
    ]
)
sort sum_gross_cost
filter sum_gross_cost > 200
take 20
"#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat);
        Ok(())
    }

    #[test]
    fn test_materialize_3() -> Result<()> {
        let query = parse(
            r#"
        func interest_rate ->  0.2

        func lag_day x ->  s"lag_day_todo({x})"
        func ret x dividend_return ->  x / (lag_day x) - 1 + dividend_return
        func excess x ->  (x - interest_rate) / 252
        func if_valid x ->  s"IF(is_valid_price, {x}, NULL)"

        from prices
        derive [
            return_total      = if_valid (ret prices_adj div_ret),
            return_usd        = if_valid (ret prices_usd div_ret),
            return_excess     = excess return_total,
            return_usd_excess = excess return_usd,
        ]
        select [
            date,
            sec_id,
            return_total,
            return_usd,
            return_excess,
            return_usd_excess,
        ]
        "#,
        )?;
        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat);

        Ok(())
    }

    #[test]
    fn test_variable_after_aggregate() -> Result<()> {
        let query = parse(
            r#"
        from employees
        group [title, emp_no] (
            aggregate [emp_salary = average salary]
        )
        group [title] (
            aggregate [avg_salary = average emp_salary]
        )
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat, @r###"
        ---
        - Transform:
            From:
              name: employees
              alias: ~
              declared_at: 79
        - Transform:
            Group:
              by:
                - Ident: title
                - Ident: emp_no
              pipeline:
                Pipeline:
                  nodes:
                    - Transform:
                        Aggregate:
                          assigns:
                            - SString:
                                - String: AVG(
                                - Expr:
                                    Ident: salary
                                - String: )
                              ty: Infer
                          by:
                            - Ident: title
                            - Ident: emp_no
                      ty:
                        Function:
                          named: {}
                          args:
                            - Infer
                          return_ty:
                            Literal: Table
          ty:
            Function:
              named: {}
              args:
                - Infer
              return_ty:
                Literal: Table
        - Transform:
            Group:
              by:
                - Ident: title
              pipeline:
                Pipeline:
                  nodes:
                    - Transform:
                        Aggregate:
                          assigns:
                            - SString:
                                - String: AVG(
                                - Expr:
                                    SString:
                                      - String: AVG(
                                      - Expr:
                                          Ident: salary
                                      - String: )
                                - String: )
                              ty: Infer
                          by:
                            - Ident: title
                      ty:
                        Function:
                          named: {}
                          args:
                            - Infer
                          return_ty:
                            Literal: Table
          ty:
            Function:
              named: {}
              args:
                - Infer
              return_ty:
                Literal: Table
        "###);

        Ok(())
    }

    #[test]
    fn test_frames_and_names() -> Result<()> {
        let query1 = parse(
            r#"
        from orders
        select [customer_no, gross, tax, gross - tax]
        take 20
        "#,
        )?;

        let query2 = parse(
            r#"
        from table_1
        join customers [customer_no]
        "#,
        )?;

        let (res1, context) = resolve(query1, None)?;
        let (res2, context) = resolve(query2, Some(context))?;

        let (mat, frame, context) = materialize(res1.main_pipeline, context.into(), None)?;

        assert_yaml_snapshot!(mat, @r###"
        ---
        - Transform:
            From:
              name: orders
              alias: ~
              declared_at: 79
        - Transform:
            Take:
              range:
                start: ~
                end:
                  Literal:
                    Integer: 20
              by: []
              sort: []
          ty:
            Function:
              named: {}
              args:
                - Infer
              return_ty:
                Literal: Table
        "###);
        assert_yaml_snapshot!(frame.columns, @r###"
        ---
        - Ident: customer_no
        - Ident: gross
        - Ident: tax
        - Binary:
            left:
              Ident: gross
            op: Sub
            right:
              Ident: tax
          ty: Infer
        "###);

        let (mat, frame, _) = materialize(res2.main_pipeline, context, None)?;

        assert_yaml_snapshot!(mat, @r###"
        ---
        - Transform:
            From:
              name: table_1
              alias: ~
              declared_at: 84
        - Transform:
            Join:
              side: Inner
              with:
                name: customers
                alias: ~
                declared_at: 85
              filter:
                Using:
                  - Ident: customer_no
          ty:
            Function:
              named: {}
              args:
                - Infer
              return_ty:
                Literal: Table
        "###);
        assert_yaml_snapshot!(frame.columns, @r###"
        ---
        - Ident: table_1.*
        - Ident: customers.*
        - Ident: customer_no
        "###);

        Ok(())
    }
}
