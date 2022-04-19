//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use crate::ast::ast_fold::*;
use crate::ast::*;

use anyhow::{anyhow, Result};
use itertools::zip;
use itertools::Itertools;

use crate::semantic::{split_var_name, Context, TableColumn};

pub struct MaterializedFrame {
    pub columns: Vec<Node>,
    pub sort: Vec<ColumnSort<Ident>>,
}

/// Replaces all resolved functions and variables with their declarations.
pub fn materialize(
    nodes: Vec<Node>,
    context: Context,
) -> Result<(Vec<Node>, Context, MaterializedFrame)> {
    let mut counter = TableCounter::default();
    let nodes = counter.fold_nodes(nodes).unwrap();

    let mut m = Materializer {
        context,
        remove_namespaces: counter.tables == 1,
    };

    // materialize the query
    let nodes = m.fold_nodes(nodes)?;

    // materialize each of the columns
    let columns = m.materialize_columns()?;

    // materialize each of the columns
    let sort = m.lookup_sort()?;

    Ok((nodes, m.context, MaterializedFrame { columns, sort }))
}

/// Can fold (walk) over AST and replace function calls and variable references with their declarations.
pub struct Materializer {
    pub context: Context,
    pub remove_namespaces: bool,
}

impl Materializer {
    /// Folds columns and returns expressions that can be used in select.
    /// Replaces declarations of each column with an identifier.
    fn materialize_columns(&mut self) -> Result<Vec<Node>> {
        // This has to be done in two stages, because some of the declarations that
        // have to be replaced may appear in other columns, where original declaration
        // is needed.
        // For example:
        // derive b: a + 1
        // select [b, b + 1]

        // materialize each column
        let res: Vec<_> = (self.context.frame.columns.clone().iter())
            .map(|column| match column {
                TableColumn::Declared(column_id) => self.materialize_column(*column_id),
                TableColumn::All(namespace) => {
                    Ok((Item::Ident(format!("{namespace}.*")).into(), None))
                }
            })
            .try_collect()?;

        // replace declarations
        let res = res
            .into_iter()
            .map(|(node, ident)| {
                if let Some((id, ident)) = ident {
                    self.context
                        .replace_declaration(id, Item::Ident(ident).into());
                }

                node
            })
            .collect();
        Ok(res)
    }

    /// Folds the column and returns expression that can be used in select.
    /// Also returns column id and name if declaration should be replaced.
    fn materialize_column(&mut self, column_id: usize) -> Result<(Node, Option<(usize, String)>)> {
        let decl = self.context.declarations[column_id].0.clone();

        // find the new column name (with namespace removed)
        let ident = (decl.as_name()).map(|i| split_var_name(i.as_str()).1.to_string());

        let node = decl.into_expr_node().unwrap();

        // materialize
        let expr_node = self.fold_node(*node)?;
        let expr_ident = expr_node.item.as_ident().map(|n| split_var_name(n).1);

        Ok(if let Some(ident) = ident {
            // is expr_node just an ident with same name?
            let expr_node = if expr_ident.map(|n| n == ident).unwrap_or(false) {
                // return just the ident
                expr_node
            } else {
                // return expr with new name
                Item::NamedExpr(NamedExpr {
                    expr: Box::new(expr_node),
                    name: ident.clone(),
                })
                .into()
            };
            (expr_node, Some((column_id, ident)))
        } else {
            // column is not named, just return its expression
            (expr_node, None)
        })
    }

