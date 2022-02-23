use super::ast::*;
use anyhow::Result;
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

    fn fold_item(&mut self, item: &Item) -> Result<Item> {
        Ok(match item {
            Item::Ident(ident) => Item::Ident(self.fold_ident(ident)?),
            Item::Items(items) => Item::Items(self.fold_items(items)?),
            Item::Idents(idents) => {
                Item::Idents(idents.iter().map(|i| self.fold_ident(i)).try_collect()?)
            }
            Item::List(items) => Item::List(self.fold_items(items)?),
            Item::Query(items) => Item::Query(self.fold_items(items)?),
            Item::Pipeline(transformations) => Item::Pipeline(
                transformations
                    .iter()
                    .map(|t| self.fold_transformation(t))
                    .try_collect()?,
            ),
            Item::NamedArg(named_arg) => Item::NamedArg(self.fold_named_arg(named_arg)?),
            Item::Assign(assign) => Item::Assign(self.fold_assign(assign)?),
            Item::Transformation(transformation) => {
                Item::Transformation(self.fold_transformation(transformation)?)
            }
            Item::SString(items) => Item::SString(
                items
                    .iter()
                    .map(|x| self.fold_sstring_item(x))
                    .try_collect()?,
            ),
            // None of these capture variables, so we don't need to replace
            // them.
            Item::Function(_) | Item::Table(_) | Item::String(_) | Item::Raw(_) | Item::TODO(_) => {
                item.clone()
            }
        })
    }

    fn fold_items(&mut self, items: &Items) -> Result<Items> {
        items.iter().map(|item| self.fold_item(item)).collect()
    }

    fn fold_function(&mut self, function: &Function) -> Result<Function> {
        Ok(Function {
            name: self.fold_ident(&function.name)?,
            args: function
                .args
                .iter()
                .map(|i| self.fold_ident(i))
                .try_collect()?,
            body: function
                .body
                .iter()
                .map(|i| self.fold_item(i))
                .try_collect()?,
        })
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
    fn fold_assign(&mut self, assign: &Assign) -> Result<Assign> {
        Ok(Assign {
            lvalue: self.fold_ident(&assign.lvalue)?,
            rvalue: Box::new(self.fold_item(&assign.rvalue)?),
        })
    }
    fn fold_sstring_item(&mut self, sstring_item: &SStringItem) -> Result<SStringItem> {
        Ok(match sstring_item {
            SStringItem::String(string) => SStringItem::String(string.clone()),
            SStringItem::Expr(expr) => SStringItem::Expr(self.fold_item(expr)?),
        })
    }
    fn fold_filter(&mut self, filter: &Filter) -> Result<Filter> {
        Ok(Filter(
            filter.0.iter().map(|i| self.fold_item(i)).try_collect()?,
        ))
    }
    fn fold_transformation(&mut self, transformation: &Transformation) -> Result<Transformation>;
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

    fn extract_variables(&mut self, assign: &Assign) -> &Self {
        // Not sure we're choosing the correct Item / Items in the types, this is a
        // bit of a smell.
        self.variables
            .insert(assign.lvalue.clone(), *(assign.rvalue).clone());
        self
    }
}

