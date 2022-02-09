use std::collections::HashMap;

use crate::parser::{
    Assign, Ident, Idents, Item, Items, NamedArg, Pipeline, Transformation, TransformationType,
};

/// An object in which we want to replace variables with the items in those variables.
pub trait ContainsVariables<'a> {
    #[must_use]
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self;
}

impl<'a> ContainsVariables<'a> for Pipeline<'a> {
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self
    where
        Self: Sized,
        // Very messy function — we should clean up.
    {
        // We don't expect to use a variables arg, but the function takes one
        // out of conformity. We use it as a base rather than discard it in the
        // case that the function is passed one.
        let mut variables = variables.clone();

        self.iter()
            .map(|transformation| match transformation.name {
                // If it's a derive, add the variables to the hashmap (while
                // also replacing its variables with those which came before
                // it).
                TransformationType::Derive => Transformation {
                    name: transformation.name.clone(),
                    named_args: transformation.named_args.clone(),
                    args: transformation
                        .args
                        .iter()
                        .map(|arg| match arg {
                            // These can either have an Assign, or a list of Assigns
                            Item::Assign(assign) => {
                                let assign_replaced = assign.replace_variables(&variables);
                                variables.extend(extract_variables(assign_replaced.clone()));
                                Item::Assign(assign_replaced)
                            }
                            Item::List(assigns) => {
                                Item::List(
                                    assigns
                                        .iter()
                                        .map(|assign| match assign {
                                            // This is copy-pasted from above —
                                            // should we run a normalization
                                            // step before to move everything
                                            // into lists?
                                            Item::Assign(assign) => {
                                                let assign_replaced =
                                                    assign.replace_variables(&variables);
                                                variables.extend(extract_variables(
                                                    assign_replaced.clone(),
                                                ));

                                                Item::Assign(assign_replaced)
                                            }
                                            _ => {
                                                unreachable!(
                                                    "Derives should only contain Assigns; {:?}",
                                                    assign
                                                )
                                            }
                                        })
                                        .collect(),
                                )
                            }
                            _ => unreachable!("Derives should only contain Assigns"),
                        })
                        .collect(),
                },
                // For everything else, just replace the variables.
                _ => transformation.replace_variables(&variables),
            })
            .collect()
    }
}

fn extract_variables(assign: Assign) -> HashMap<Ident, Item> {
    let mut variables = HashMap::new();
    // Not sure we're choosing the correct Item / Items in the types, this is a
    // bit of a smell.
    variables.insert(assign.lvalue, Item::Items(assign.rvalue));
    variables
}

impl<'a> ContainsVariables<'a> for Item<'a> {
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self {
        // This is verbose — is there a better approach? If we have to do this
        // again for another function, we could change it to a Visitor pattern.
        // But we'd need to encode things like not replacing `lvalue`s. Many of
        // these are doing exactly the same thing — iterating through their
        // itesm.
        match self {
            Item::Ident(ident) => {
                if variables.contains_key(ident) {
                    variables[ident].clone()
                } else {
                    self.clone()
                }
            }
            Item::Items(items) => Item::Items(items.replace_variables(variables)),
            Item::List(items) => Item::List(items.replace_variables(variables)),
            Item::Query(items) => Item::Query(items.replace_variables(variables)),
            Item::Pipeline(transformations) => {
                Item::Pipeline(transformations.replace_variables(variables))
            }
            Item::NamedArg(named_arg) => Item::NamedArg(named_arg.replace_variables(variables)),
            Item::Assign(assign) => Item::Assign(assign.replace_variables(variables)),
            Item::String(_) | Item::Raw(_) | Item::TODO(_) => self.clone(),
            Item::Transformation(transformation) => {
                Item::Transformation(transformation.replace_variables(variables))
            }
            Item::Idents(idents) => Item::Idents(idents.replace_variables(variables)),
            // Currently functions don't capture variables, so we don't need to
            // replace them.
            Item::Function(function) => Item::Function(function.clone()),
        }
    }
}

impl<'a> ContainsVariables<'a> for Assign<'a> {
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self {
        Assign {
            lvalue: self.lvalue,
            rvalue: self
                .rvalue
                .iter()
                .map(|item| item.replace_variables(variables))
                .collect::<Items<'a>>(),
        }
    }
}

impl<'a> ContainsVariables<'a> for NamedArg<'a> {
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self {
        NamedArg {
            lvalue: self.lvalue,
            rvalue: self
                .rvalue
                .iter()
                .map(|item| item.replace_variables(variables))
                .collect::<Items<'a>>(),
        }
    }
}

impl<'a> ContainsVariables<'a> for Idents<'a> {
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self {
        self.iter()
            // TODO: Not the most elegant approach. Possibly up a level we could parse
            // `Ident`s into `Items` — but probably push until we add named_args
            // to functions.
            .map(|item| Item::Ident(item).replace_variables(variables).as_ident())
            .collect()
    }
}

impl<'a> ContainsVariables<'a> for Items<'a> {
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self {
        self.iter()
            .map(|item| item.replace_variables(variables))
            .collect()
    }
}

impl<'a> ContainsVariables<'a> for Transformation<'a> {
    fn replace_variables(&self, variables: &HashMap<Ident<'a>, Item<'a>>) -> Self {
        Transformation {
            name: self.name.to_owned(),
            args: self
                .args
                .iter()
                .map(|item| item.replace_variables(variables))
                .collect(),
            named_args: self
                .named_args
                .iter()
                .map(|named_arg| named_arg.replace_variables(variables))
                .collect(),
        }
    }
}

#[test]
fn test_replace_variables() {
    use crate::parser::{parse, parse_to_pest_tree, Rule};
    use insta::assert_yaml_snapshot;
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
    assert_yaml_snapshot!(ast.replace_variables(&HashMap::new()));

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
    count,
]
sort sum_gross_cost
filter count > 200
take 20
    "#,
            Rule::query,
        )
        .unwrap(),
    )
    .unwrap()[0];
    assert_yaml_snapshot!(ast.replace_variables(&HashMap::new()));
}
