//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use super::ast::*;
use super::ast_fold::*;

use anyhow::Context;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use std::{collections::HashMap, iter::zip};

/// "Flatten" a PRQL AST by running functions & replacing variables.
pub fn materialize(ast: Item) -> Result<Item> {
    let functions = load_std_lib()?;
    let mut run_functions = RunFunctions::new();
    functions.into_iter().for_each(|f| {
        run_functions.add_def(f.into_func_def().unwrap());
    });
    let mut replace_variables = ReplaceVariables::new();
    // TODO: is it always OK to run these serially?
    let ast = run_functions.fold_item(ast)?;
    let ast = replace_variables.fold_item(ast)?;
    Ok(ast)
}

fn load_std_lib() -> Result<Vec<Item>> {
    use super::parse;
    let std_lib = include_str!("stdlib.prql");
    Ok(parse(std_lib)?.into_query()?.items)
}

/// Holds currently known variables and their values.
/// Can fold (walk) over AST and replace variables with their names.
struct ReplaceVariables {
    variables: HashMap<Ident, Item>,
}

impl ReplaceVariables {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
    fn add_variables(&mut self, expr: NamedExpr) -> &Self {
        self.variables.insert(expr.name, *expr.expr);
        self
    }
}

impl AstFold for ReplaceVariables {
    fn fold_named_expr(&mut self, assign: NamedExpr) -> Result<NamedExpr> {
        let replaced_assign = fold_named_expr(self, assign)?;
        self.add_variables(replaced_assign.clone());
        Ok(replaced_assign)
    }
    fn fold_item(&mut self, item: Item) -> Result<Item> {
        Ok(match item {
            // Because this returns an Item rather than an Ident, we need to
            // have a custom `fold_item` method; a custom `fold_ident` method
            // wouldn't return the correct type.
            Item::Ident(ident) => {
                if self.variables.contains_key(ident.as_str()) {
                    self.variables[ident.as_str()].clone()
                } else {
                    Item::Ident(ident)
                }
            }
            _ => fold_item(self, item)?,
        })
    }
    // Once we get to an Aggregate, we want to run the replacement, but then we
    // want to remove the variable, because SQL can support it from then on. If
    // we don't do this, we get errors like `AVG(AVG(x))` in later CTEs; see #213.
    fn fold_transformation(&mut self, transformation: Transformation) -> Result<Transformation> {
        let out = fold_transformation(self, transformation.clone());

        if let Transformation::Aggregate { select, .. } = transformation {
            for item in select.iter() {
                if let Some(named) = item.as_named_expr() {
                    self.variables.remove(&named.name);
                }
            }
        }
        out
    }
}

/// Holds currently known functions.
/// Can fold (walk) over AST and replace function calls with their declared body.
#[derive(Debug)]
struct RunFunctions {
    // This stores the name twice, but that's probably OK.
    functions: HashMap<Ident, FuncDef>,

    functions_no_args: HashMap<Ident, FuncDef>,
}

impl RunFunctions {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            functions_no_args: HashMap::new(),
        }
    }

    fn add_def(&mut self, func: FuncDef) -> &Self {
        if func.named_params.is_empty() && func.positional_params.is_empty() {
            self.functions_no_args.insert(func.name.clone(), func);
        } else {
            self.functions.insert(func.name.clone(), func);
        }
        self
    }
    fn inline_func_call(&mut self, func_call: &FuncCall) -> Result<Item> {
        // Get the function
        let func = self
            .functions
            .get(&func_call.name)
            .ok_or_else(|| anyhow!("Function {:?} not found", func_call.name))?;

        // TODO: check if the function is called recursively.

        if func.positional_params.len() != func_call.args.len() {
            return Err(anyhow!(
                "Function {:?} called with wrong number of arguments. Expected {}, got {}; from {func_call:?}.",
                func_call.name,
                func.positional_params.len(),
                func_call.args.len()
            ));
        }
        let named_args = func.named_params.iter().map(|param| {
            let value = func_call
                .named_args
                .iter()
                // Quite inefficient when number of arguments > 10. We could instead use merge join.
                .find(|named_arg| named_arg.name == param.name)
                // Put the value of the named arg if it's there; otherwise use
                // the default (which is sorted on `param.arg`).
                .map_or_else(
                    || (*param.expr).clone(),
                    |named_arg| *(named_arg.expr).clone(),
                );

            NamedExpr {
                name: param.name.clone(),
                expr: Box::new(value),
            }
        });

        // Make a ReplaceVariables fold which we'll use to replace the variables
        // in the function with their argument values.
        let mut replace_variables = ReplaceVariables::new();
        for (arg, arg_call) in zip(func.positional_params.iter(), func_call.args.iter()) {
            replace_variables.add_variables(NamedExpr {
                name: arg.clone(),
                expr: Box::new(arg_call.clone()),
            });
        }
        for arg in named_args {
            replace_variables.add_variables(arg);
        }
        // Take a clone of the function call's body, replace the variables with their
        // values, and return the modified function call.
        replace_variables.fold_item(*func.body.clone())
    }

    fn inline_pipeline(&mut self, pipeline: InlinePipeline) -> Result<Item> {
        let mut value = self.fold_item(*pipeline.value)?;

        for mut func_call in pipeline.functions {
            // The value from the previous pipeline becomes the final arg.
            func_call.args.push(value);

            value = self.inline_func_call(&func_call)?;
        }
        Ok(value)
    }
}

