use super::ast::*;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use std::collections::HashMap;

// Fold pattern:
// - https://rust-unofficial.github.io/patterns/patterns/creational/fold.html
// Good discussions on the visitor / fold pattern:
// - https://github.com/rust-unofficial/patterns/discussions/236 (within this,
//   this comment looked interesting: https://github.com/rust-unofficial/patterns/discussions/236#discussioncomment-393517)
// - https://news.ycombinator.com/item?id=25620110

// TODO: some of these impls will be too specific because they were copied from
// when ReplaceVariables was implemented directly. When we find a case that is
// overfit on ReplaceVariables, we should add the custom impl to
// ReplaceVariables, and write a more generic impl to this.
pub trait AstFold {
    fn fold_pipeline(&mut self, pipeline: &Pipeline) -> Result<Pipeline> {
        pipeline
            .iter()
            .map(|t| self.fold_transformation(t))
            .collect()
    }
    fn fold_ident(&mut self, ident: &Ident) -> Result<Ident> {
        Ok(ident.clone())
    }
    fn fold_items(&mut self, items: &Items) -> Result<Items> {
        items.iter().map(|item| self.fold_item(item)).collect()
    }
    fn fold_table(&mut self, table: &Table) -> Result<Table> {
        Ok(Table {
            name: self.fold_ident(&table.name)?,
            pipeline: self.fold_pipeline(&table.pipeline)?,
        })
    }
    fn fold_named_arg(&mut self, named_arg: &NamedArg) -> Result<NamedArg> {
        Ok(NamedArg {
            name: self.fold_ident(&named_arg.name)?,
            arg: Box::new(self.fold_item(&named_arg.arg)?),
        })
    }
    fn fold_filter(&mut self, filter: &Filter) -> Result<Filter> {
        Ok(Filter(
            filter.0.iter().map(|i| self.fold_item(i)).try_collect()?,
        ))
    }
    // For some functions, we want to call a default impl, because copying &
    // pasting everything apart from a specific match is lots of repetition. So
    // we define a function outside the trait, by default call it, and let
    // implementors override the default while calling the function directly for
    // some cases. Feel free to extend the functions that are separate when
    // necessary. Ref https://stackoverflow.com/a/66077767/3064736
    fn fold_transformation(&mut self, transformation: &Transformation) -> Result<Transformation> {
        fold_transformation(self, transformation)
    }
    fn fold_item(&mut self, item: &Item) -> Result<Item> {
        fold_item(self, item)
    }
    fn fold_function(&mut self, function: &Function) -> Result<Function> {
        fold_function(self, function)
    }
    fn fold_func_call(&mut self, func_call: &FuncCall) -> Result<FuncCall> {
        fold_func_call(self, func_call)
    }
    fn fold_assign(&mut self, assign: &Assign) -> Result<Assign> {
        fold_assign(self, assign)
    }
    fn fold_sstring_item(&mut self, sstring_item: &SStringItem) -> Result<SStringItem> {
        fold_sstring_item(self, sstring_item)
    }
}

