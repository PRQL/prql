use std::collections::HashMap;
use std::fmt::{Display, Write};

use anyhow::{anyhow, Result};
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Reason, Span};
use crate::semantic::Declaration;

use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    #[serde(flatten)]
    pub kind: ExprKind,
    #[serde(skip)]
    pub span: Option<Span>,
    #[serde(skip)]
    pub declared_at: Option<usize>,

    /// Type of expression this node represents. [None] means type has not yet been determined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,

    /// Is true when containing window functions
    #[serde(skip)]
    pub is_complex: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum ExprKind {
    Empty,
    Ident(Ident),
    Literal(Literal),
    Pipeline(Pipeline),
    List(Vec<Expr>),
    Range(Range),
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    FuncCall(FuncCall),
    Closure(Closure),
    Type(Ty),
    SString(Vec<InterpolateItem>),
    FString(Vec<InterpolateItem>),
    Interval(Interval),
    Windowed(Windowed),

    /// Resolved table transforms.
    ResolvedPipeline(Vec<Transform>),
}

/// A name. Generally columns, tables, functions, variables.
pub type Ident = String;

#[derive(
    Debug, PartialEq, Eq, Clone, Serialize, Deserialize, strum::Display, strum::EnumString,
)]
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

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, strum::EnumString)]
pub enum UnOp {
    #[strum(to_string = "-")]
    Neg,
    #[strum(to_string = "!")]
    Not,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ListItem(pub Expr);

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: Box<Expr>,
    pub args: Vec<Expr>,
    pub named_args: HashMap<Ident, Expr>,
}

impl FuncCall {
    pub fn without_args(name: Expr) -> Self {
        FuncCall {
            name: Box::new(name),
            args: vec![],
            named_args: HashMap::new(),
        }
    }
}
/// Function called with possibly missing positional arguments.
/// May also contain environment that is needed to evaluate the body.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Closure {
    pub name: Option<String>,
    pub body: Box<Expr>,

    pub args: Vec<Expr>,
    pub params: Vec<FuncParam>,

    pub named_args: Vec<Option<Expr>>,
    pub named_params: Vec<FuncParam>,

    pub env: HashMap<String, Declaration>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Windowed {
    pub expr: Box<Expr>,
    pub group: Vec<Expr>,
    pub sort: Vec<ColumnSort<Expr>>,
    pub window: (WindowKind, Range),
}

impl Windowed {
    pub fn new(node: Expr, window: (WindowKind, Range)) -> Self {
        Windowed {
            expr: Box::new(node),
            group: vec![],
            sort: vec![],
            window,
        }
    }
}

/// A value and a series of functions that are to be applied to that value one after another.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub exprs: Vec<Expr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum InterpolateItem {
    String(String),
    Expr(Box<Expr>),
}

