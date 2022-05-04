use std::collections::HashSet;

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::ast_fold::*;
use crate::error::{Error, Reason, WithErrorInfo};
use crate::{ast::*, Declaration};

use super::frame::extract_sorts;
use super::transforms;
use super::Context;

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

    within_group: Vec<usize>,

    sorted: Vec<ColumnSort<usize>>,
}
impl Resolver {
    fn new(context: Context) -> Self {
        Resolver {
            context,
            within_curry: false,
            within_group: vec![],
            sorted: vec![],
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
                        self.context.scope.clear();

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
                match func_def.kind {
                    Some(FuncKind::Transform) => {
                        let transform = transforms::cast_transform(func_call, node.span)?;

                        Item::Transform(self.fold_transform(transform)?)
                    }
                    Some(FuncKind::Window) => {
                        // wrap into Windowed
                        let mut expr: Node = Item::FuncCall(self.fold_func_call(func_call)?).into();
                        expr.declared_at = node.declared_at;
                        node.declared_at = None;

                        Item::Windowed(Windowed::new(expr))
                    }
                    _ => Item::FuncCall(self.fold_func_call(func_call)?),
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
        let mut t = match t {
            Transform::From(mut t) => {
                self.sorted.clear();

                self.context.scope.clear();

                self.context.declare_table(&mut t);

                Transform::From(t)
            }

            Transform::Select(assigns) => {
                let assigns = self.fold_assigns(assigns)?;
                self.context.scope.clear();

                Transform::Select(assigns)
            }
            Transform::Derive(assigns) => {
                let assigns = self.fold_assigns(assigns)?;

                Transform::Derive(assigns)
            }
            Transform::Group { by, pipeline } => {
                let by = self.fold_nodes(by)?;

                self.within_group = by.iter().filter_map(|n| n.declared_at).collect();
                let sorted = self.sorted.clone();

                let pipeline = Box::new(self.fold_node(*pipeline)?);

                self.within_group = vec![];
                self.sorted = sorted;

                Transform::Group { by, pipeline }
            }
            Transform::Aggregate { assigns, by } => {
                let assigns = self.fold_assigns(assigns)?;
                self.context.scope.clear();

                Transform::Aggregate { assigns, by }
            }
            Transform::Join {
                side,
                mut with,
                filter,
            } => {
                self.context.declare_table(&mut with);

                Transform::Join {
                    side,
                    with,
                    filter: self.fold_join_filter(filter)?,
                }
            }
            Transform::Sort(sorts) => {
                let sorts = self.fold_column_sorts(sorts)?;

                self.sorted = extract_sorts(&sorts)?;

                Transform::Sort(sorts)
            }
            t => fold_transform(self, t)?,
        };

        if !self.within_group.is_empty() {
            self.apply_group(&mut t)?;
        }
        if !self.sorted.is_empty() {
            self.apply_sort(&mut t)?;
        }

        Ok(t)
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

                    node.declared_at = Some(id);
                }
                JoinFilter::Using(nodes)
            }
        })
    }

    fn fold_table(&mut self, mut table: Table) -> Result<Table> {
        // fold pipeline
        table.pipeline = Box::new(self.fold_node(*table.pipeline)?);

        // declare table
        let decl = Declaration::Table(table.name.clone());
        table.id = Some(self.context.declare(decl, None));

        Ok(table)
    }
}

impl Resolver {
    fn fold_assigns(&mut self, nodes: Vec<Node>) -> Result<Vec<Node>> {
        nodes
            .into_iter()
            .map(|mut node| {
                Ok(match node.item {
                    Item::Assign(NamedExpr { name, expr }) => {
                        // introduce a new expression alias

                        let expr = self.fold_assign_expr(*expr)?;
                        let id = expr.declared_at.unwrap();

                        self.context.scope.add(name.clone(), id);

                        node.item = Item::Ident(name);
                        node.declared_at = Some(id);
                        node
                    }
                    _ => {
                        // no new names, only fold the expr
                        self.fold_assign_expr(node)?
                    }
                })
            })
            .try_collect()
    }

