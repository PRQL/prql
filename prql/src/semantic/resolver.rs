use anyhow::Result;
use itertools::Itertools;

use crate::ast::*;
use crate::ast_fold::*;
use crate::error::{Error, Reason};

use super::context::Context;

/// Runs semantic analysis on the query, using current state.
/// Appends query to current query.
///
/// Note that analyzer removes function declarations, derive and select
/// transformations from AST and saves them as current context.
pub fn resolve(nodes: Vec<Node>, context: Option<Context>) -> Result<(Vec<Node>, Context)> {
    let context = context.unwrap_or_default();

    let mut resolver = Resolver { context };

    let nodes = resolver.fold_nodes(nodes)?;

    Ok((nodes, resolver.context))
}

/// Can fold (walk) over AST and for each function calls or variable find what they are referencing.
pub struct Resolver {
    pub context: Context,
}

impl AstFold for Resolver {
    // save functions declarations
    fn fold_nodes(&mut self, items: Vec<Node>) -> Result<Vec<Node>> {
        // We cut out function def, so we need to run it
        // here rather than in `fold_func_def`.
        items
            .into_iter()
            .map(|item| {
                Ok(match item {
                    Node {
                        item: Item::FuncDef(mut func_def),
                        ..
                    } => {
                        // declare variables
                        for param in &mut func_def.named_params {
                            param.declared_at = Some(self.context.declare_func_param(param));
                        }
                        for param in &mut func_def.positional_params {
                            param.declared_at = Some(self.context.declare_func_param(param));
                        }

                        // fold body
                        func_def.body = Box::new(self.fold_node(*func_def.body)?);

                        // clear declared variables
                        self.context.variables.remove("_");
                        self.context.refresh_inverse_index();

                        self.context.declare_func(func_def);
                        None
                    }
                    _ => Some(self.fold_node(item)?),
                })
            })
            .filter_map(|x| x.transpose())
            .try_collect()
    }

    fn fold_node(&mut self, mut node: Node) -> Result<Node> {
        node.item = match node.item {
            Item::FuncCall(func_call) => {
                node.declared_at = self.context.functions.get(&func_call.name).cloned();

                Item::FuncCall(self.fold_func_call(func_call)?)
            }

            Item::Ident(ident) => {
                node.declared_at = (self.context.lookup_variable(&ident))
                    .map_err(|e| Error::new(Reason::Simple(e)).with_span(node.span))?;

                Item::Ident(ident)
            }

            item => fold_item(self, item)?,
        };
        Ok(node)
    }

    fn fold_pipeline(&mut self, pipeline: Vec<Transform>) -> Result<Vec<Transform>> {
        pipeline
            .into_iter()
            .map(|t| {
                Ok(match t {
                    Transform::From(t) => {
                        self.context.clear_scopes();

                        self.context.frame.clear();

                        self.context.declare_table(&t);

                        let t = Transform::From(t);
                        Some(fold_transform(self, t)?)
                    }

                    Transform::Select(nodes) => {
                        self.context.frame.clear();

                        self.fold_and_declare(nodes)?;

                        self.context.clear_scopes();
                        None
                    }
                    Transform::Derive(nodes) => {
                        self.fold_and_declare(nodes)?;
                        None
                    }
                    Transform::Aggregate { by, select } => {
                        self.context.frame.clear();

                        let by = self.fold_and_declare(by)?;

                        self.fold_and_declare(select)?;

                        self.context.clear_scopes();

                        Some(Transform::Aggregate { by, select: vec![] })
                    }
                    Transform::Join { side, with, filter } => {
                        self.context.declare_table(&with);

                        Some(Transform::Join {
                            side,
                            with: self.fold_table_ref(with)?,
                            filter: match filter {
                                JoinFilter::On(nodes) => JoinFilter::On(self.fold_nodes(nodes)?),
                                JoinFilter::Using(nodes) => {
                                    for node in &nodes {
                                        self.ensure_in_two_namespaces(node).map_err(|e| {
                                            Error::new(Reason::Simple(e)).with_span(node.span)
                                        })?;
                                        self.context.declare_table_column(node, true);
                                    }
                                    JoinFilter::Using(nodes)
                                }
                            },
                        })
                    }
                    t => Some(fold_transform(self, t)?),
                })
            })
            .filter_map(|x| x.transpose())
            .try_collect()
    }
}