    /// Folds the column and returns expression that can be used in select.
    /// Also returns column id and name if declaration should be replaced.
    fn lookup_sort(&mut self) -> Result<Vec<ColumnSort<Ident>>> {
        let sort = &self.context.frame.sort;

        sort.iter()
            .cloned()
            .map(|s| {
                let decl = self.context.declarations[s.column].0.clone();
                let column = decl
                    .as_name()
                    .ok_or_else(|| anyhow!("unnamed sort column?"))?
                    .clone();

                Ok(ColumnSort {
                    column,
                    direction: s.direction,
                })
            })
            .try_collect()
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
                .map_or_else(|| (*param.expr).clone(), |expr| *(*expr).clone());

            self.context.replace_declaration(id, value);
        }
        for (param, arg) in zip(func_dec.positional_params.iter(), func_call.args.iter()) {
            self.context
                .replace_declaration(param.declared_at.unwrap(), arg.clone());
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
                let node = if let Some(id) = node.declared_at {
                    let (decl, _) = &self.context.declarations[id];

                    let new_node = *decl.clone().into_expr_node()?;
                    self.fold_node(new_node)?
                } else {
                    node
                };
                if self.remove_namespaces {
                    remove_namespace(node)
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

fn remove_namespace(mut node: Node) -> Node {
    node.item = match node.item {
        Item::Ident(ident) => {
            let (_, variable) = split_var_name(&ident);
            Item::Ident(variable.to_string())
        }
        i => i,
    };
    node
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
    use crate::{parse, semantic::resolve, sql::resolve_and_materialize, utils::diff};
    use insta::{assert_display_snapshot, assert_snapshot, assert_yaml_snapshot};
    use serde_yaml::to_string;

    #[test]
    fn test_replace_variables_1() -> Result<()> {
        let ast = parse(
            r#"from employees
    derive [                                         # This adds columns / variables.
      gross_salary: salary + payroll_tax,
      gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
    ]
    "#,
        )?;

        let (res, context) = resolve(ast.nodes, None)?;
        let (mat, _, _) = materialize(res.clone(), context)?;

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(diff(
            &to_string(&res)?,
            &to_string(&mat)?
        ),
        @r###"
        @@ -19,6 +19,9 @@
                         name: gross_cost
                         expr:
                           Expr:
        -                    - Ident: gross_salary
        +                    - Expr:
        +                        - Ident: salary
        +                        - Raw: +
        +                        - Ident: payroll_tax
                             - Raw: +
                             - Ident: benefits_cost
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
        )?;
        let (res, context) = resolve(ast.nodes, None)?;
        let (mat, _, _) = materialize(res, context)?;
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
        )?;

        assert_yaml_snapshot!(ast, @r###"
        ---
        version: ~
        dialect: Generic
        nodes:
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

        let (mat, _, _) = resolve_and_materialize(ast.nodes.clone(), None)?;

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        let diff = diff(&to_string(&ast)?, &to_string(&mat)?);
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -1,34 +1,15 @@
         ---
        -version: ~
        -dialect: Generic
        -nodes:
        -  - FuncDef:
        -      name: count
        -      kind: ~
        -      positional_params:
        -        - Ident: x
        -      named_params: []
        -      body:
        -        SString:
        -          - String: count(
        -          - Expr:
        -              Ident: x
        -          - String: )
        -  - Pipeline:
        -      value: ~
        -      functions:
        -        - FuncCall:
        -            name: from
        -            args:
        -              - Ident: employees
        -            named_args: {}
        -        - FuncCall:
        -            name: aggregate
        -            args:
        -              - List:
        -                  - FuncCall:
        -                      name: count
        -                      args:
        -                        - Ident: salary
        -                      named_args: {}
        -            named_args: {}
        +- Pipeline:
        +    value: ~
        +    functions:
        +      - Transform:
        +          From:
        +            name: employees
        +            alias: ~
        +      - Transform:
        +          Aggregate:
        +            - SString:
        +                - String: count(
        +                - Expr:
        +                    Ident: salary
        +                - String: )
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
        )?;

        assert_yaml_snapshot!(ast.nodes[2], @r###"
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

        let (mat, _, _) = resolve_and_materialize(ast.nodes, None)?;
        assert_yaml_snapshot!(mat[0], @r###"
        ---
        Pipeline:
          value: ~
          functions:
            - Transform:
                From:
                  name: a
                  alias: ~
            - Transform:
                Select:
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
        )?;

        let (res, context) = resolve(ast.nodes, None)?;
        let (mat, _, _) = materialize(res.clone(), context)?;

        assert_snapshot!(diff(&to_string(&res)?, &to_string(&mat)?), @r###"
        @@ -11,22 +11,16 @@
                     - NamedExpr:
                         name: one
                         expr:
        -                  Pipeline:
        -                    value:
        -                      Ident: foo
        -                    functions:
        -                      - FuncCall:
        -                          name: sum
        -                          args: []
        -                          named_args: {}
        +                  SString:
        +                    - String: SUM(
        +                    - Expr:
        +                        Ident: foo
        +                    - String: )
                     - NamedExpr:
                         name: two
                         expr:
        -                  Pipeline:
        -                    value:
        -                      Ident: foo
        -                    functions:
        -                      - FuncCall:
        -                          name: sum
        -                          args: []
        -                          named_args: {}
        +                  SString:
        +                    - String: SUM(
        +                    - Expr:
        +                        Ident: foo
        +                    - String: )
        "###);

        // Test it'll run the `sum foo` function first.
        let ast = parse(
            r#"
func sum x = s"SUM({x})"
func plus_one x = x + 1

from a
aggregate [a: (sum foo | plus_one)]
"#,
        )?;

        let (mat, _, _) = resolve_and_materialize(ast.nodes, None)?;

        assert_yaml_snapshot!(mat[0], @r###"
        ---
        Pipeline:
          value: ~
          functions:
            - Transform:
                From:
                  name: a
                  alias: ~
            - Transform:
                Aggregate:
                  - NamedExpr:
                      name: a
                      expr:
                        Expr:
                          - SString:
                              - String: SUM(
                              - Expr:
                                  Ident: foo
                              - String: )
                          - Raw: +
                          - Raw: "1"
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
        )?;
        let (mat, _, _) = resolve_and_materialize(ast.nodes, None)?;

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
                    - NamedExpr:
                        name: added
                        expr:
                          Expr:
                            - Ident: bar
                            - Raw: +
                            - Raw: "3"
                    - NamedExpr:
                        name: added_default
                        expr:
                          Expr:
                            - Ident: bar
                            - Raw: +
                            - Raw: "1"
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
        )?;

        let (mat, _, _) = resolve_and_materialize(ast.nodes, None)?;
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
                    - SString:
                        - String: count(
                        - Expr:
                            Ident: salary
                        - String: )
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
        )?;

        let (mat, _, _) = resolve_and_materialize(ast.nodes, None)?;
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
        )?;
        let (mat, _, _) = resolve_and_materialize(ast.nodes, None)?;
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
        )?;

        let (mat, _, _) = resolve_and_materialize(ast.nodes, None)?;
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
                                - NamedExpr:
                                    name: emp_salary
                                    expr:
                                      SString:
                                        - String: AVG(
                                        - Expr:
                                            Ident: salary
                                        - String: )
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
                                - NamedExpr:
                                    name: avg_salary
                                    expr:
                                      SString:
                                        - String: AVG(
                                        - Expr:
                                            SString:
                                              - String: AVG(
                                              - Expr:
                                                  Ident: salary
                                              - String: )
                                        - String: )
        "###);

        Ok(())
    }
}
