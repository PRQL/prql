use std::collections::HashSet;

use anyhow::{anyhow, Result};
use itertools::Itertools;

use crate::ast::ast_fold::*;
use crate::error::{Error, Reason, WithErrorInfo};
use crate::{ast::*, Declaration, FrameColumn};

use super::transforms;
use super::{Context, Frame};

/// Runs semantic analysis on the query, using current state.
/// Appends query to current query.
///
/// Note that analyzer removes function declarations, derive and select
/// transformations from AST and saves them as current context.
pub fn resolve(nodes: Vec<Node>, context: Option<Context>) -> Result<(Vec<Node>, Context)> {
    let context = context.unwrap_or_else(init_context);

    let mut resolver = Resolver::new(context);

    let nodes = resolver.fold_nodes(nodes)?;

    Ok((nodes, resolver.context))
}

/// Can fold (walk) over AST and for each function calls or variable find what they are referencing.
pub struct Resolver {
    pub context: Context,

    /// True iff resolving a function curry (in a pipeline)
    within_curry: bool,
}
impl Resolver {
    fn new(context: Context) -> Self {
        Resolver {
            context,
            within_curry: false,
        }
    }
}

impl AstFold for Resolver {
    fn fold_pipeline(&mut self, pipeline: Pipeline) -> Result<Pipeline> {
        let value = fold_optional_box(self, pipeline.value)?;

        self.within_curry = true;
        let functions = self.fold_nodes(pipeline.functions)?;
        self.within_curry = false;

        Ok(Pipeline { value, functions })
    }

    // save functions declarations
    fn fold_nodes(&mut self, items: Vec<Node>) -> Result<Vec<Node>> {
        // We cut out function def, so we need to run it
        // here rather than in `fold_func_def`.
        items
            .into_iter()
            .map(|node| {
                Ok(match node.item {
                    Item::FuncDef(mut func_def) => {
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
                        self.context.clear_scope();

                        self.context.declare_func(func_def);
                        None
                    }
                    _ => Some(self.fold_node(node)?),
                })
            })
            .filter_map(|x| x.transpose())
            .try_collect()
    }

    fn fold_node(&mut self, mut node: Node) -> Result<Node> {
        let within_curry = self.within_curry;
        self.within_curry = false;

        node.item = match node.item {
            Item::FuncCall(func_call) => {
                // find declaration
                node.declared_at = self.context.scope.functions.get(&func_call.name).cloned();

                // validate function call
                let (func_call, func_def) = self
                    .validate_function_call(node.declared_at, func_call, within_curry)
                    .with_span(node.span)?;

                // fold (and cast if this is a transform)
                if let Some(FuncKind::Transform) = func_def.kind {
                    let transform = transforms::cast_transform(func_call, node.span)?;

                    let transform = self.fold_transform(transform)?;

                    node.frame = Some(self.context.frame.clone());

                    Item::Transform(transform)
                } else {
                    Item::FuncCall(self.fold_func_call(func_call)?)
                }
            }

            Item::Ident(ident) => {
                node.declared_at = Some(
                    (self.context.lookup_variable(&ident, node.span))
                        .map_err(|e| Error::new(Reason::Simple(e)).with_span(node.span))?,
                );

                Item::Ident(ident)
            }

            item => fold_item(self, item)?,
        };

        self.within_curry = within_curry;
        Ok(node)
    }

    fn fold_transform(&mut self, t: Transform) -> Result<Transform> {
        Ok(match t {
            Transform::From(t) => {
                self.context.clear_scope();

                self.context.frame = Frame::default();

                self.context.declare_table(&t);

                let t = Transform::From(t);
                fold_transform(self, t)?
            }

            Transform::Select(mut select) => {
                self.context.frame.columns.clear();

                select.assigns = self.fold_assigns(select.assigns)?;
                self.apply_context(&mut select)?;

                self.context.clear_scope();

                Transform::Select(select)
            }
            Transform::Derive(mut select) => {
                select.assigns = self.fold_assigns(select.assigns)?;
                self.apply_context(&mut select)?;

                Transform::Derive(select)
            }
            Transform::Group { by, pipeline } => {
                let by = self.fold_nodes(by)?;
                self.context.frame.group = extract_group_by(&by)?;

                let pipeline = Box::new(self.fold_node(*pipeline)?);

                self.context.frame.group.clear();
                Transform::Group { by, pipeline }
            }
            Transform::Aggregate(mut select) => {
                self.context.frame.columns.clear();
                self.context.frame.groups_to_columns();

                select.assigns = self.fold_assigns(select.assigns)?;
                self.apply_context(&mut select)?;

                self.context.clear_scope();

                Transform::Aggregate(select)
            }
            Transform::Join { side, with, filter } => {
                self.context.declare_table(&with);

                Transform::Join {
                    side,
                    with: self.fold_table_ref(with)?,
                    filter: self.fold_join_filter(filter)?,
                }
            }
            Transform::Sort(sort) => {
                let sort = self.fold_column_sorts(sort)?;

                self.context.frame.sort = extract_sorts(sort.clone())?;

                Transform::Sort(sort)
            }
            t => fold_transform(self, t)?,
        })
    }

