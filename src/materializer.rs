/// Transform the parsed AST into a "materialized" AST, by executing functions and
/// replacing variables. The materialized AST is "flat", in the sense that it
/// contains no query-specific logic.
use super::ast::*;
use super::ast_fold::*;
use anyhow::{anyhow, Result};
use std::{collections::HashMap, iter::zip};

pub fn materialize(ast: Item) -> Result<Item> {
    let functions = load_std_lib()?;
    let mut run_functions = RunFunctions::new();
    functions.into_iter().for_each(|f| {
        run_functions.add_function(f.into_function().unwrap());
    });
    let mut replace_variables = ReplaceVariables::new();
    // TODO: is it always OK to run these serially?
    let ast = run_functions.fold_item(ast)?;
    let ast = replace_variables.fold_item(ast)?;
    Ok(ast)
}

fn load_std_lib() -> Result<Items> {
    use super::parse;
    let std_lib = include_str!("stdlib.prql");
    Ok(parse(std_lib)?.into_inner_items())
}

struct ReplaceVariables {
    variables: HashMap<Ident, Item>,
}

impl ReplaceVariables {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
    fn add_variables(&mut self, assign: Assign) -> &Self {
        self.variables.insert(assign.lvalue, *assign.rvalue);
        self
    }
}

impl AstFold for ReplaceVariables {
    fn fold_assign(&mut self, assign: Assign) -> Result<Assign> {
        let replaced_assign = fold_assign(self, assign)?;
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
        }
        .into_unnested())
    }
}

#[derive(Debug)]
struct RunFunctions {
    // This stores the name twice, but that's probably OK.
    functions: HashMap<Ident, Function>,
}

impl RunFunctions {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    fn add_function(&mut self, func: Function) -> &Self {
        self.functions.insert(func.name.clone(), func);
        self
    }
    fn run_function(&mut self, func_call: &FuncCall) -> Result<Item> {
        // Get the function
        let func = self
            .functions
            .get(&func_call.name)
            .ok_or_else(|| anyhow!("Function {:?} not found", func_call.name))?;

        // TODO: check if the function is called recursively.

        if func.args.len() != func_call.args.len() {
            return Err(anyhow!(
                "Function {:?} called with wrong number of arguments. Expected {}, got {}; from {func_call:?}.",
                func_call.name,
                func.args.len(),
                func_call.args.len()
            ));
        }
        // Make a ReplaceVariables fold which we'll use to replace the variables
        // in the function with their argument values.
        let mut replace_variables = ReplaceVariables::new();
        zip(func.args.iter(), func_call.args.iter()).for_each(|(arg, arg_call)| {
            replace_variables.add_variables(Assign {
                lvalue: arg.clone(),
                rvalue: Box::new(arg_call.clone()),
            });
        });
        // Take a clone of the body and replace the arguments with their values.
        Ok(Item::Terms(replace_variables.fold_items(func.body.clone())?).into_unnested())
    }
}