impl Resolver {
    fn fold_and_declare(&mut self, nodes: Vec<Node>) -> Result<Vec<Node>> {
        nodes
            .into_iter()
            .map(|node| {
                let node = self.fold_node(node)?;

                self.context.declare_table_column(&node, true);
                Ok(node)
            })
            .try_collect()
    }

    fn ensure_in_two_namespaces(&mut self, node: &Node) -> Result<(), String> {
        let ident = node.item.as_ident().unwrap();
        let namespaces = self.context.lookup_namespaces_of(ident);
        match namespaces.len() {
            0 => Err(format!("Unknown variable `{ident}`")),
            1 => Err("join using a column name must belong to both tables".to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use insta::{assert_debug_snapshot, assert_snapshot, assert_yaml_snapshot};
    use serde_yaml::from_str;

    use crate::{parse, translate};

    use super::*;

    #[test]
    fn test_scopes_during_from() {
        let context = Context::default();

        let mut resolver = Resolver { context };

        let pipeline: Node = from_str(
            r##"
            Pipeline:
                - From:
                    name: employees
                    alias: ~
        "##,
        )
        .unwrap();
        resolver.fold_node(pipeline).unwrap();

        assert_yaml_snapshot!(resolver.context.frame, @r###"
        ---
        - All: employees
        "###);
        assert!(resolver.context.variables["employees"].len() == 1);
    }

    #[test]
    fn test_scopes_during_select() {
        let context = Context::default();

        let mut resolver = Resolver { context };

        let pipeline: Node = from_str(
            r##"
            Pipeline:
                - From:
                    name: employees
                    alias: ~
                - Select:
                    - NamedExpr:
                        name: salary_1
                        expr:
                            Ident: salary
                    - NamedExpr:
                        name: salary_2
                        expr:
                            Expr:
                              - Ident: salary_1
                              - Raw: +
                              - Raw: '1'
                    - Ident: age
        "##,
        )
        .unwrap();
        resolver.fold_node(pipeline).unwrap();

        assert_eq!(resolver.context.frame.len(), 3);
        assert_debug_snapshot!(resolver.context.variables["$"].iter().sorted(), @r###"
        IntoIter(
            [
                (
                    "age",
                    4,
                ),
                (
                    "salary_1",
                    1,
                ),
                (
                    "salary_2",
                    2,
                ),
            ],
        )
        "###);
    }

    #[test]
    fn test_variable_scoping() {
        let prql = r#"
        from employees
        select first_name
        select last_name
        "#;
        let result = parse(prql).and_then(|x| resolve(x.nodes, None));
        assert!(result.is_err());

        let prql = r#"
        from employees
        select [salary1: salary, salary2: salary1 + 1, age]
        "#;
        let result: String = parse(prql).and_then(|x| translate(&x)).unwrap();
        assert_snapshot!(result, @r###"
        SELECT
          salary AS salary1,
          salary + 1 AS salary2,
          age
        FROM
          employees
        "###);
    }

    #[test]
    fn test_join_using_two_tables() {
        let prql = r#"
        from employees
        select [first_name, emp_no]
        join salaries [emp_no]
        select [first_name, salaries.salary]
        "#;
        let result = parse(prql).and_then(|x| resolve(x.nodes, None));
        result.unwrap();

        let prql = r#"
        from employees
        select first_name
        join salaries [emp_no]
        select [first_name, salaries.salary]
        "#;
        let result = parse(prql).and_then(|x| resolve(x.nodes, None));
        assert!(result.is_err());
    }

    #[test]
    fn test_ambiguous_resolve() {
        let prql = r#"
        from employees
        join salaries [emp_no]
        select first_name      # this could belong to either table!
        "#;
        let result = parse(prql).and_then(|x| translate(&x)).unwrap();
        assert_snapshot!(result, @r###"
        SELECT
          first_name
        FROM
          employees
          JOIN salaries USING(emp_no)
        "###);

        let prql = r#"
        from employees
        select first_name      # this can only be from employees
        "#;
        let result = parse(prql).and_then(|x| translate(&x)).unwrap();
        assert_snapshot!(result, @r###"
        SELECT
          first_name
        FROM
          employees
        "###);

        let prql = r#"
        from employees
        select [first_name, emp_no]
        join salaries [emp_no]
        select [first_name, emp_no, salary]
        "#;
        let result = parse(prql).and_then(|x| translate(&x)).unwrap();
        assert_snapshot!(result, @r###"
        SELECT
          employees.first_name,
          emp_no,
          salaries.salary
        FROM
          employees
          JOIN salaries USING(emp_no)
        "###);
    }
}