    fn fold_join_filter(&mut self, filter: JoinFilter) -> Result<JoinFilter> {
        Ok(match filter {
            JoinFilter::On(nodes) => JoinFilter::On(self.fold_nodes(nodes)?),
            JoinFilter::Using(mut nodes) => {
                for node in &mut nodes {
                    let ident = node.item.as_ident().unwrap();

                    // ensure two namespaces
                    let namespaces = self.context.lookup_namespaces_of(ident);
                    match namespaces.len() {
                        0 => Err(format!("Unknown variable `{ident}`")),
                        1 => Err("join using a column name must belong to both tables".to_string()),
                        _ => Ok(()),
                    }
                    .map_err(|e| Error::new(Reason::Simple(e)).with_span(node.span))?;

                    let decl = Declaration::ExternRef {
                        table: None,
                        variable: ident.to_string(),
                    };

                    let id = self.context.declare(decl, node.span);
                    self.context.scope.add(ident.clone(), id);

                    let column = FrameColumn::Named(ident.clone(), id);
                    self.context.frame.columns.push(column);

                    node.declared_at = Some(id);
                }
                JoinFilter::Using(nodes)
            }
        })
    }
}

impl Resolver {
    fn fold_assigns(&mut self, nodes: Vec<Node>) -> Result<Vec<Node>> {
        nodes
            .into_iter()
            .map(|mut node| {
                match node.item {
                    Item::NamedExpr(NamedExpr { name, expr }) => {
                        // introduce a new expression alias

                        let (expr, _) = self.fold_assign_expr(*expr)?;
                        let id = expr.declared_at.unwrap();

                        self.context.frame.add_column(Some(name.clone()), id);
                        self.context.scope.add(name.clone(), id);

                        node.item = Item::Ident(name);
                        node.declared_at = Some(id);
                        Ok(node)
                    }
                    _ => {
                        // try to guess a name, otherwise use unnamed column

                        let (expr, name) = self.fold_assign_expr(node)?;
                        let id = expr.declared_at.unwrap();

                        self.context.frame.add_column(name, id);
                        Ok(expr)
                    }
                }
            })
            .try_collect()
    }

    fn fold_assign_expr(&mut self, node: Node) -> Result<(Node, Option<String>)> {
        let span = node.span;

        match node.item {
            Item::Ident(ref ident) => {
                // keep existing ident

                let name = ident.clone();
                let node = self.fold_node(node)?;

                Ok((node, Some(name)))
            }
            _ => {
                // declare new expression

                let expr = self.fold_node(node)?;
                let decl = Declaration::Expression(Box::from(expr));

                let id = self.context.declare(decl, span);

                let mut placeholder: Node = Item::Ident("<unnamed>".to_string()).into();
                placeholder.declared_at = Some(id);
                Ok((placeholder, None))
            }
        }
    }

    fn apply_context(&self, select: &mut Select) -> Result<()> {
        select.group = (self.context.frame.group)
            .iter()
            .map(|(_, id)| {
                let mut node: Node = Item::Ident("<un-materialized>".to_string()).into();
                node.declared_at = Some(*id);
                node
            })
            .collect();

        Ok(())
    }

