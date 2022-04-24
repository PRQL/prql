//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::Declaration;

use anyhow::{anyhow, Result};
use itertools::zip;
use itertools::Itertools;

use crate::semantic::{split_var_name, Context, Frame, TableColumn};

pub struct MaterializedFrame {
    pub columns: Vec<Node>,
    pub sort: Vec<ColumnSort<Ident>>,
}

/// Replaces all resolved functions and variables with their declarations.
pub fn materialize(
    pipeline: Pipeline,
    frame: Frame,
    context: Context,
    as_table: Option<&str>,
) -> Result<(Pipeline, MaterializedFrame, Context)> {
    let mut counter = TableCounter::default();
    let pipeline = counter.fold_pipeline(pipeline).unwrap();

    let mut m = Materializer {
        context,
        remove_namespaces: counter.tables == 1,
    };

    // materialize the query
    let pipeline = m.fold_pipeline(pipeline)?;

    // materialize each of the columns
    let columns = m.anchor_columns(frame.columns, as_table)?;

    // materialize each of the columns
    let sort = m.lookup_sort(frame.sort)?;

    // rename tables for future pipelines
    if let Some(as_table) = as_table {
        m.rename_tables(frame.tables, as_table);
    }

    Ok((pipeline, MaterializedFrame { columns, sort }, m.context))
}

/// Can fold (walk) over AST and replace function calls and variable references with their declarations.
pub struct Materializer {
    pub context: Context,
    pub remove_namespaces: bool,
}

impl Materializer {
    /// Looks up column declarations and replaces them with an identifiers.
    fn anchor_columns(
        &mut self,
        columns: Vec<TableColumn>,
        as_table: Option<&str>,
    ) -> Result<Vec<Node>> {
        let as_table = as_table.map(|table| {
            let decl = Declaration::Table(table.to_string());
            self.context.declare(decl, None)
        });

        columns
            .into_iter()
            .map(|column| {
                Ok(match column {
                    TableColumn::Named(name, id) => {
                        let expr_node = self.lookup_declaration(id)?;

                        let name = split_var_name(&name).1.to_string();

                        let decl = Declaration::ExternRef {
                            variable: name.clone(),
                            table: as_table,
                        };
                        self.context.replace_declaration(id, decl);

                        // is expr_node just an ident with same name?
                        let expr_ident = expr_node.item.as_ident().map(|n| split_var_name(n).1);

                        if expr_ident.map(|n| n == name).unwrap_or(false) {
                            // return just the ident
                            expr_node
                        } else {
                            // return expr with new name
                            Item::NamedExpr(NamedExpr {
                                expr: Box::new(expr_node),
                                name,
                            })
                            .into()
                        }
                    }
                    TableColumn::Unnamed(id) => {
                        // no need to replace declaration, since it cannot be referenced again
                        self.lookup_declaration(id)?
                    }
                    TableColumn::All(namespace) => {
                        let (decl, _) = &self.context.declarations[namespace];
                        let table = decl.as_table().unwrap();
                        Item::Ident(format!("{table}.*")).into()
                    }
                })
            })
            .try_collect()
    }

    fn rename_tables(&mut self, tables: Vec<usize>, new_table: &str) {
        for id in tables {
            let (decl, _) = self.context.declarations.get_mut(id).unwrap();

            let table = decl.as_table_mut().unwrap();
            *table = new_table.to_string();
        }
    }

    fn lookup_declaration(&mut self, id: usize) -> Result<Node> {
        let (decl, _) = &self.context.declarations[id];
        if !matches!(decl, &Declaration::Expression { .. }) {
            self.materialize_declaration(id)?;
        }

        let (decl, _) = &self.context.declarations[id];
        Ok(*decl.clone().into_expression()?)
    }

    /// Folds the column and returns expression that can be used in select.
    /// Also returns column id and name if declaration should be replaced.
    fn lookup_sort(&mut self, sort: Vec<ColumnSort<usize>>) -> Result<Vec<ColumnSort<Ident>>> {
        sort.into_iter()
            .map(|s| {
                let ident = self.lookup_declaration(s.column)?;
                let column = ident.item.into_ident()?;

                Ok(ColumnSort {
                    column,
                    direction: s.direction,
                })
            })
            .try_collect()
    }