fn fold_sstring_item<T: ?Sized + AstFold>(
    fold: &mut T,
    sstring_item: &SStringItem,
) -> Result<SStringItem> {
    Ok(match sstring_item {
        SStringItem::String(string) => SStringItem::String(string.clone()),
        SStringItem::Expr(expr) => SStringItem::Expr(fold.fold_item(expr)?),
    })
}
fn fold_transformation<T: ?Sized + AstFold>(
    fold: &mut T,
    transformation: &Transformation,
) -> Result<Transformation> {
    match transformation {
        Transformation::Derive(assigns) => Ok(Transformation::Derive({
            assigns
                .iter()
                .map(|assign| fold.fold_assign(assign))
                .try_collect()?
        })),
        Transformation::From(items) => Ok(Transformation::From(fold.fold_items(items)?)),
        Transformation::Filter(Filter(items)) => {
            Ok(Transformation::Filter(Filter(fold.fold_items(items)?)))
        }
        Transformation::Sort(items) => Ok(Transformation::Sort(fold.fold_items(items)?)),
        Transformation::Join(items) => Ok(Transformation::Join(fold.fold_items(items)?)),
        Transformation::Select(items) => Ok(Transformation::Select(fold.fold_items(items)?)),
        Transformation::Aggregate { by, calcs, assigns } => Ok(Transformation::Aggregate {
            by: fold.fold_items(by)?,
            calcs: fold.fold_items(calcs)?,
            assigns: assigns
                .iter()
                .map(|assign| fold.fold_assign(assign))
                .try_collect()?,
        }),
        Transformation::Func(func_call) => {
            Ok(Transformation::Func(fold.fold_func_call(func_call)?))
        }
        // TODO: generalize? Or this never changes?
        Transformation::Take(_) => Ok(transformation.clone()),
    }
}
fn fold_func_call<T: ?Sized + AstFold>(fold: &mut T, func_call: &FuncCall) -> Result<FuncCall> {
    Ok(FuncCall {
        // TODO: generalize? Or this never changes?
        name: func_call.name.to_owned(),
        args: func_call
            .args
            .iter()
            .map(|item| fold.fold_item(item))
            .try_collect()?,
        named_args: func_call
            .named_args
            .iter()
            .map(|named_arg| fold.fold_named_arg(named_arg))
            .try_collect()?,
    })
}
fn fold_item<T: ?Sized + AstFold>(fold: &mut T, item: &Item) -> Result<Item> {
    Ok(match item {
        Item::Ident(ident) => Item::Ident(fold.fold_ident(ident)?),
        Item::Items(items) => Item::Items(fold.fold_items(items)?),
        // TODO: possibly implement for expr.
        Item::Expr(items) => Item::Expr(fold.fold_items(items)?),
        Item::Idents(idents) => {
            Item::Idents(idents.iter().map(|i| fold.fold_ident(i)).try_collect()?)
        }
        Item::List(items) => Item::List(fold.fold_items(items)?),
        Item::Query(items) => Item::Query(fold.fold_items(items)?),
        Item::Pipeline(transformations) => Item::Pipeline(
            transformations
                .iter()
                .map(|t| fold.fold_transformation(t))
                .try_collect()?,
        ),
        Item::NamedArg(named_arg) => Item::NamedArg(fold.fold_named_arg(named_arg)?),
        Item::Assign(assign) => Item::Assign(fold.fold_assign(assign)?),
        Item::Transformation(transformation) => {
            Item::Transformation(fold.fold_transformation(transformation)?)
        }
        Item::SString(items) => Item::SString(
            items
                .iter()
                .map(|x| fold.fold_sstring_item(x))
                .try_collect()?,
        ),
        Item::Function(func) => Item::Function(fold.fold_function(func)?),
        // TODO: implement for these
        Item::Table(_) => item.clone(),
        // None of these capture variables, so we don't need to replace
        // them.
        Item::String(_) | Item::Raw(_) | Item::Todo(_) => item.clone(),
    })
}
fn fold_function<T: ?Sized + AstFold>(fold: &mut T, function: &Function) -> Result<Function> {
    Ok(Function {
        name: fold.fold_ident(&function.name)?,
        args: function
            .args
            .iter()
            .map(|i| fold.fold_ident(i))
            .try_collect()?,
        body: fold.fold_items(&function.body)?,
    })
}
fn fold_assign<T: ?Sized + AstFold>(fold: &mut T, assign: &Assign) -> Result<Assign> {
    Ok(Assign {
        lvalue: fold.fold_ident(&assign.lvalue)?,
        rvalue: Box::new(fold.fold_item(&assign.rvalue)?),
    })
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
        })
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
            .ok_or_else(|| anyhow!("Function {} not found", func_call.name))?;
        Ok(Item::Items(func.body.clone()))
    }
}

// One issue is that we don't actually know where a function call will be. So
// `count foo` could be `count(foo)`, or `foo` could be a function with no args
// that refers to `bar`, meaning it evaluates to `count(bar)`.
impl AstFold for RunFunctions {
    fn fold_function(&mut self, func: &Function) -> Result<Function> {
        let out = fold_function(self, func);
        // Add function to our list, after running it (no recursive functions atm).
        self.add_function(func);
        out
    }
    fn fold_item(&mut self, item: &Item) -> Result<Item> {
        match item {
            Item::Transformation(Transformation::Func(func_call)) => {
                Ok(self.run_function(func_call)?)
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
                .map(|f| Item::Items(f.0))
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
        @@ -12,6 +12,9 @@
               - lvalue: gross_cost
                 rvalue:
                   Items:
        -            - Ident: gross_salary
        +            - Items:
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
    fn test_run_functions() -> Result<()> {
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
              - From:
                  - Ident: employees
              - Aggregate:
                  by: []
                  calcs:
                    - Ident: count
                  assigns: []
        "###);

        // TODO: Fix now we have a new way of running functions.
        // use serde_yaml::to_string;
        // use similar::TextDiff;

        // let mut fold = RunFunctions::new();
        // // We could make a convenience function for this. It's useful for
        // // showing the diffs of an operation.
        // let diff = TextDiff::from_lines(
        //     &to_string(ast).unwrap(),
        //     &to_string(&fold.fold_item(ast).unwrap()).unwrap(),
        // )
        // .unified_diff()
        // .to_string();
        // assert!(!diff.is_empty());
        // assert_display_snapshot!(diff, @r###"
        // @@ -12,9 +12,7 @@
        //        - Aggregate:
        //            by: []
        //            calcs:
        // -            - Transformation:
        // -                Func:
        // -                  name: count
        // -                  args: []
        // -                  named_args: []
        // +            - Items:
        // +                - Items:
        // +                    - Ident: testing_count
        //            assigns: []
        // "###);

        Ok(())
    }
}
