/// Transform the parsed AST into a "materialized" AST, by executing functions and
/// replacing variables. The materialized AST is "flat", in the sense that it
/// contains no query-specific logic.
use super::ast::*;
use super::ast_fold::*;
use super::utils::*;
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
    fn run_inline_pipeline(&mut self, items: Items) -> Result<Item> {
        // TODO: Fold the first item; it could be a function call
        let mut items = items.into_iter().map(|x| x.into_expr().unwrap());
        let mut value = items
            .next()
            .ok_or_else(|| anyhow!("Expected at least one item"))?
            .into_only()?;
        for pipe_contents in items {
            // The value from the previous pipeline becomes the final arg.
            let args = [pipe_contents, vec![value]].concat();
            let func_call: FuncCall = args.try_into()?;
            value = self.run_function(&func_call)?;
        }
        Ok(value)
    }
    fn to_function_call(item: Item) -> Result<FuncCall> {
        match item {
            Item::Terms(terms) => terms.try_into(),
            Item::Ident(_) => Self::to_function_call(Item::Terms(item.into_inner_terms())),
            _ => Err(anyhow!(
                "FuncCalls can only be made from Terms or Ident; found {item:?}"
            )),
        }
    }
}

impl AstFold for RunFunctions {
    fn fold_function(&mut self, func: Function) -> Result<Function> {
        let out = fold_function(self, func.clone());
        // Add function to our list _after_ running it (no recursive functions atm).
        self.add_function(func);
        out
    }
    fn fold_terms(&mut self, terms: Items) -> Result<Items> {
        // If any of the terms are an Expr, then they may contain other function
        // calls (e.g. `foo (bar baz)` needs to run `bar baz`), which need to be
        // run before running this call. So fold those first.
        // If we just delegate to `fold_terms`, we get a stack overflow; would
        // be nice to clean this up a bit, but need to resolve that.
        let terms = terms
            .into_iter()
            .map(|term| match term {
                Item::Expr(_) => self.fold_item(term),
                _ => Ok(term),
            })
            .collect::<Result<Vec<Item>>>()
            .map(Item::Terms)?;

        if let Ok(func_call) = Self::to_function_call(terms.clone()) {
            if self.functions.get(&func_call.name).is_some() {
                let function_result = self.run_function(&func_call)?;

                // It's important to fold the result of the function call, so
                // the functions get executed transitively.
                return Ok(self.fold_item(function_result)?.into_inner_terms());
            }
        }
        Ok(terms.into_terms()?)
    }
    fn fold_item(&mut self, item: Item) -> Result<Item> {
        let item = match item.clone() {
            // If it's an ident, try running it as a function, in case it's a
            // function with no args.
            Item::Ident(_) => Item::Terms(self.fold_terms(vec![item])?).into_unnested(),
            // We replace the InlinePipeline with an Item, so we need to run it
            // here rather than in `fold_inline_pipeline`.
            Item::InlinePipeline(items) => self.run_inline_pipeline(items)?,
            // Otherwise just delegate to the upstream fold.
            _ => item,
        };
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
        let diff = TextDiff::from_lines(&to_string(&ast)?, &to_string(&fold.fold_item(ast)?)?)
            .unified_diff()
            .to_string();
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -17,6 +17,9 @@
                     by: []
                     calcs:
                       - Terms:
        -                  - Ident: count
        -                  - Ident: salary
        +                  - SString:
        +                      - String: count(
        +                      - Expr:
        +                          Ident: salary
        +                      - String: )
                     assigns: []
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
          - From: a
          - Select:
              - Expr:
                  - Terms:
                      - Ident: ret
                      - Ident: b
        "###);

        assert_yaml_snapshot!(materialize(ast)?.into_query()?.items[2], @r###"
        ---
        Pipeline:
          - From: a
          - Select:
              - Expr:
                  - Terms:
                      - Ident: b
                      - Raw: /
                      - Expr:
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

        assert_yaml_snapshot!(ast, @r###"
        ---
        Query:
          items:
            - Function:
                name: sum
                args:
                  - x
                body:
                  - SString:
                      - String: SUM(
                      - Expr:
                          Ident: x
                      - String: )
            - Pipeline:
                - From: a
                - Aggregate:
                    by: []
                    calcs: []
                    assigns:
                      - lvalue: one
                        rvalue:
                          InlinePipeline:
                            - Expr:
                                - Ident: foo
                            - Expr:
                                - Ident: sum
                      - lvalue: two
                        rvalue:
                          InlinePipeline:
                            - Expr:
                                - Ident: foo
                            - Expr:
                                - Ident: sum
        "###);

        let mut run_functions = RunFunctions::new();
        assert_yaml_snapshot!(run_functions.fold_item(ast)?, @r###"
        ---
        Query:
          items:
            - Function:
                name: sum
                args:
                  - x
                body:
                  - SString:
                      - String: SUM(
                      - Expr:
                          Ident: x
                      - String: )
            - Pipeline:
                - From: a
                - Aggregate:
                    by: []
                    calcs: []
                    assigns:
                      - lvalue: one
                        rvalue:
                          SString:
                            - String: SUM(
                            - Expr:
                                Ident: foo
                            - String: )
                      - lvalue: two
                        rvalue:
                          SString:
                            - String: SUM(
                            - Expr:
                                Ident: foo
                            - String: )
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