    fn materialize_declaration(&mut self, id: usize) -> Result<Node> {
        let (decl, _) = &self.context.declarations[id];

        // eprintln!("materialize_declaration {id}");
        // dbg!(decl);

        let materialized = match decl.clone() {
            Declaration::Expression(inner) => self.fold_node(*inner)?,
            Declaration::ExternRef { table, variable } => {
                let name = if let Some(table) = table {
                    let (_, var_name) = split_var_name(&variable);

                    if self.remove_namespaces {
                        var_name.to_string()
                    } else {
                        let (table, _) = &self.context.declarations[table];
                        let table = table.as_table().unwrap();
                        format!("{table}.{var_name}")
                    }
                } else {
                    variable
                };

                Item::Ident(name).into()
            }
            Declaration::Function(func_call) => {
                // function without arguments (a global variable)

                let body = func_call.body;

                self.fold_node(*body)?
            }
            Declaration::Table(table) => Item::Ident(format!("{table}.*")).into(),
        };
        self.context
            .replace_declaration_expr(id, materialized.clone());

        Ok(materialized)
    }

    fn materialize_func_call(&mut self, node: &Node) -> Result<Node> {
        let func_call = node.item.as_func_call().unwrap();

        // locate declaration
        let func_dec = node.declared_at.ok_or_else(|| anyhow!("unresolved"))?;
        let func_dec = &self.context.declarations[func_dec].0;
        let func_dec = func_dec.as_function().unwrap().clone();

        // TODO: check if the function is called recursively.

        // for each of the params, replace its declared value
        for param in func_dec.named_params {
            let id = param.declared_at.unwrap();
            let param = param.item.into_named_expr()?;

            let value = func_call
                .named_args
                .get(&param.name)
                .map_or_else(|| param.expr.item.clone(), |expr| expr.item.clone());

            self.context.replace_declaration_expr(id, value.into());
        }
        for (param, arg) in zip(func_dec.positional_params.iter(), func_call.args.iter()) {
            let id = param.declared_at.unwrap();
            let expr = arg.item.clone().into();
            self.context.replace_declaration_expr(id, expr);
        }

        // Now fold body as normal node
        self.fold_node(*func_dec.body)
    }
}

impl AstFold for Materializer {
    fn fold_node(&mut self, mut node: Node) -> Result<Node> {
        // We replace Items and also pass node to `inline_func_call`,
        // so we need to run this here rather than in `fold_func_call` or `fold_item`.

        Ok(match node.item {
            Item::FuncCall(func_call) => {
                let func_call = Item::FuncCall(self.fold_func_call(func_call)?);
                let func_call = Node {
                    item: func_call,
                    ..node
                };

                self.materialize_func_call(&func_call)?
            }

            Item::Pipeline(p) => {
                if let Some(value) = p.value {
                    // there is leading value -> this is an inline pipeline -> materialize

                    let mut value = self.fold_node(*value)?;

                    for mut func_call in p.functions {
                        // The value from the previous pipeline becomes the final arg.
                        if let Some(call) = func_call.item.as_func_call_mut() {
                            call.args.push(value);
                        }

                        value = self.materialize_func_call(&func_call)?;
                    }
                    value
                } else {
                    // there is no leading value -> this is a frame pipeline -> just fold

                    let pipeline = fold_pipeline(self, p)?;

                    node.item = Item::Pipeline(pipeline);
                    node
                }
            }

            Item::Ident(_) => {
                if let Some(id) = node.declared_at {
                    self.materialize_declaration(id)?
                } else {
                    node
                }
            }

            _ => {
                node.item = fold_item(self, node.item)?;
                node
            }
        })
    }
}

/// Counts all tables in scope
#[derive(Default)]
struct TableCounter {
    tables: usize,
}

impl AstFold for TableCounter {
    fn fold_func_def(&mut self, function: FuncDef) -> Result<FuncDef> {
        Ok(function)
    }

    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        match &transform {
            Transform::From(_) | Transform::Join { .. } => {
                self.tables += 1;
            }
            _ => {}
        }
        // no need to recurse, transformations cannot be nested
        Ok(transform)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{parse, semantic::resolve, utils::diff};
    use insta::{assert_display_snapshot, assert_snapshot, assert_yaml_snapshot};
    use serde_yaml::to_string;

    fn resolve_and_materialize(nodes: Vec<Node>) -> Result<Vec<Node>> {
        let (res, context) = resolve(nodes, None)?;

        let pipeline = res.last().unwrap().item.as_pipeline().unwrap();
        let frame = pipeline.functions.last().unwrap().frame.clone().unwrap();

        let (mat, _, _) = materialize(res.into(), frame, context, Some("table_1"))?;
        Ok(mat.functions)
    }

