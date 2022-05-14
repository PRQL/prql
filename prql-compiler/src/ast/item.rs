use std::{
    collections::HashMap,
    fmt::{Display, Write},
};

use anyhow::anyhow;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

pub use super::*;

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum Item {
    Ident(Ident),
    Operator(String),
    Literal(Literal),
    Assign(NamedExpr),
    NamedArg(NamedExpr),
    Query(Query),
    Pipeline(Pipeline),
    Transform(Transform),
    List(Vec<Node>),
    Range(Range),
    Expr(Vec<Node>),
    FuncDef(FuncDef),
    FuncCall(FuncCall),
    Type(Type),
    Table(Table),
    SString(Vec<InterpolateItem>),
    FString(Vec<InterpolateItem>),
    Interval(Interval),
    Windowed(Windowed),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ListItem(pub Node);

/// Function call.
///
/// Note that `named_args` cannot be determined during parsing, but only during resolving.
/// Until then, they are stored in args as named expression.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: Ident,
    pub args: Vec<Node>,
    pub named_args: HashMap<Ident, Box<Node>>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Windowed {
    pub expr: Box<Node>,
    pub group: Vec<Node>,
    pub sort: Vec<ColumnSort<Node>>,
    pub window: (WindowKind, Range),
}

impl Windowed {
    pub fn new(node: Node, window: (WindowKind, Range)) -> Self {
        Windowed {
            expr: Box::new(node),
            group: vec![],
            sort: vec![],
            window,
        }
    }
}
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub value: Option<Box<Node>>,
    pub functions: Vec<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct NamedExpr {
    pub name: Ident,
    pub expr: Box<Node>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum InterpolateItem {
    String(String),
    Expr(Box<Node>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Range {
    pub start: Option<Box<Node>>,
    pub end: Option<Box<Node>>,
}

impl Range {
    pub const fn unbounded() -> Self {
        Range {
            start: None,
            end: None,
        }
    }

    pub fn new_int(start: Option<i64>, end: Option<i64>) -> Self {
        let start = start.map(|x| Box::new(Node::from(Item::Literal(Literal::Integer(x)))));
        let end = end.map(|x| Box::new(Node::from(Item::Literal(Literal::Integer(x)))));
        Range { start, end }
    }
}

// I could imagine there being a wrapper of this to represent "2 days 3 hours".
// Or should that be written as `2days + 3hours`?
//
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// pub struct Interval(pub Vec<IntervalPart>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interval {
    pub n: i64,       // Do any DBs use floats or decimals for this?
    pub unit: String, // Could be an enum IntervalType,
}

impl Pipeline {
    pub fn into_transforms(self) -> Result<Vec<Transform>, Item> {
        self.functions
            .into_iter()
            .map(|f| f.item.into_transform())
            .try_collect()
    }

    pub fn as_transforms(&self) -> Option<Vec<&Transform>> {
        self.functions
            .iter()
            .map(|f| f.item.as_transform())
            .collect()
    }
}
impl From<Vec<Node>> for Pipeline {
    fn from(functions: Vec<Node>) -> Self {
        let value = None;
        Pipeline { functions, value }
    }
}

impl From<Item> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(item: Item) -> Self {
        // panic!("Failed to convert {item}")
        anyhow!("Failed to convert `{item}`")
    }
}

impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Ident(s) => {
                f.write_str(s)?;
            }
            Item::Operator(o) => {
                f.write_str(o)?;
            }
            Item::Assign(ne) => {
                write!(f, "{} = {}", ne.name, ne.expr.item)?;
            }
            Item::NamedArg(ne) => {
                write!(f, "{}:{}", ne.name, ne.expr.item)?;
            }
            Item::Query(query) => {
                write!(f, "prql dialect: {}\n\n", query.dialect)?;

                for node in &query.nodes {
                    match &node.item {
                        Item::Pipeline(p) => {
                            for node in &p.functions {
                                writeln!(f, "{}", node.item)?;
                            }
                        }
                        _ => write!(f, "{}", node.item)?,
                    }
                }
            }
            Item::Pipeline(pipeline) => {
                if let Some(value) = &pipeline.value {
                    write!(f, "({}", value.item)?;
                    for node in &pipeline.functions {
                        write!(f, " | {}", node.item)?;
                    }
                    f.write_char(')')?;
                } else {
                    f.write_str("(\n")?;
                    for node in &pipeline.functions {
                        writeln!(f, "  {}", node.item)?;
                    }
                    f.write_str(")")?;
                }
            }
            Item::Transform(transform) => {
                write!(f, "{} <unimplemented>", transform.as_ref())?;
            }
            Item::FuncDef(func_def) => {
                write!(f, "func {}", func_def.name)?;
                for arg in &func_def.positional_params {
                    write!(f, " {}", arg.0.item)?;
                }
                for arg in &func_def.named_params {
                    write!(f, " {}", arg.0.item)?;
                }
                write!(f, " = {}\n\n", func_def.body.item)?;
            }
            Item::Table(table) => {
                write!(f, "table {} = {}\n\n", table.name, table.pipeline.item)?;
            }
            Item::List(nodes) => {
                if nodes.is_empty() {
                    f.write_str("[]")?;
                } else if nodes.len() == 1 {
                    write!(f, "[{}]", nodes[0].item)?;
                } else {
                    f.write_str("[\n")?;
                    for li in nodes.iter() {
                        writeln!(f, "  {},", li.item)?;
                    }
                    f.write_str("]")?;
                }
            }
            Item::Range(r) => {
                if let Some(start) = &r.start {
                    write!(f, "{}", start.item)?;
                }
                f.write_str("..")?;
                if let Some(end) = &r.end {
                    write!(f, "{}", end.item)?;
                }
            }
            Item::Expr(nodes) => {
                for (i, node) in nodes.iter().enumerate() {
                    write!(f, "{}", node.item)?;
                    if i + 1 < nodes.len() {
                        f.write_char(' ')?;
                    }
                }
            }
            Item::FuncCall(func_call) => {
                f.write_str(func_call.name.as_str())?;

                for (name, arg) in &func_call.named_args {
                    write!(f, " {name}: {}", arg.item)?;
                }
                for arg in &func_call.args {
                    write!(f, " {}", arg.item)?;
                }
            }
            Item::SString(parts) => {
                display_interpolation(f, "s", parts)?;
            }
            Item::FString(parts) => {
                display_interpolation(f, "f", parts)?;
            }
            Item::Interval(i) => {
                write!(f, "{}{}", i.n, i.unit)?;
            }
            Item::Windowed(w) => {
                write!(f, "{:?}", w.expr)?;
            }
            Item::Type(typ) => {
                f.write_char('<')?;
                display_type(f, typ)?;
                f.write_char('>')?;
            }
            Item::Literal(literal) => {
                write!(f, "{:?}", literal)?;
            }
        }
        Ok(())
    }
}

fn display_interpolation(
    f: &mut std::fmt::Formatter,
    prefix: &str,
    parts: &[InterpolateItem],
) -> Result<(), std::fmt::Error> {
    f.write_str(prefix)?;
    f.write_char('"')?;
    for part in parts {
        match &part {
            InterpolateItem::String(s) => write!(f, "{s}")?,
            InterpolateItem::Expr(e) => write!(f, "{{{}}}", e.item)?,
        }
    }
    f.write_char('"')?;
    Ok(())
}

fn display_type(f: &mut std::fmt::Formatter, typ: &Type) -> Result<(), std::fmt::Error> {
    match typ {
        Type::Native(kind) => write!(f, "{:}", kind)?,
        Type::Parameterized(t, param) => {
            display_type(f, t)?;
            write!(f, "<{:?}>", param.item)?
        }
        Type::AnyOf(ts) => {
            for (i, t) in ts.iter().enumerate() {
                display_type(f, t)?;
                if i < ts.len() - 1 {
                    f.write_char('|')?;
                }
            }
        }
    }
    Ok(())
}
