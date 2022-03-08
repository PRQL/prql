use super::ast::*;
use super::ast_fold::*;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use std::{collections::HashMap, iter::zip};

pub fn compile(ast: Item) -> Result<Item> {
    let functions = load_std_lib()?;
    let mut run_functions = RunFunctions::new();
    functions.into_iter().for_each(|f| {
        run_functions.add_function(&f.into_function().unwrap());
    });
    let mut replace_variables = ReplaceVariables::new();
    // TODO: is it always OK to run these serially?
    let ast = run_functions.fold_item(&ast)?;
    let ast = replace_variables.fold_item(&ast)?;
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
    // Clippy is fine with this (correctly), but rust-analyzer is not (incorrectly).
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
    fn add_variables(&mut self, assign: &Assign) -> &Self {
        // Not sure we're choosing the correct Item / Items in the types, this is a
        // bit of a smell.
        self.variables
            .insert(assign.lvalue.clone(), *(assign.rvalue).clone());
        self
    }
}

impl AstFold for ReplaceVariables {
    fn fold_assign(&mut self, assign: &Assign) -> Result<Assign> {
        let replaced_assign = fold_assign(self, assign)?;
        self.add_variables(&replaced_assign);
        Ok(replaced_assign)
    }
    fn fold_item(&mut self, item: &Item) -> Result<Item> {
        Ok(match item {
            // Because this returns an Item rather than an Ident, we need to
            // have a custom `fold_item` method; a custom `fold_ident` method
            // wouldn't return the correct type.
            Item::Ident(ident) => {
                if self.variables.contains_key(ident) {
                    self.variables[ident].clone()
                } else {
                    Item::Ident(ident.clone())
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
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    fn add_function(&mut self, func: &Function) -> &Self {
        self.functions.insert(func.name.clone(), func.clone());
        self
    }
    fn run_function(&mut self, func_call: &FuncCall) -> Result<Item> {
        let func = self
            .functions
            .get(&func_call.name)
            .ok_or_else(|| anyhow!("Function {:?} not found", func_call.name))?;
        for arg in &func_call.args {
            if let Item::Ident(ident) = arg {
                if self.functions.contains_key(ident) {
                    return Err(anyhow!("Function {:?} called recursively", func_call.name));
                }
            }
        }
        if func.args.len() != func_call.args.len() {
            return Err(anyhow!(
                "Function {:?} called with wrong number of arguments. Expected {}, got {}",
                func_call.name,
                func.args.len(),
                func_call.args.len()
            ));
        }
        // Make a ReplaceVariables fold which we'll use to replace the variables
        // in the function with their argument values.
        let mut replace_variables = ReplaceVariables::new();
        zip(func.args.iter(), func_call.args.iter()).for_each(|(arg, arg_call)| {
            replace_variables.add_variables(&Assign {
                lvalue: arg.clone(),
                rvalue: Box::new(arg_call.clone()),
            });
        });
        // Take a clone of the body and replace the arguments with their values.
        Ok(Item::Terms(replace_variables.fold_items(&func.body)?).into_unnested())
    }
}

impl AstFold for RunFunctions {
    fn fold_function(&mut self, func: &Function) -> Result<Function> {
        let out = fold_function(self, func);
        // Add function to our list, after running it (no recursive functions atm).
        self.add_function(func);
        out
    }
    fn fold_item(&mut self, item: &Item) -> Result<Item> {
        // If it's an ident, it could be a func with no arg, so convert to Items.
        match (item).clone().coerce_to_terms() {
            Item::Terms(items) => {
                if let Some(Item::Ident(ident)) = items.first() {
                    if self.functions.get(ident).is_some() {
                        // Currently a transformation expects a Expr to wrap
                        // all the terms after the name. TODO: another area
                        // that's messy, should we parse a FuncCall directly?
                        let (name, body) = items.split_first().unwrap();
                        let func_call_transform =
                            vec![name.clone(), Item::Items(body.to_vec())].try_into()?;
                        if let Transformation::Func(func_call) = func_call_transform {
                            return self.run_function(&func_call);
                        } else {
                            unreachable!()
                        }
                    }
                }
                fold_item(self, item)
            }
            _ => Ok(fold_item(self, item)?),
        }
    }
}

/// Combines filters by putting them in parentheses and then joining them with `and`.
// Feels hacky — maybe this should be operation on a different level.
impl Filter {
    #[allow(unstable_name_collisions)] // Same behavior as the std lib; we can remove this + itertools when that's released.
    pub fn combine_filters(filters: Vec<Filter>) -> Filter {
        Filter(
            filters
                .into_iter()
                .map(|f| Item::Terms(f.0))
                .intersperse(Item::Raw("and".to_owned()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use insta::{assert_display_snapshot, assert_yaml_snapshot};

    use crate::parser::{ast_of_string, Rule};

    #[test]
    fn test_replace_variables() -> Result<()> {
        use super::*;
        use serde_yaml::to_string;
        use similar::TextDiff;

        let ast = &ast_of_string(
            r#"from employees
    derive [                                         # This adds columns / variables.
      gross_salary: salary + payroll_tax,
      gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
    ]
    "#,
            Rule::pipeline,
        )?;

        let mut fold = ReplaceVariables::new();
        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(TextDiff::from_lines(
            &to_string(ast)?,
            &to_string(&fold.fold_item(ast)?)?
        ).unified_diff(),
        @r###"
        @@ -11,6 +11,9 @@
               - lvalue: gross_cost
                 rvalue:
                   Terms:
        -            - Ident: gross_salary
        +            - Terms:
        +                - Ident: salary
        +                - Raw: +
        +                - Ident: payroll_tax
                     - Raw: +
                     - Ident: benefits_cost
        "###);

        let mut fold = ReplaceVariables::new();
        let ast = &ast_of_string(
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
            Rule::query,
        )?;
        assert_yaml_snapshot!(&fold.fold_item(ast)?);

        Ok(())
    }

    #[test]
    fn test_run_functions_no_arg() -> Result<()> {
        let ast = &ast_of_string(
            "
func count = testing_count

from employees
aggregate [
  count
]
",
            Rule::query,
        )?;

        assert_yaml_snapshot!(ast, @r###"
        ---
        Query:
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
            &to_string(ast).unwrap(),
            &to_string(&fold.fold_item(ast).unwrap()).unwrap(),
        )
        .unified_diff()
        .to_string();
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -10,5 +10,5 @@
               - Aggregate:
                   by: []
                   calcs:
        -            - Ident: count
        +            - Ident: testing_count
                   assigns: []
        "###);

        Ok(())
    }

    #[test]
    fn test_run_functions_args() -> Result<()> {
        let ast = &ast_of_string(
            r#"
func count x = s"count({x})"

from employees
aggregate [
  count salary
]
"#,
            Rule::query,
        )?;

        assert_yaml_snapshot!(ast, @r###"
        ---
        Query:
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
            &to_string(ast).unwrap(),
            &to_string(&fold.fold_item(ast).unwrap()).unwrap(),
        )
        .unified_diff()
        .to_string();
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -15,7 +15,9 @@
               - Aggregate:
                   by: []
                   calcs:
        -            - Terms:
        -                - Ident: count
        -                - Ident: salary
        +            - SString:
        +                - String: count(
        +                - Expr:
        +                    Ident: salary
        +                - String: )
                   assigns: []
        "###);

        Ok(())
    }

    #[test]
    fn test_compile() -> Result<()> {
        let pipeline = ast_of_string(
            r#"
func count x = s"count({x})"

from employees
aggregate [
  count salary
]
"#,
            Rule::query,
        )?;
        let ast = compile(pipeline)?;
        assert_yaml_snapshot!(ast,
            @r###"
        ---
        Query:
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

        let ast = ast_of_string(
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
            Rule::query,
        )?;
        assert_yaml_snapshot!(compile(ast)?);

        Ok(())
    }
}