    #[test]
    fn test_replace_variables_1() -> Result<()> {
        let ast = parse(
            r#"from employees
    derive [                                         # This adds columns / variables.
      gross_salary: salary + payroll_tax,
      gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
    ]
    "#,
        )?
        .nodes;

        let mat = resolve_and_materialize(ast.clone()).unwrap();

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(diff(
            &to_string(&ast)?,
            &to_string(&mat)?
        ),
        @r###"
        @@ -2,27 +2,24 @@
         - Pipeline:
             value: ~
             functions:
        -      - FuncCall:
        -          name: from
        -          args:
        -            - Ident: employees
        -          named_args: {}
        -      - FuncCall:
        -          name: derive
        -          args:
        -            - List:
        -                - NamedExpr:
        -                    name: gross_salary
        -                    expr:
        -                      Expr:
        -                        - Ident: salary
        -                        - Raw: +
        -                        - Ident: payroll_tax
        -                - NamedExpr:
        -                    name: gross_cost
        -                    expr:
        -                      Expr:
        -                        - Ident: gross_salary
        -                        - Raw: +
        -                        - Ident: benefits_cost
        -          named_args: {}
        +      - Transform:
        +          From:
        +            name: employees
        +            alias: ~
        +      - Transform:
        +          Derive:
        +            assigns:
        +              - Expr:
        +                  - Ident: salary
        +                  - Raw: +
        +                  - Ident: payroll_tax
        +              - Expr:
        +                  - Expr:
        +                      - Ident: salary
        +                      - Raw: +
        +                      - Ident: payroll_tax
        +                  - Raw: +
        +                  - Ident: benefits_cost
        +            group: []
        +            window: ~
        +            sort: ~
        "###);

        Ok(())
    }

    #[test]
    fn test_replace_variables_2() -> Result<()> {
        let ast = parse(
            r#"
func count = s"COUNT(*)"
func average column = s"AVG({column})"
func sum column = s"SUM({column})"

from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost  : gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
    aggregate [                  # `by` are the columns to group by.
        average salary,                              # These are aggregation calcs run on each group.
        sum     salary,
        average gross_salary,
        sum     gross_salary,
        average gross_cost,
        sum_gross_cost: sum gross_cost,
        ct: count,
    ]
)
sort sum_gross_cost
filter sum_gross_cost > 200
take 20
"#,
        )?.nodes;

        let mat = resolve_and_materialize(ast).unwrap();
        assert_yaml_snapshot!(&mat);

        Ok(())
    }