impl AstFold for RunFunctions {
    fn fold_function(&mut self, func: Function) -> Result<Function> {
        let out = fold_function(self, func.clone());
        // Add function to our list, after running it (no recursive functions atm).
        self.add_function(func);
        out
    }
    fn fold_item(&mut self, item: Item) -> Result<Item> {
        // If it's an ident, it could be a func with no arg, so normalize into a
        // vec of items whether or not it's currently wrapped in a Terms or not.
        let items = item.clone().into_inner_terms();

        if let Some(Item::Ident(ident)) = items.first() {
            if self.functions.get(ident).is_some() {
                // Currently a transformation expects a Expr to wrap
                // all the terms after the name. TODO: another area
                // that's messy, should we parse a FuncCall directly?
                let (name, body) = items.split_first().unwrap();
                let func_call_transform =
                    vec![name.clone(), Item::Expr(body.to_vec())].try_into()?;

                if let Transformation::Func(func_call) = func_call_transform {
                    return self.run_function(&func_call);
                } else {
                    unreachable!()
                }
            }
        }
        fold_item(self, item)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::parse;
    use insta::{assert_display_snapshot, assert_yaml_snapshot};

    #[test]
    fn test_replace_variables() -> Result<()> {
        use super::*;
        use serde_yaml::to_string;
        use similar::TextDiff;

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
        assert_display_snapshot!(TextDiff::from_lines(
            &to_string(&ast)?,
            &to_string(&fold.fold_item(ast)?)?
        ).unified_diff(),
        @r###"
        @@ -13,6 +13,9 @@
                     - lvalue: gross_cost
                       rvalue:
                         Terms:
        -                  - Ident: gross_salary
        +                  - Terms:
        +                      - Ident: salary
        +                      - Raw: +
        +                      - Ident: payroll_tax
                           - Raw: +
                           - Ident: benefits_cost
        "###);

        let mut fold = ReplaceVariables::new();
        let ast = parse(
            r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    count: count,
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
    fn test_run_functions_no_arg() -> Result<()> {
        let ast = parse(
            "
func count = testing_count

from employees
aggregate [
  count
]
",
        )?;

        assert_yaml_snapshot!(ast, @r###"
        ---
        Query:
          items:
            - Function:
                name: count
                args: []
                body:
                  - Ident: testing_count
            - Pipeline:
                - From: employees
                - Aggregate:
                    by: []
                    calcs:
                      - Ident: count
                    assigns: []
        "###);

        use serde_yaml::to_string;
        use similar::TextDiff;

        let mut fold = RunFunctions::new();
        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        let diff = TextDiff::from_lines(
            &to_string(&ast).unwrap(),
            &to_string(&fold.fold_item(ast).unwrap()).unwrap(),
        )
        .unified_diff()
        .to_string();
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -11,5 +11,5 @@
                 - Aggregate:
                     by: []
                     calcs:
        -              - Ident: count
        +              - Ident: testing_count
                     assigns: []
        "###);

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
            - Function:
                name: count
                args:
                  - x
                body:
                  - SString:
                      - String: count(
                      - Expr:
                          Ident: x
                      - String: )
            - Pipeline:
                - From: employees
                - Aggregate:
                    by: []
                    calcs:
                      - Terms:
                          - Ident: count
                          - Ident: salary
                    assigns: []
        "###);

        use serde_yaml::to_string;
        use similar::TextDiff;

        let mut fold = RunFunctions::new();
        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        let diff = TextDiff::from_lines(
            &to_string(&ast).unwrap(),
            &to_string(&fold.fold_item(ast).unwrap()).unwrap(),
        )
        .unified_diff()
        .to_string();
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -16,7 +16,9 @@
                 - Aggregate:
                     by: []
                     calcs:
        -              - Terms:
        -                  - Ident: count
        -                  - Ident: salary
        +              - SString:
        +                  - String: count(
        +                  - Expr:
        +                      Ident: salary
        +                  - String: )
                     assigns: []
        "###);

        Ok(())
    }

    #[test]
    fn test_materialize() -> Result<()> {
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
            - Function:
                name: count
                args:
                  - x
                body:
                  - SString:
                      - String: count(
                      - Expr:
                          Ident: x
                      - String: )
            - Pipeline:
                - From: employees
                - Aggregate:
                    by: []
                    calcs:
                      - SString:
                          - String: count(
                          - Expr:
                              Ident: salary
                          - String: )
                    assigns: []
        "###
        );

        let ast = parse(
            r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    count: count,
]
sort sum_gross_cost
filter sum_gross_cost > 200
take 20
"#,
        )?;
        assert_yaml_snapshot!(materialize(ast)?);

        let ast = parse(
            r#"
    func lag_day x = s"lag_day_todo({x})"
    func ret x = x / (lag_day x) - 1 + dividend_return
    func excess x = (x - interest_rate) / 252
    func if_valid x = s"IF(is_valid_price, {x}, NULL)"

    from prices
    derive [
      return_total:      if_valid (ret prices_adj),
      return_usd:        if_valid (ret prices_usd),
      return_excess:     excess return_total,
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
}
