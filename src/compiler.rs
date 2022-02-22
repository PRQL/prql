use super::ast::*;
use itertools::Itertools;
use std::collections::HashMap;

/// An object in which we want to replace variables with the items in those variables.
pub trait ContainsVariables {
    #[must_use]
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self;
}

impl ContainsVariables for Pipeline {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self
    where
        Self: Sized,
    {
        // We don't expect to use a variables arg, but the function takes one
        // out of conformity. We use it as a base rather than discard it in the
        // case that the function is passed one.
        let mut variables = variables.clone();

        self.iter()
            .map(|t| t.replace_variables(&mut variables))
            .collect()
    }
}

fn extract_variables(assign: &Assign) -> HashMap<Ident, Item> {
    let mut variables = HashMap::new();
    // Not sure we're choosing the correct Item / Items in the types, this is a
    // bit of a smell.
    variables.insert(assign.lvalue.clone(), *(assign.rvalue).clone());
    variables
}

impl ContainsVariables for Item {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self {
        // This is verbose — is there a better approach? If we have to do this
        // again for another function, we could change it to a Visitor pattern.
        // But we'd need to encode things like not replacing `lvalue`s. Many of
        // these are doing exactly the same thing — iterating through their
        // items.
        match self {
            Item::Ident(ident) => {
                if variables.contains_key(ident) {
                    variables[ident].clone()
                } else {
                    self.clone()
                }
            }
            Item::Items(items) => Item::Items(items.replace_variables(variables)),
            // See notes in func — possibly this should just parse to Items.
            Item::Idents(idents) => Item::Idents(idents.replace_variables(variables)),
            Item::List(items) => Item::List(items.replace_variables(variables)),
            Item::Query(items) => Item::Query(items.replace_variables(variables)),
            Item::Pipeline(transformations) => {
                Item::Pipeline(transformations.replace_variables(variables))
            }
            Item::NamedArg(named_arg) => Item::NamedArg(named_arg.replace_variables(variables)),
            Item::Assign(assign) => Item::Assign(assign.replace_variables(variables)),
            Item::Transformation(transformation) => {
                Item::Transformation(transformation.replace_variables(variables))
            }
            // None of these capture variables, so we don't need to replace
            // them.
            Item::Function(_) | Item::Table(_) | Item::String(_) | Item::Raw(_) | Item::TODO(_) => {
                self.clone()
            }
        }
    }
}

impl ContainsVariables for Assign {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self {
        Assign {
            lvalue: self.lvalue.to_owned(),
            rvalue: Box::new(self.rvalue.replace_variables(variables)),
        }
    }
}

impl ContainsVariables for NamedArg {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self {
        NamedArg {
            name: self.name.to_owned(),
            arg: Box::new(self.arg.replace_variables(variables)),
        }
    }
}

impl ContainsVariables for Idents {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self {
        self.iter()
            // TODO: Not the most elegant approach. Possibly up a level we could parse
            // `Ident`s into `Items` — but probably push until we add named_args
            // to functions.
            .map(|item| {
                Item::Ident(item.to_string())
                    .replace_variables(variables)
                    .as_ident()
                    .cloned()
                    .unwrap()
            })
            .collect()
    }
}

impl ContainsVariables for Items {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self {
        self.iter()
            .map(|item| item.replace_variables(variables))
            .collect()
    }
}

impl ContainsVariables for Filter {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self {
        Filter(
            self.0
                .iter()
                .map(|item| item.replace_variables(variables))
                .collect(),
        )
    }
}

impl ContainsVariables for Transformation {
    fn replace_variables(&self, variables: &mut HashMap<Ident, Item>) -> Self {
        // As above re this being verbose and possibly we write a visitor
        // pattern to visit all `items`.
        match self {
            // If it's a derive, add the variables to the hashmap (while
            // also replacing its variables with those which came before
            // it).
            Transformation::Derive(assigns) => Transformation::Derive({
                assigns
                    .iter()
                    .map(|assign| {
                        {
                            // Replace this assign using existing variable
                            // mapping before adding its variables into the
                            // variable mapping.
                            let assign_replaced = assign.replace_variables(variables);
                            variables.extend(extract_variables(&assign_replaced));
                            assign_replaced
                        }
                    })
                    .collect()
            }),
            Transformation::From(items) => Transformation::From(items.replace_variables(variables)),
            Transformation::Filter(items) => {
                Transformation::Filter(items.replace_variables(variables))
            }
            Transformation::Sort(ref items) => {
                Transformation::Sort(items.replace_variables(variables))
            }
            Transformation::Join(ref items) => {
                Transformation::Join(items.replace_variables(variables))
            }
            Transformation::Select(ref items) => {
                Transformation::Select(items.replace_variables(variables))
            }
            Transformation::Aggregate { by, calcs, assigns } => Transformation::Aggregate {
                by: by.replace_variables(variables),
                // TODO: this is currently matching against the impl on Pipeline
                // because it's a Vec of Transformation — is that OK?
                calcs: calcs.replace_variables(variables),
                assigns: assigns
                    .iter()
                    .map(|assign| assign.replace_variables(variables))
                    .collect(),
            },
            // For everything else, just visit each object and replace the variables.
            Transformation::Custom {
                name,
                args,
                named_args,
            } => Transformation::Custom {
                name: name.to_owned(),
                args: args
                    .iter()
                    .map(|item| item.replace_variables(variables))
                    .collect(),
                named_args: named_args
                    .iter()
                    .map(|named_arg| named_arg.replace_variables(variables))
                    .collect(),
            },
            &Transformation::Take(_) => self.clone(),
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

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(TextDiff::from_lines(
            &to_string(ast).unwrap(),
            &to_string(&ast.replace_variables(&mut HashMap::new())).unwrap()
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
        assert_yaml_snapshot!(ast.replace_variables(&mut HashMap::new()));
    }
}