    #[test]
    fn test_run_functions_args() -> Result<()> {
        let ast = parse(
            r#"
        func count x = s"count({x})"

        from employees
        aggregate [
        count salary
        ]
        "#,
        )?
        .nodes;

        assert_yaml_snapshot!(ast, @r###"
        ---
        - FuncDef:
            name: count
            kind: ~
            positional_params:
              - Ident: x
            named_params: []
            body:
              SString:
                - String: count(
                - Expr:
                    Ident: x
                - String: )
        - Pipeline:
            value: ~
            functions:
              - FuncCall:
                  name: from
                  args:
                    - Ident: employees
                  named_args: {}
              - FuncCall:
                  name: aggregate
                  args:
                    - List:
                        - FuncCall:
                            name: count
                            args:
                              - Ident: salary
                            named_args: {}
                  named_args: {}
        "###);

        let mat = resolve_and_materialize(ast.clone()).unwrap();

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        let diff = diff(&to_string(&ast)?, &to_string(&mat)?);
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -1,31 +1,19 @@
         ---
        -- FuncDef:
        -    name: count
        -    kind: ~
        -    positional_params:
        -      - Ident: x
        -    named_params: []
        -    body:
        -      SString:
        -        - String: count(
        -        - Expr:
        -            Ident: x
        -        - String: )
         - Pipeline:
             value: ~
             functions:
        -      - FuncCall:
        -          name: from
        -          args:
        -            - Ident: employees
        -          named_args: {}
        -      - FuncCall:
        -          name: aggregate
        -          args:
        -            - List:
        -                - FuncCall:
        -                    name: count
        -                    args:
        -                      - Ident: salary
        -                    named_args: {}
        -          named_args: {}
        +      - Transform:
        +          From:
        +            name: employees
        +            alias: ~
        +      - Transform:
        +          Aggregate:
        +            assigns:
        +              - SString:
        +                  - String: count(
        +                  - Expr:
        +                      Ident: salary
        +                  - String: )
        +            group: []
        +            window: ~
        +            sort: ~
        "###);

        Ok(())
    }

    #[test]
    fn test_run_functions_nested() -> Result<()> {
        let ast = parse(
            r#"
        func lag_day x = s"lag_day_todo({x})"
        func ret x dividend_return = x / (lag_day x) - 1 + dividend_return

        from a
        select (ret b c)
        "#,
        )?
        .nodes;

        assert_yaml_snapshot!(ast[2], @r###"
        ---
        Pipeline:
          value: ~
          functions:
            - FuncCall:
                name: from
                args:
                  - Ident: a
                named_args: {}
            - FuncCall:
                name: select
                args:
                  - FuncCall:
                      name: ret
                      args:
                        - Ident: b
                        - Ident: c
                      named_args: {}
                named_args: {}
        "###);

        let mat = resolve_and_materialize(ast).unwrap();
        assert_yaml_snapshot!(mat, @r###"
        ---
        - Pipeline:
            value: ~
            functions:
              - Transform:
                  From:
                    name: a
                    alias: ~
              - Transform:
                  Select:
                    assigns:
                      - Expr:
                          - Expr:
                              - Ident: b
                              - Raw: /
                              - SString:
                                  - String: lag_day_todo(
                                  - Expr:
                                      Ident: b
                                  - String: )
                          - Raw: "-"
                          - Raw: "1"
                          - Raw: +
                          - Ident: c
                    group: []
                    window: ~
                    sort: ~
        "###);

        Ok(())
    }

    #[test]
    fn test_run_inline_pipelines() -> Result<()> {
        let ast = parse(
            r#"
        func sum x = s"SUM({x})"

        from a
        aggregate [one: (foo | sum), two: (foo | sum)]
        "#,
        )?
        .nodes;

        let mat = resolve_and_materialize(ast.clone()).unwrap();

        assert_snapshot!(diff(&to_string(&ast)?, &to_string(&mat)?), @r###"
        @@ -1,48 +1,24 @@
         ---
        -- FuncDef:
        -    name: sum
        -    kind: ~
        -    positional_params:
        -      - Ident: x
        -    named_params: []
        -    body:
        -      SString:
        -        - String: SUM(
        -        - Expr:
        -            Ident: x
        -        - String: )
         - Pipeline:
             value: ~
             functions:
        -      - FuncCall:
        -          name: from
        -          args:
        -            - Ident: a
        -          named_args: {}
        -      - FuncCall:
        -          name: aggregate
        -          args:
        -            - List:
        -                - NamedExpr:
        -                    name: one
        -                    expr:
        -                      Pipeline:
        -                        value:
        -                          Ident: foo
        -                        functions:
        -                          - FuncCall:
        -                              name: sum
        -                              args: []
        -                              named_args: {}
        -                - NamedExpr:
        -                    name: two
        -                    expr:
        -                      Pipeline:
        -                        value:
        -                          Ident: foo
        -                        functions:
        -                          - FuncCall:
        -                              name: sum
        -                              args: []
        -                              named_args: {}
        -          named_args: {}
        +      - Transform:
        +          From:
        +            name: a
        +            alias: ~
        +      - Transform:
        +          Aggregate:
        +            assigns:
        +              - SString:
        +                  - String: SUM(
        +                  - Expr:
        +                      Ident: foo
        +                  - String: )
        +              - SString:
        +                  - String: SUM(
        +                  - Expr:
        +                      Ident: foo
        +                  - String: )
        +            group: []
        +            window: ~
        +            sort: ~
        "###);

        // Test it'll run the `sum foo` function first.
        let ast = parse(
            r#"
        func sum x = s"SUM({x})"
        func plus_one x = x + 1

        from a
        aggregate [a: (sum foo | plus_one)]
        "#,
        )?
        .nodes;

        let mat = resolve_and_materialize(ast).unwrap();

        assert_yaml_snapshot!(mat, @r###"
        ---
        - Pipeline:
            value: ~
            functions:
              - Transform:
                  From:
                    name: a
                    alias: ~
              - Transform:
                  Aggregate:
                    assigns:
                      - Expr:
                          - SString:
                              - String: SUM(
                              - Expr:
                                  Ident: foo
                              - String: )
                          - Raw: +
                          - Raw: "1"
                    group: []
                    window: ~
                    sort: ~
        "###);

        Ok(())
    }

    #[test]
    fn test_named_args() -> Result<()> {
        let ast = parse(
            r#"
        func add x to:1  = x + to

        from foo_table
        derive [
        added:         add bar to:3,
        added_default: add bar
        ]
        "#,
        )?
        .nodes;
        let mat = resolve_and_materialize(ast).unwrap();

        assert_yaml_snapshot!(mat, @r###"
        ---
        - Pipeline:
            value: ~
            functions:
              - Transform:
                  From:
                    name: foo_table
                    alias: ~
              - Transform:
                  Derive:
                    assigns:
                      - Expr:
                          - Ident: bar
                          - Raw: +
                          - Raw: "3"
                      - Expr:
                          - Ident: bar
                          - Raw: +
                          - Raw: "1"
                    group: []
                    window: ~
                    sort: ~
        "###);

        Ok(())
    }

    #[test]
    fn test_materialize_1() -> Result<()> {
        let ast = parse(
            r#"
        func count x = s"count({x})"

        from employees
        aggregate [
        count salary
        ]
        "#,
        )?
        .nodes;

        let mat = resolve_and_materialize(ast).unwrap();
        assert_yaml_snapshot!(mat,
            @r###"
        ---
        - Pipeline:
            value: ~
            functions:
              - Transform:
                  From:
                    name: employees
                    alias: ~
              - Transform:
                  Aggregate:
                    assigns:
                      - SString:
                          - String: count(
                          - Expr:
                              Ident: salary
                          - String: )
                    group: []
                    window: ~
                    sort: ~
        "###
        );
        Ok(())
    }

    #[test]
    fn test_materialize_2() -> Result<()> {
        let ast = parse(
            r#"
func count = s"COUNT(*)"
func average column = s"AVG({column})"
func sum column = s"SUM({column})"

from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost  : gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
    aggregate [                  # `by` are the columns to group by.
        average salary,                              # These are aggregation calcs run on each group.
        sum     salary,
        average gross_salary,
        sum     gross_salary,
        average gross_cost,
        sum_gross_cost: sum gross_cost,
        ct: count,
    ]
)
sort sum_gross_cost
filter sum_gross_cost > 200
take 20
"#,
        )?.nodes;

        let mat = resolve_and_materialize(ast).unwrap();
        assert_yaml_snapshot!(mat);
        Ok(())
    }

    #[test]
    fn test_materialize_3() -> Result<()> {
        let ast = parse(
            r#"
    func interest_rate = 0.2

    func lag_day x = s"lag_day_todo({x})"
    func ret x dividend_return = x / (lag_day x) - 1 + dividend_return
    func excess x = (x - interest_rate) / 252
    func if_valid x = s"IF(is_valid_price, {x}, NULL)"

    from prices
    derive [
      return_total     : if_valid (ret prices_adj div_ret),
      return_usd       : if_valid (ret prices_usd div_ret),
      return_excess    : excess return_total,
      return_usd_excess: excess return_usd,
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
        )?
        .nodes;
        let mat = resolve_and_materialize(ast).unwrap();
        assert_yaml_snapshot!(mat);

        Ok(())
    }

    #[test]
    fn test_variable_after_aggregate() -> Result<()> {
        let ast = parse(
            r#"
func average column = s"AVG({column})"

from employees
group [title, emp_no] (
    aggregate [emp_salary: average salary]
)
group [title] (
    aggregate [avg_salary: average emp_salary]
)
"#,
        )?
        .nodes;

        let mat = resolve_and_materialize(ast).unwrap();
        assert_yaml_snapshot!(mat, @r###"
        ---
        - Pipeline:
            value: ~
            functions:
              - Transform:
                  From:
                    name: employees
                    alias: ~
              - Transform:
                  Group:
                    by:
                      - Ident: title
                      - Ident: emp_no
                    pipeline:
                      Pipeline:
                        value: ~
                        functions:
                          - Transform:
                              Aggregate:
                                assigns:
                                  - SString:
                                      - String: AVG(
                                      - Expr:
                                          Ident: salary
                                      - String: )
                                group:
                                  - Ident: title
                                  - Ident: emp_no
                                window: ~
                                sort: ~
              - Transform:
                  Group:
                    by:
                      - Ident: title
                    pipeline:
                      Pipeline:
                        value: ~
                        functions:
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
                                group:
                                  - Ident: title
                                window: ~
                                sort: ~
        "###);

        Ok(())
    }
}