    fn fold_assign_expr(&mut self, node: Node) -> Result<Node> {
        let span = node.span;

        Ok(match node.item {
            Item::Ident(_) => {
                // keep existing ident
                self.fold_node(node)?
            }
            _ => {
                // declare new expression so it can be references from FrameColumn
                let expr = self.fold_node(node)?;
                let decl = Declaration::Expression(Box::from(expr));

                let id = self.context.declare(decl, span);

                let mut placeholder: Node = Item::Ident("<unnamed>".to_string()).into();
                placeholder.declared_at = Some(id);
                placeholder
            }
        })
    }

    fn apply_group(&mut self, t: &mut Transform) -> Result<()> {
        const REF: &str = "<ref>";

        let group_by: Vec<_> = (self.within_group)
            .iter()
            .map(|id| Node::new_ident(REF, *id))
            .collect();

        match t {
            Transform::Select(assigns) | Transform::Derive(assigns) => {
                for assign in assigns {
                    let id = assign.declared_at.unwrap();

                    // try to update existing Windowed
                    let decl = self.context.declarations.get_mut(id);
                    if let Some((Declaration::Expression(node), _)) = decl {
                        if let Item::Windowed(window) = &mut node.item {
                            window.group = group_by.clone();
                            continue;
                        }
                    }

                    // wrap with new Windowed declaration
                    let windowed = Windowed {
                        expr: Box::new(Node::new_ident(REF, id)),
                        group: group_by.clone(),
                        sort: vec![],
                    };
                    let decl = Declaration::Expression(Box::new(Item::Windowed(windowed).into()));
                    let window_id = self.context.declare(decl, None);
                    assign.declared_at = Some(window_id);
                }
            }
            Transform::Aggregate { by, .. } => {
                *by = group_by;
            }
            Transform::Sort(_) => {}
            _ => {
                // TODO: attach span to this error
                bail!(Error::new(Reason::Simple(format!(
                    "transform `{}` is not allowed within group context",
                    t.as_ref()
                ))))
            }
        }
        Ok(())
    }

    fn apply_sort(&mut self, t: &mut Transform) -> Result<()> {
        match t {
            Transform::Select(assigns) | Transform::Derive(assigns) => {
                let sort: Vec<_> = (self.sorted)
                    .iter()
                    .map(|s| ColumnSort {
                        column: Node::new_ident("<ref>", s.column),
                        direction: s.direction.clone(),
                    })
                    .collect();

                for assign in assigns {
                    let id = assign.declared_at.unwrap();

                    let decl = self.context.declarations.get_mut(id);
                    if let Some((Declaration::Expression(node), _)) = decl {
                        if let Item::Windowed(window) = &mut node.item {
                            window.sort = sort.clone();
                        }
                    }
                }
            }
            _ => {}
        }
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
            .map(|param| &param.item.as_named_arg().unwrap().name)
            .collect();
        let (named, positional) = func_call
            .args
            .into_iter()
            .partition(|arg| matches!(&arg.item, Item::NamedArg(_)));
        func_call.args = positional;

        for node in named {
            let arg = node.item.into_named_arg().unwrap();
            if !named_params.contains(&arg.name) {
                return Err(Error::new(Reason::Unexpected {
                    found: format!("argument named `{}`", arg.name),
                })
                .with_span(node.span));
            }
            func_call.named_args.insert(arg.name, arg.expr);
        }

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
    use insta::assert_snapshot;
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
                        - Assign:
                            name: salary_1
                            expr:
                                Ident: salary
                        - Assign:
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
        select [salary1 = salary, salary2 = salary1 + 1, age]
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

    #[test]
    fn test_applying_group_context() {
        let prql = r#"
        from employees
        group last_name (
            sort first_name
            take 1
        )
        "#;
        let result = parse(prql).and_then(resolve_and_translate);
        assert!(result.is_err());

        let prql = r#"
        from employees
        group last_name (
            select first_name
        )
        "#;
        let result = parse(prql).and_then(resolve_and_translate).unwrap();
        assert_snapshot!(result, @r###"
        SELECT
          first_name OVER (PARTITION BY last_name) AS first_name
        FROM
          employees
        "###);

        let prql = r#"
        from employees
        group last_name (
            aggregate count
        )
        "#;
        let result = parse(prql).and_then(resolve_and_translate).unwrap();
        assert_snapshot!(result, @r###"
        SELECT
          last_name,
          COUNT(*) AS count
        FROM
          employees
        GROUP BY
          last_name
        "###);
    }
}
