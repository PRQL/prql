use std::collections::HashMap;
use std::fmt::{Display, Write};

use anyhow::anyhow;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum Item {
    Ident(Ident),
    Literal(Literal),
    Assign(NamedExpr),
    NamedArg(NamedExpr),
    Query(Query),
    Pipeline(Pipeline),
    Transform(Transform),
    List(Vec<Node>),
    Range(Range),
    Binary {
        left: Box<Node>,
        op: BinOp,
        right: Box<Node>,
    },
    Unary {
        op: UnOp,
        expr: Box<Node>,
    },
    FuncDef(FuncDef),
    FuncCall(FuncCall),
    Type(Ty),
    Table(Table),
    SString(Vec<InterpolateItem>),
    FString(Vec<InterpolateItem>),
    Interval(Interval),
    Windowed(Windowed),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::Display, strum::EnumString)]
pub enum BinOp {
    #[strum(to_string = "*")]
    Mul,
    #[strum(to_string = "/")]
    Div,
    #[strum(to_string = "%")]
    Mod,
    #[strum(to_string = "+")]
    Add,
    #[strum(to_string = "-")]
    Sub,
    #[strum(to_string = "==")]
    Eq,
    #[strum(to_string = "!=")]
    Ne,
    #[strum(to_string = ">")]
    Gt,
    #[strum(to_string = "<")]
    Lt,
    #[strum(to_string = ">=")]
    Gte,
    #[strum(to_string = "<=")]
    Lte,
    #[strum(to_string = "and")]
    And,
    #[strum(to_string = "or")]
    Or,
    #[strum(to_string = "??")]
    Coalesce,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::EnumString)]
pub enum UnOp {
    #[strum(to_string = "-")]
    Neg,
    #[strum(to_string = "!")]
    Not,
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

/// Represents a value and a series of function curries
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub nodes: Vec<Node>,
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

/// Inclusive-inclusive range.
/// Missing bound means unbounded range.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Range<T = Box<Node>> {
    pub start: Option<T>,
    pub end: Option<T>,
}

impl Range {
    pub const fn unbounded() -> Self {
        Range {
            start: None,
            end: None,
        }
    }

    pub fn from_ints(start: Option<i64>, end: Option<i64>) -> Self {
        let start = start.map(|x| Box::new(Node::from(Item::Literal(Literal::Integer(x)))));
        let end = end.map(|x| Box::new(Node::from(Item::Literal(Literal::Integer(x)))));
        Range { start, end }
    }

    pub fn into_int(self) -> Result<Range<i64>> {
        fn cast_bound(bound: Node) -> Result<i64> {
            Ok(bound.item.into_literal()?.into_integer()?)
        }

        Ok(Range {
            start: self.start.map(|b| cast_bound(*b)).transpose()?,
            end: self.end.map(|b| cast_bound(*b)).transpose()?,
        })
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
        self.nodes
            .into_iter()
            .map(|f| f.item.into_transform())
            .try_collect()
    }

    pub fn as_transforms(&self) -> Option<Vec<&Transform>> {
        self.nodes.iter().map(|f| f.item.as_transform()).collect()
    }
}
impl From<Vec<Node>> for Pipeline {
    fn from(nodes: Vec<Node>) -> Self {
        Pipeline { nodes }
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
                write!(f, "`{s}`")?;
            }
            Item::Assign(ne) => {
                match ne.expr.item {
                    Item::FuncCall(_) => {
                        write!(f, "{} = ({})", ne.name, ne.expr.item)?;
                    }

                    _ => {
                        write!(f, "{} = {}", ne.name, ne.expr.item)?;
                    }
                };
            }
            Item::NamedArg(ne) => {
                write!(f, "{}:{}", ne.name, ne.expr.item)?;
            }
            Item::Query(query) => {
                write!(f, "prql dialect:{}", query.dialect)?;
                if let Some(version) = query.version {
                    write!(f, " version:{}", version)?
                };
                write!(f, "\n\n")?;

                for node in &query.nodes {
                    match &node.item {
                        Item::Pipeline(p) => {
                            for node in &p.nodes {
                                writeln!(f, "{}", node.item)?;
                            }
                            f.write_char('\n')?;
                        }
                        _ => writeln!(f, "{}\n", node.item)?,
                    }
                }
            }
            Item::Pipeline(pipeline) => {
                f.write_char('(')?;
                match pipeline.nodes.len() {
                    0 => {}
                    1 => {
                        write!(f, "{}", pipeline.nodes[0].item)?;
                        for node in &pipeline.nodes[1..] {
                            write!(f, " | {}", node.item)?;
                        }
                    }
                    _ => {
                        writeln!(f, "\n  {}", pipeline.nodes[0].item)?;
                        for node in &pipeline.nodes[1..] {
                            writeln!(f, "  {}", node.item)?;
                        }
                    }
                }
                f.write_char(')')?;
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
                write!(f, " -> {}\n\n", func_def.body.item)?;
            }
            Item::Table(table) => {
                match table.pipeline.item {
                    Item::FuncCall(_) => {
                        write!(
                            f,
                            "table {} = (\n  {}\n)\n\n",
                            table.name, table.pipeline.item
                        )?;
                    }

                    _ => {
                        write!(f, "table {} = {}\n\n", table.name, table.pipeline.item)?;
                    }
                };
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
            Item::Binary { op, left, right } => {
                match left.item {
                    Item::FuncCall(_) => write!(f, "( {} )", left.item)?,
                    _ => write!(f, "{}", left.item)?,
                };
                write!(f, " {op} ")?;
                match right.item {
                    Item::FuncCall(_) => write!(f, "( {} )", right.item)?,
                    _ => write!(f, "{}", right.item)?,
                };
            }
            Item::Unary { op, expr } => match op {
                UnOp::Neg => write!(f, "-{}", expr.item)?,
                UnOp::Not => write!(f, "not {}", expr.item)?,
            },
            Item::FuncCall(func_call) => {
                f.write_str(func_call.name.as_str())?;

                for (name, arg) in &func_call.named_args {
                    write!(f, " {name}: {}", arg.item)?;
                }
                for arg in &func_call.args {
                    match arg.item {
                        Item::FuncCall(_) => {
                            writeln!(f, " (")?;
                            writeln!(f, "  {}", arg.item)?;
                            f.write_char(')')?;
                        }

                        _ => {
                            write!(f, " {}", arg.item)?;
                        }
                    }
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
                write!(f, "{}", w.expr.item)?;
            }
            Item::Type(typ) => {
                f.write_char('<')?;
                display_type(f, typ)?;
                f.write_char('>')?;
            }
            Item::Literal(literal) => {
                write!(f, "{}", literal)?;
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

fn display_type(f: &mut std::fmt::Formatter, typ: &Ty) -> Result<(), std::fmt::Error> {
    match typ {
        Ty::Literal(lit) => write!(f, "{:}", lit)?,
        Ty::Named(name) => write!(f, "{:}", name)?,
        Ty::Parameterized(t, param) => {
            display_type(f, t)?;
            write!(f, "<{:?}>", param.item)?
        }
        Ty::AnyOf(ts) => {
            for (i, t) in ts.iter().enumerate() {
                display_type(f, t)?;
                if i < ts.len() - 1 {
                    f.write_char('|')?;
                }
            }
        }
        Ty::Infer => {}
    }
    Ok(())
}