impl AstFold for RunFunctions {
    fn fold_func_call(&mut self, func_call: FuncCall) -> Result<Item> {
        // fold arguments
        let func_call = FuncCall {
            name: func_call.name,
            args: func_call
                .args
                .into_iter()
                .map(|x| self.fold_item(x))
                .try_collect()?,
            named_args: func_call
                .named_args
                .into_iter()
                .map(|x| self.fold_named_expr(x))
                .try_collect()?,
        };

        (self.functions.get(&func_call.name))
            .context(format!("Unknown function {}", func_call.name))?;

        self.inline_func_call(&func_call)
    }

    fn fold_items(&mut self, items: Vec<Item>) -> Result<Vec<Item>> {
        // We cut out function def, so we need to run it
        // here rather than in `fold_func_def`.
        let mut r = Vec::with_capacity(items.len());

        for item in items {
            match item {
                Item::FuncDef(func_def) => {
                    let func_def = fold_func_def(self, func_def)?;

                    self.add_def(func_def);
                }
                _ => r.push(self.fold_item(item)?),
            }
        }
        Ok(r)
    }

    fn fold_item(&mut self, item: Item) -> Result<Item> {
        match item.clone() {
            // We replace the InlinePipeline with an Item, so we need to run it
            // here rather than in `fold_inline_pipeline`.
            Item::InlinePipeline(p) => self.inline_pipeline(p),

            // Because this returns an Item rather than an Ident, we need to
            // have a custom `fold_item` method.
            Item::Ident(ident) => Ok(
                if let Some(def) = self.functions_no_args.get(ident.as_str()) {
                    *def.body.clone()
                } else {
                    Item::Ident(ident)
                },
            ),

            // Otherwise just delegate to the upstream fold.
            _ => fold_item(self, item),
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::{parse, utils::diff};
    use insta::{assert_display_snapshot, assert_snapshot, assert_yaml_snapshot};
    use itertools::Itertools;
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

        let mut fold = ReplaceVariables::new();
        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(diff(
            &to_string(&ast)?,
            &to_string(&fold.fold_item(ast)?)?
        ),
        @r###"
        @@ -17,6 +17,9 @@
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
        let mut fold = ReplaceVariables::new();
        let ast = parse(
            r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost  : gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    ct: count,
]
sort sum_gross_cost
filter sum_gross_cost > 200
take 20
"#,
        )?;
        assert_yaml_snapshot!(&fold.fold_item(ast)?);

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
        Query:
          items:
            - FuncDef:
                name: count
                positional_params:
                  - x
                named_params: []
                body:
                  SString:
                    - String: count(
                    - Expr:
                        Ident: x
                    - String: )
            - Pipeline:
                - From:
                    name: employees
                    alias: ~
                - Aggregate:
                    by: []
                    select:
                      - FuncCall:
                          name: count
                          args:
                            - Ident: salary
                          named_args: []
        "###);

        let mut fold = RunFunctions::new();
        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        let diff = diff(&to_string(&ast)?, &to_string(&fold.fold_item(ast)?)?);
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -1,17 +1,6 @@
         ---
         Query:
           items:
        -    - FuncDef:
        -        name: count
        -        positional_params:
        -          - x
        -        named_params: []
        -        body:
        -          SString:
        -            - String: count(
        -            - Expr:
        -                Ident: x
        -            - String: )
             - Pipeline:
                 - From:
                     name: employees
        @@ -19,8 +8,8 @@
                 - Aggregate:
                     by: []
                     select:
        -              - FuncCall:
        -                  name: count
        -                  args:
        -                    - Ident: salary
        -                  named_args: []
        +              - SString:
        +                  - String: count(
        +                  - Expr:
        +                      Ident: salary
        +                  - String: )
        "###);

        Ok(())
    }

    #[test]
    fn test_run_functions_nested() -> Result<()> {
        let ast = parse(
            r#"
func lag_day x = s"lag_day_todo({x})"
func ret x = x / (lag_day x) - 1 + dividend_return

from a
select (ret b)
"#,
        )?;

        assert_yaml_snapshot!(ast.clone().into_query()?.items[2], @r###"
        ---
        Pipeline:
          - From:
              name: a
              alias: ~
          - Select:
              - FuncCall:
                  name: ret
                  args:
                    - Ident: b
                  named_args: []
        "###);

        assert_yaml_snapshot!(materialize(ast)?.into_query()?.items[0], @r###"
        ---
        Pipeline:
          - From:
              name: a
              alias: ~
          - Select:
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
                  - Ident: dividend_return
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

        let mut run_functions = RunFunctions::new();
        assert_snapshot!(diff(&to_string(&ast)?, &to_string(&run_functions.fold_item(ast)?)?), @r###"
        @@ -1,17 +1,6 @@
         ---
         Query:
           items:
        -    - FuncDef:
        -        name: sum
        -        positional_params:
        -          - x
        -        named_params: []
        -        body:
        -          SString:
        -            - String: SUM(
        -            - Expr:
        -                Ident: x
        -            - String: )
             - Pipeline:
                 - From:
                     name: a
        @@ -22,20 +11,16 @@
                       - NamedExpr:
                           name: one
                           expr:
        -                    InlinePipeline:
        -                      value:
        -                        Ident: foo
        -                      functions:
        -                        - name: sum
        -                          args: []
        -                          named_args: []
        +                    SString:
        +                      - String: SUM(
        +                      - Expr:
        +                          Ident: foo
        +                      - String: )
                       - NamedExpr:
                           name: two
                           expr:
        -                    InlinePipeline:
        -                      value:
        -                        Ident: foo
        -                      functions:
        -                        - name: sum
        -                          args: []
        -                          named_args: []
        +                    SString:
        +                      - String: SUM(
        +                      - Expr:
        +                          Ident: foo
        +                      - String: )
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

        assert_yaml_snapshot!(materialize(ast)?.into_query()?.items[0], @r###"
        ---
        Pipeline:
          - From:
              name: a
              alias: ~
          - Aggregate:
              by: []
              select:
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
        assert_yaml_snapshot!(
        materialize(ast)?
            .into_query()?
            .items
            .iter()
            .filter_map(|x| x.as_pipeline())
            .collect_vec(), @r###"
        ---
        - - From:
              name: foo_table
              alias: ~
          - Derive:
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
        let pipeline = parse(
            r#"
func count x = s"count({x})"

from employees
aggregate [
  count salary
]
"#,
        )?;
        let ast = materialize(pipeline)?;
        assert_yaml_snapshot!(ast,
            @r###"
        ---
        Query:
          items:
            - Pipeline:
                - From:
                    name: employees
                    alias: ~
                - Aggregate:
                    by: []
                    select:
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
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost  : gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    ct: count,
]
sort sum_gross_cost
filter sum_gross_cost > 200
take 20
"#,
        )?;
        assert_yaml_snapshot!(materialize(ast)?);
        Ok(())
    }

    #[test]
    fn test_materialize_3() -> Result<()> {
        let ast = parse(
            r#"
    func lag_day x = s"lag_day_todo({x})"
    func ret x = x / (lag_day x) - 1 + dividend_return
    func excess x = (x - interest_rate) / 252
    func if_valid x = s"IF(is_valid_price, {x}, NULL)"

    from prices
    derive [
      return_total     : if_valid (ret prices_adj),
      return_usd       : if_valid (ret prices_usd),
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
        assert_yaml_snapshot!(materialize(ast)?);

        Ok(())
    }

    #[test]
    fn test_variable_after_aggregate() -> Result<()> {
        let ast = parse(
            r#"
from employees
aggregate by:[emp_no] [
  emp_salary: average salary
]
aggregate by:[title] [
  avg_salary: average emp_salary
]
"#,
        )?;

        let materialized = materialize(ast)?;

        assert_yaml_snapshot!(materialized, @r###"
        ---
        Query:
          items:
            - Pipeline:
                - From:
                    name: employees
                    alias: ~
                - Aggregate:
                    by:
                      - Ident: emp_no
                    select:
                      - NamedExpr:
                          name: emp_salary
                          expr:
                            SString:
                              - String: AVG(
                              - Expr:
                                  Ident: salary
                              - String: )
                - Aggregate:
                    by:
                      - Ident: title
                    select:
                      - NamedExpr:
                          name: avg_salary
                          expr:
                            SString:
                              - String: AVG(
                              - Expr:
                                  Ident: emp_salary
                              - String: )
        "###);

        Ok(())
    }
}
