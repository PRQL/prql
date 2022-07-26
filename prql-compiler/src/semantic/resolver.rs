use std::collections::HashMap;
use std::iter::zip;

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::error::{Error, Reason, Span};

use super::scope::NS_FRAME;
use super::type_resolver::{resolve_type, type_of_func_def, validate_type};
use super::{Context, Declaration, Frame};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(query: Query, context: Context) -> Result<(Query, Context)> {
    let mut resolver = Resolver::new(context);

    let query = resolver.fold_query(query)?;

    Ok((query, resolver.context))
}

/// Can fold (walk) over AST and for each function call or variable find what they are referencing.
pub struct Resolver {
    pub context: Context,

    namespace: Namespace,
}

impl Resolver {
    fn new(context: Context) -> Self {
        Resolver {
            context,
            namespace: Namespace::FunctionsColumns,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Namespace {
    FunctionsColumns,
    Tables,
}

impl AstFold for Resolver {
    fn fold_node(&mut self, mut node: Node) -> Result<Node> {
        let mut r = match node.item {
            Item::Ident(ref ident) => {
                let id = self.lookup_name(ident, node.span)?;
                node.declared_at = Some(id);

                let decl = self.context.declarations.get(id);
                match decl {
                    // convert ident to function without args
                    Declaration::Function(_) => {
                        let curry = FuncCurry {
                            def_id: id,
                            args: vec![],
                            named_args: vec![],
                        };
                        self.fold_function(curry, vec![], HashMap::new(), node.span)?
                    }

                    // init type for tables
                    Declaration::Table(_) => Node {
                        ty: Some(Ty::Table(Frame::unknown(id))),
                        ..node
                    },

                    // init type for tables
                    Declaration::Expression(expr) => *expr.clone(),

                    _ => node,
                }
            }

            Item::FuncCall(FuncCall {
                name,
                args,
                named_args,
            }) => {
                // find function
                let curry = match name.item {
                    // by function name
                    Item::Ident(name) => {
                        let id = self.lookup_name(&name, node.span)?;

                        // construct an empty curry (this is a "fresh" call)
                        FuncCurry {
                            def_id: id,
                            args: vec![],
                            named_args: vec![],
                        }
                    }

                    // by using an inner curry
                    Item::FuncCurry(curry) => curry,

                    _ => todo!("throw an error"),
                };

                self.fold_function(curry, args, named_args, node.span)?
            }

            Item::Pipeline(Pipeline { mut nodes }) => {
                let value = nodes.remove(0);

                let mut value = self.fold_node(value)?;

                if let Item::FuncCurry(_) = &value.item {
                    // first value has evaluated to a function, which means we cannot
                    // evaluate the pipeline at the moment -> just keep the pipeline as is
                    nodes.insert(0, value);
                    node.item = Item::Pipeline(Pipeline { nodes });
                    node
                } else {
                    let mut work_stack = Vec::with_capacity(nodes.len());
                    work_stack.extend(nodes.into_iter().rev());
                    while let Some(node) = work_stack.pop() {
                        //// dbg!(&node);

                        let node = self.fold_node(node)?;

                        //// dbg!(&node);

                        value = match node.item {
                            Item::FuncCurry(func) => {
                                self.fold_function(func, vec![value], HashMap::new(), node.span)?
                            }
                            Item::Pipeline(Pipeline { nodes }) => {
                                work_stack.extend(nodes.into_iter().rev());
                                value
                            }
                            item => bail!(
                                "cannot apply argument `{}` to non-function: `{item}`",
                                value.item
                            ),
                        };
                    }
                    value
                }
            }

            Item::FuncCurry(_) => {
                // this can happen on occasional second resolve of same expression
                // in such case: skip any resolving
                node
            }

            item => {
                node.item = fold_item(self, item)?;
                node
            }
        };

        if r.ty.is_none() {
            r.ty = Some(resolve_type(&r)?)
        }
        Ok(r)
    }

    // save functions declarations
    fn fold_nodes(&mut self, items: Vec<Node>) -> Result<Vec<Node>> {
        // We cut out function def, so we need to run it
        // here rather than in `fold_func_def`.
        items
            .into_iter()
            .map(|node| {
                Ok(match node.item {
                    Item::FuncDef(func_def) => {
                        self.context.declare_func(func_def);
                        None
                    }
                    _ => Some(self.fold_node(node)?),
                })
            })
            .filter_map(|x| x.transpose())
            .try_collect()
    }
}

impl Resolver {
    pub fn lookup_name(&mut self, name: &str, span: Option<Span>) -> Result<usize> {
        match self.namespace {
            Namespace::Tables => {
                let id = self.context.declare_table(name.to_string(), None);
                Ok(id)
            }
            Namespace::FunctionsColumns => match self.context.lookup_name(name, span) {
                Ok(id) => Ok(id),
                Err(e) => bail!(Error::new(Reason::Simple(e)).with_span(span)),
            },
        }
    }

    fn fold_function(
        &mut self,
        curry: FuncCurry,
        args: Vec<Node>,
        named_args: HashMap<String, Box<Node>>,
        span: Option<Span>,
    ) -> Result<Node, anyhow::Error> {
        //dbg!(&curry);

        let id = Some(curry.def_id);
        let func_def = self.context.declarations.get_func(id)?.clone();

        let curry = self.apply_args_to_curry(curry, args, named_args, &func_def)?;
        let args_len = curry.args.len();

        let enough_args = args_len >= func_def.positional_params.len();

        let mut r = if enough_args {
            // eprintln!(
            //     "resolving function {} with args: {:#?}",
            //     func_def.name, curry.args
            // );

            // fold curry
            let curry = self.resolve_function_args(curry, &func_def)?;

            // eprintln!("materializing function {}", func_def.name);

            // evaluate
            match super::transforms::cast_transform(self, curry)? {
                Ok((preceding_pipeline, transform)) => {
                    // this function call is a transform, append it to the pipeline

                    let mut pipeline = preceding_pipeline.unwrap_or_else(|| {
                        Item::ResolvedQuery(ResolvedQuery { transforms: vec![] }).into()
                    });

                    let ty = pipeline.ty.as_ref().and_then(|t| t.as_table());
                    let frame_before = ty.cloned().unwrap_or_default();
                    let frame_after = transform.apply_to(frame_before)?;
                    pipeline.ty = Some(Ty::Table(frame_after.clone()));

                    let transform = Transform {
                        kind: transform,
                        ty: frame_after,
                        is_complex: false,
                    };

                    if let Some(query) = pipeline.item.as_resolved_query_mut() {
                        query.transforms.push(transform);
                    }

                    pipeline
                }
                Err(curry) => {
                    // this function call is not a transform, proceed with materialization

                    let param_namespace = format!("_param.{}", curry.def_id);

                    // declare each of the params and add it to scope
                    for (param, arg) in zip(func_def.named_params, curry.named_args) {
                        let value = arg.unwrap_or_else(|| param.default_value.unwrap());
                        let dec = Declaration::Expression(Box::new(value));
                        let id = self.context.declarations.push(dec, None);

                        self.context.scope.add(&param_namespace, &param.name, id);
                    }
                    for (param, arg) in zip(func_def.positional_params, curry.args) {
                        let value = arg.item.clone().into();
                        let dec = Declaration::Expression(Box::new(value));
                        let id = self.context.declarations.push(dec, None);

                        self.context.scope.add(&param_namespace, &param.name, id);
                    }

                    // dbg!(&self.context.scope);
                    // dbg!(&func_def.body);

                    // fold again, to resolve inner variables & functions
                    let body = self.fold_node(*func_def.body)?;

                    // dbg!(&body);

                    // TODO: late binding of args may cause args of some functions not to be resolved at this point
                    //  this means that when we drop this function's scope and param decls, they will never be resolved.
                    //  test case: func take_oldest n -> (sort [-age] | take n)

                    // remove param decls
                    let dropped = self.context.scope.drop(&param_namespace);
                    for id in dropped.values() {
                        self.context.declarations.forget(*id);
                    }

                    body
                }
            }
        } else {
            // not enough arguments: construct a func closure

            let mut node = Node::from(Item::FuncCurry(curry));

            let mut ty = type_of_func_def(&func_def);
            ty.args = ty.args[args_len..].to_vec();
            ty.named.clear();
            node.ty = Some(Ty::Function(ty));

            node
        };
        r.span = span;
        Ok(r)
    }

    fn apply_args_to_curry(
        &mut self,
        mut curry: FuncCurry,
        args: Vec<Node>,
        named_args: HashMap<Ident, Box<Node>>,
        func_def: &FuncDef,
    ) -> Result<FuncCurry> {
        for arg in args {
            curry.args.push(arg);
        }

        // named arguments are consumed only by the first function (non-curry)
        if !curry.named_args.is_empty() {
            if !named_args.is_empty() {
                bail!("function curry cannot accept named arguments");
            }
        } else {
            curry.named_args = func_def.named_params.iter().map(|_| None).collect();
            for (name, arg) in named_args {
                let (index, _) = func_def
                    .named_params
                    .iter()
                    .find_position(|p| p.name == name)
                    .ok_or_else(|| anyhow::anyhow!("unknown named argument"))?;

                curry.named_args[index] = Some(*arg);
            }
        }

        Ok(curry)
    }

    fn resolve_function_args(&mut self, curry: FuncCurry, func_def: &FuncDef) -> Result<FuncCurry> {
        let mut result = FuncCurry {
            def_id: curry.def_id,
            args: Vec::with_capacity(curry.args.len()),
            named_args: Vec::with_capacity(curry.named_args.len()),
        };

        {
            // positional args
            let mut frame_in_scope = false;
            for (index, arg) in curry.args.into_iter().enumerate().rev() {
                let param = &func_def.positional_params[index];

                let arg = self.resolve_function_arg(arg, param, &func_def.name)?;

                if !frame_in_scope {
                    // eprintln!("arg: {:#?}", arg);

                    if let Some(Ty::Table(frame)) = &arg.ty {
                        // eprintln!("add frame to scope: {frame}");
                        self.context.scope.add_frame_columns(frame);
                        frame_in_scope = true;
                    }
                }

                // push front (because of reverse resolve)
                result.args.insert(0, arg);
            }
            self.context.scope.drop(NS_FRAME);
        }

        {
            // named args
            for (index, arg) in curry.named_args.into_iter().enumerate() {
                if let Some(arg) = arg {
                    let param = &func_def.named_params[index];

                    let arg = self.resolve_function_arg(arg, param, &func_def.name)?;
                    result.named_args.push(Some(arg));
                } else {
                    result.named_args.push(None);
                }
            }
        }

        Ok(result)
    }

    fn resolve_function_arg(
        &mut self,
        arg: Node,
        param: &FuncParam,
        func_name: &str,
    ) -> Result<Node> {
        let prev_namespace = self.namespace;

        let mut arg = match param.ty.as_ref() {
            Some(Ty::BuiltinKeyword) => arg,
            Some(Ty::Table(_)) => {
                self.namespace = Namespace::Tables;
                self.fold_node(arg)?
            }
            _ => {
                self.namespace = Namespace::FunctionsColumns;
                self.fold_node(arg)?
            }
        };

        // validate type
        let param_ty = param.ty.as_ref().unwrap_or(&Ty::Infer);
        let assumed_ty = validate_type(&arg, param_ty, || Some(func_name.to_string()))?;
        arg.ty = Some(assumed_ty);

        self.namespace = prev_namespace;
        Ok(arg)
    }
}