/// Inclusive-inclusive range.
/// Missing bound means unbounded range.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range<T = Box<Expr>> {
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
        let start = start.map(|x| Box::new(Expr::from(ExprKind::Literal(Literal::Integer(x)))));
        let end = end.map(|x| Box::new(Expr::from(ExprKind::Literal(Literal::Integer(x)))));
        Range { start, end }
    }

    pub fn into_int(self) -> Result<Range<i64>> {
        fn cast_bound(bound: Expr) -> Result<i64> {
            Ok(bound.kind.into_literal()?.into_integer()?)
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Interval {
    pub n: i64,       // Do any DBs use floats or decimals for this?
    pub unit: String, // Could be an enum IntervalType,
}

impl Expr {
    pub fn new_ident<S: ToString>(name: S, declared_at: usize) -> Expr {
        let mut node: Expr = ExprKind::Ident(name.to_string()).into();
        node.declared_at = Some(declared_at);
        node
    }

    pub fn coerce_into_vec(self) -> Vec<Expr> {
        match self.kind {
            ExprKind::List(items) => items,
            _ => vec![self],
        }
    }

    pub fn coerce_as_mut_vec(&mut self) -> Vec<&mut Expr> {
        if matches!(self.kind, ExprKind::List(_)) {
            match &mut self.kind {
                ExprKind::List(items) => items.iter_mut().collect(),
                _ => unreachable!(),
            }
        } else {
            vec![self]
        }
    }

    pub fn try_cast<T, F, S2: ToString>(
        self,
        f: F,
        who: Option<&str>,
        expected: S2,
    ) -> Result<T, Error>
    where
        F: FnOnce(ExprKind) -> Result<T, ExprKind>,
    {
        f(self.kind).map_err(|i| {
            Error::new(Reason::Expected {
                who: who.map(|s| s.to_string()),
                expected: expected.to_string(),
                found: format!("`{}`", Expr::from(i)),
            })
            .with_span(self.span)
        })
    }
}

impl From<ExprKind> for Expr {
    fn from(item: ExprKind) -> Self {
        Expr {
            kind: item,
            span: None,
            declared_at: None,
            ty: None,
            is_complex: false,
            alias: None,
        }
    }
}

impl From<Vec<Expr>> for Pipeline {
    fn from(nodes: Vec<Expr>) -> Self {
        Pipeline { exprs: nodes }
    }
}

impl From<ExprKind> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(kind: ExprKind) -> Self {
        // panic!("Failed to convert {item}")
        anyhow!("Failed to convert `{}`", Expr::from(kind))
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(alias) = &self.alias {
            display_ident(f, alias)?;
            f.write_str(" = ")?;
        }

        match &self.kind {
            ExprKind::Empty => {
                f.write_str("()")?;
            }
            ExprKind::Ident(s) => {
                display_ident(f, s)?;
            }
            ExprKind::Pipeline(pipeline) => {
                f.write_char('(')?;
                match pipeline.exprs.len() {
                    0 => {}
                    1 => {
                        write!(f, "{}", pipeline.exprs[0])?;
                        for node in &pipeline.exprs[1..] {
                            write!(f, " | {}", node)?;
                        }
                    }
                    _ => {
                        writeln!(f, "\n  {}", pipeline.exprs[0])?;
                        for node in &pipeline.exprs[1..] {
                            writeln!(f, "  {}", node)?;
                        }
                    }
                }
                f.write_char(')')?;
            }
            ExprKind::List(nodes) => {
                if nodes.is_empty() {
                    f.write_str("[]")?;
                } else if nodes.len() == 1 {
                    write!(f, "[{}]", nodes[0])?;
                } else {
                    f.write_str("[\n")?;
                    for li in nodes.iter() {
                        writeln!(f, "  {},", li)?;
                    }
                    f.write_str("]")?;
                }
            }
            ExprKind::Range(r) => {
                if let Some(start) = &r.start {
                    write!(f, "{}", start)?;
                }
                f.write_str("..")?;
                if let Some(end) = &r.end {
                    write!(f, "{}", end)?;
                }
            }
            ExprKind::Binary { op, left, right } => {
                match left.kind {
                    ExprKind::FuncCall(_) => write!(f, "( {} )", left)?,
                    _ => write!(f, "{}", left)?,
                };
                write!(f, " {op} ")?;
                match right.kind {
                    ExprKind::FuncCall(_) => write!(f, "( {} )", right)?,
                    _ => write!(f, "{}", right)?,
                };
            }
            ExprKind::Unary { op, expr } => match op {
                UnOp::Neg => write!(f, "-{}", expr)?,
                UnOp::Not => write!(f, "not {}", expr)?,
            },
            ExprKind::FuncCall(func_call) => {
                write!(f, "{:}", func_call.name)?;

                for (name, arg) in &func_call.named_args {
                    write!(f, " {name}: {}", arg)?;
                }
                for arg in &func_call.args {
                    match arg.kind {
                        ExprKind::FuncCall(_) => {
                            writeln!(f, " (")?;
                            writeln!(f, "  {}", arg)?;
                            f.write_char(')')?;
                        }

                        _ => {
                            write!(f, " {}", arg)?;
                        }
                    }
                }
            }
            ExprKind::Closure(c) => {
                write!(
                    f,
                    "<closure over {} with {}/{} args>",
                    &c.body,
                    c.args.len(),
                    c.params.len()
                )?;
            }
            ExprKind::SString(parts) => {
                display_interpolation(f, "s", parts)?;
            }
            ExprKind::FString(parts) => {
                display_interpolation(f, "f", parts)?;
            }
            ExprKind::Interval(i) => {
                write!(f, "{}{}", i.n, i.unit)?;
            }
            ExprKind::Windowed(w) => {
                write!(f, "{}", w.expr)?;
            }
            ExprKind::ResolvedPipeline(transforms) => {
                for transform in transforms {
                    writeln!(f, "{} <unimplemented>", transform.kind.as_ref())?;
                }
            }
            ExprKind::Type(typ) => {
                f.write_char('<')?;
                write!(f, "{typ}")?;
                f.write_char('>')?;
            }
            ExprKind::Literal(literal) => {
                write!(f, "{}", literal)?;
            }
        }
        Ok(())
    }
}

fn display_ident(f: &mut std::fmt::Formatter, s: &str) -> Result<(), std::fmt::Error> {
    fn forbidden_start(c: char) -> bool {
        !(('a'..='z').contains(&c) || matches!(c, '_' | '$'))
    }
    fn forbidden_subsequent(c: char) -> bool {
        !(('a'..='z').contains(&c) || ('0'..='9').contains(&c) || matches!(c, '_'))
    }
    let needs_escape = s.is_empty()
        || s.starts_with(forbidden_start)
        || (s.len() > 1 && s.chars().skip(1).any(forbidden_subsequent));

    if needs_escape {
        write!(f, "`{s}`")
    } else {
        write!(f, "{s}")
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
            InterpolateItem::Expr(e) => write!(f, "{{{e}}}")?,
        }
    }
    f.write_char('"')?;
    Ok(())
}