impl AstFold for ReplaceVariables {
    fn fold_transformation(&mut self, transformation: &Transformation) -> Result<Transformation> {
        match transformation {
            // If it's a derive, add the variables to the hashmap (while
            // also replacing its variables with those which came before
            // it).
            Transformation::Derive(assigns) => Ok(Transformation::Derive({
                assigns
                    .iter()
                    .map(|assign| {
                        {
                            // Replace this assign using existing variable
                            // mapping before adding its variables into the
                            // variable mapping.
                            // let assign_replaced = self.fold_assign(assign)?;
                            let assign_replaced = self.fold_assign(assign).unwrap();
                            self.extract_variables(&assign_replaced);
                            assign_replaced
                        }
                    })
                    .collect()
            })),
            Transformation::From(items) => Ok(Transformation::From(self.fold_items(items)?)),
            Transformation::Filter(Filter(items)) => {
                Ok(Transformation::Filter(Filter(self.fold_items(items)?)))
            }
            Transformation::Sort(items) => Ok(Transformation::Sort(self.fold_items(items)?)),
            Transformation::Join(items) => Ok(Transformation::Join(self.fold_items(items)?)),
            Transformation::Select(items) => Ok(Transformation::Select(self.fold_items(items)?)),
            Transformation::Aggregate { by, calcs, assigns } => Ok(Transformation::Aggregate {
                by: self.fold_items(by)?,
                // TODO: this is currently matching against the impl on Pipeline
                // because it's a Vec of Transformation — is that OK?
                calcs: calcs
                    .iter()
                    .map(|t| self.fold_transformation(t))
                    .try_collect()?,
                assigns: assigns
                    .iter()
                    .map(|assign| self.fold_assign(assign))
                    .try_collect()?,
            }),
            // For everything else, just visit each object and replace the variables.
            Transformation::Func {
                name,
                args,
                named_args,
            } => Ok(Transformation::Func {
                // TODO: generalize
                name: name.to_owned(),
                args: args.iter().map(|item| self.fold_item(item)).try_collect()?,
                named_args: named_args
                    .iter()
                    .map(|named_arg| self.fold_named_arg(named_arg))
                    .try_collect()?,
            }),
            // TODO: generalize
            Transformation::Take(_) => Ok(transformation.clone()),
        }
    }
    // same as above apart from Ident
    fn fold_item(&mut self, item: &Item) -> Result<Item> {
        Ok(match item {
            Item::Ident(ident) => {
                if self.variables.contains_key(ident) {
                    self.variables[ident].clone()
                } else {
                    Item::Ident(ident.clone())
                }
            }
            Item::Items(items) => Item::Items(self.fold_items(items)?),
            Item::Idents(idents) => {
                Item::Idents(idents.iter().map(|i| self.fold_ident(i)).try_collect()?)
            }
            Item::List(items) => Item::List(self.fold_items(items)?),
            Item::Query(items) => Item::Query(self.fold_items(items)?),
            Item::Pipeline(transformations) => Item::Pipeline(
                transformations
                    .iter()
                    .map(|t| self.fold_transformation(t))
                    .try_collect()?,
            ),
            Item::NamedArg(named_arg) => Item::NamedArg(self.fold_named_arg(named_arg)?),
            Item::Assign(assign) => Item::Assign(self.fold_assign(assign)?),
            Item::Transformation(transformation) => {
                Item::Transformation(self.fold_transformation(transformation)?)
            }
            Item::SString(items) => Item::SString(
                items
                    .iter()
                    .map(|x| self.fold_sstring_item(x))
                    .try_collect()?,
            ),
            // None of these capture variables, so we don't need to replace
            // them.
            Item::Function(_) | Item::Table(_) | Item::String(_) | Item::Raw(_) | Item::TODO(_) => {
                item.clone()
            }
        })
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

    #[test]
    fn test_replace_variables() {
        use crate::parser::{parse, parse_to_pest_tree, Rule};
        use insta::assert_yaml_snapshot;
        use serde_yaml::to_string;
        use similar::TextDiff;

        let ast = &parse(
            parse_to_pest_tree(
                r#"from employees
    derive [                                         # This adds columns / variables.
      gross_salary: salary + payroll_tax,
      gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
    ]
    "#,
                Rule::pipeline,
            )
            .unwrap(),
        )
        .unwrap()[0];

        let mut fold = ReplaceVariables::new();
        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(TextDiff::from_lines(
            &to_string(ast).unwrap(),
            &to_string(&fold.fold_item(ast).unwrap()).unwrap()
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
        let ast = &parse(
            parse_to_pest_tree(
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
            )
            .unwrap(),
        )
        .unwrap()[0];
        assert_yaml_snapshot!(&fold.fold_item(ast).unwrap());
    }
}