    fn validate_function_call(
        &self,
        declared_at: Option<usize>,
        mut func_call: FuncCall,
        is_curry: bool,
    ) -> Result<(FuncCall, FuncDef), Error> {
        if declared_at.is_none() {
            return Err(Error::new(Reason::NotFound {
                name: func_call.name,
                namespace: "function".to_string(),
            }));
        }

        let func_dec = declared_at.unwrap();
        let func_dec = &self.context.declarations[func_dec].0;
        let func_def = func_dec.as_function().unwrap().clone();

        // extract needed named args from positionals
        let named_params: HashSet<_> = (func_def.named_params)
            .iter()
            .map(|param| &param.item.as_named_expr().unwrap().name)
            .collect();
        let (named, positional) = func_call.args.into_iter().partition(|arg| {
            // TODO: replace with drain_filter when it hits stable
            if let Item::NamedExpr(ne) = &arg.item {
                named_params.contains(&ne.name)
            } else {
                false
            }
        });
        func_call.args = positional;
        func_call.named_args = named
            .into_iter()
            .map(|arg| arg.item.into_named_expr().unwrap())
            .map(|ne| (ne.name, ne.expr))
            .collect();

        // validate number of parameters
        let expected_len = func_def.positional_params.len() - is_curry as usize;
        let passed_len = func_call.args.len();
        if expected_len != passed_len {
            let mut err = Error::new(Reason::Expected {
                who: Some(func_call.name.clone()),
                expected: format!("{} arguments", expected_len),
                found: format!("{}", passed_len),
            });

            if passed_len > expected_len && passed_len >= 2 {
                err = err.with_help(format!(
                    "If you are calling a function, you may want to add braces: `{} [{:?} {:?}]`",
                    func_call.name, func_call.args[0], func_call.args[1]
                ));
            }

            return Err(err);
        }

        Ok((func_call, func_def))
    }
}

fn extract_group_by(nodes: &[Node]) -> Result<Vec<(String, usize)>> {
    nodes
        .iter()
        .map(|n| Ok((n.item.clone().into_ident()?, n.declared_at.unwrap())))
        .try_collect()
}

fn extract_sorts(sort: Vec<ColumnSort>) -> Result<Vec<ColumnSort<usize>>> {
    sort.into_iter()
        .map(|s| {
            Ok(ColumnSort {
                column: (s.column.declared_at)
                    .ok_or_else(|| anyhow!("Unresolved ident in sort?"))?,
                direction: s.direction,
            })
        })
        .try_collect()
}

/// Loads `internal.prql` which contains type definitions of transforms
pub fn init_context() -> Context {
    use crate::parse;
    let transforms = include_str!("./transforms.prql");
    let transforms = parse(transforms).unwrap().nodes;

    let (_, context) = resolve(transforms, Some(Context::default())).unwrap();
    context
}

#[cfg(test)]
mod tests {
    use insta::{assert_snapshot, assert_yaml_snapshot};
    use serde_yaml::from_str;

    use crate::{parse, resolve_and_translate};

    use super::*;

    #[test]
    fn test_scopes_during_from() {
        let context = init_context();

        let mut resolver = Resolver::new(context);

        let pipeline: Node = from_str(
            r##"
            Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                    - Ident: employees
                    named_args: {}
        "##,
        )
        .unwrap();
        resolver.fold_node(pipeline).unwrap();

        assert_yaml_snapshot!(resolver.context.frame, @r###"
        ---
        columns:
          - All: 30
        sort: []
        group: []
        tables:
          - 30
        "###);
        assert!(resolver.context.scope.variables["employees.*"].len() == 1);
    }

    #[test]
    fn test_scopes_during_select() {
        let context = init_context();

        let mut resolver = Resolver::new(context);

        let pipeline: Node = from_str(
            r##"
            Pipeline:
              value: ~
              functions:
                - FuncCall:
                    name: from
                    args:
                    - Ident: employees
                    named_args: {}
                - FuncCall:
                    name: select
                    args:
                    - List:
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
                                - Raw: "1"
                        - Ident: age
                    named_args: {}
        "##,
        )
        .unwrap();
        resolver.fold_node(pipeline).unwrap();

        assert_eq!(resolver.context.frame.columns.len(), 3);

        assert!(resolver.context.scope.variables.contains_key("salary_1"));
        assert!(resolver.context.scope.variables.contains_key("salary_2"));
        assert!(resolver.context.scope.variables.contains_key("age"));
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
        let result: String = parse(prql).and_then(resolve_and_translate).unwrap();
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
        let result = parse(prql).and_then(resolve_and_translate).unwrap();
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
        let result = parse(prql).and_then(resolve_and_translate).unwrap();
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
        let result = parse(prql).and_then(resolve_and_translate).unwrap();
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
