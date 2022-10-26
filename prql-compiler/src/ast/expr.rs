use std::collections::HashMap;
use std::fmt::{Display, Write};

use anyhow::{anyhow, Result};
use enum_as_inner::EnumAsInner;
use semver::VersionReq;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Reason, Span};
use crate::ir::CId;
use crate::semantic::Declaration;

use super::*;

/// Expr is anything that has a value and thus a type.
/// If it cannot contain nested Exprs, is should be under [ExprKind::Literal].
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
    TransformCall(TransformCall),
    SString(Vec<InterpolateItem>),
    FString(Vec<InterpolateItem>),
}

impl ExprKind {
    pub fn parse_version(self) -> std::result::Result<VersionReq, Self> {
        match self {
            Self::Literal(Literal::String(ref s)) => VersionReq::parse(s).map_err(|_| self),
            _ => Err(self),
        }
    }
}

/// A name. Generally columns, tables, functions, variables.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Ident {
    pub namespace: Option<String>,
    pub name: String,
}

impl Ident {
    pub fn from_name<S: ToString>(name: S) -> Self {
        Ident {
            namespace: None,
            name: name.to_string(),
        }
    }
}

impl ToString for Ident {
    fn to_string(&self) -> String {
        match &self.namespace {
            Some(namespace) => format!("{}.{}", namespace, self.name),
            None => self.name.clone(),
        }
    }
}

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
    #[strum(to_string = "~")]
    EqSelf,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ListItem(pub Expr);

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: Box<Expr>,
    pub args: Vec<Expr>,
    pub named_args: HashMap<String, Expr>,
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
pub struct Window {
    pub kind: WindowKind,
    pub range: Range,
}

/// A value and a series of functions that are to be applied to that value one after another.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub exprs: Vec<Expr>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum InterpolateItem<T = Expr> {
    String(String),
    Expr(Box<T>),
}

/// Inclusive-inclusive range.
/// Missing bound means unbounded range.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range<T = Box<Expr>> {
    pub start: Option<T>,
    pub end: Option<T>,
}

impl<T> Range<T> {
    pub const fn unbounded() -> Self {
        Range {
            start: None,
            end: None,
        }
    }
}

impl Range {
    pub fn from_ints(start: Option<i64>, end: Option<i64>) -> Self {
        let start = start.map(|x| Box::new(Expr::from(ExprKind::Literal(Literal::Integer(x)))));
        let end = end.map(|x| Box::new(Expr::from(ExprKind::Literal(Literal::Integer(x)))));
        Range { start, end }
    }
}

/// FuncCall with better typing. Returns the modified table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TransformCall {
    pub kind: Box<TransformKind>,

    /// Grouping of values in columns
    pub partition: Vec<Expr>,

    /// Windowing of values in columns
    pub window: Window,

    /// Windowing of values in columns
    pub sort: Vec<ColumnSort>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, EnumAsInner)]
pub enum TransformKind {
    From(Expr),
    Derive {
        assigns: Vec<Expr>,
        tbl: Expr,
    },
    Select {
        assigns: Vec<Expr>,
        tbl: Expr,
    },
    Filter {
        filter: Box<Expr>,
        tbl: Expr,
    },
    Aggregate {
        assigns: Vec<Expr>,
        tbl: Expr,
    },
    Sort {
        by: Vec<ColumnSort<Expr>>,
        tbl: Expr,
    },
    Take {
        range: Range,
        tbl: Expr,
    },
    Join {
        side: JoinSide,
        with: Box<Expr>,
        filter: Box<Expr>,
        tbl: Expr,
    },

    Group {
        by: Vec<Expr>,
        pipeline: Box<Expr>,
        tbl: Expr,
    },

    Window {
        kind: WindowKind,
        range: Range,
        pipeline: Box<Expr>,
        tbl: Expr,
    },
}

impl TransformKind {
    pub fn tbl_arg_mut(&mut self) -> Option<&mut Expr> {
        use TransformKind::*;
        match self {
            From(_) => None,
            Derive { tbl, .. }
            | Select { tbl, .. }
            | Filter { tbl, .. }
            | Aggregate { tbl, .. }
            | Sort { tbl, .. }
            | Take { tbl, .. }
            | Join { tbl, .. }
            | Group { tbl, .. }
            | Window { tbl, .. } => Some(tbl),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum WindowKind {
    Rows,
    Range,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum TableRef {
    LocalTable(String),
    // TODO: add other sources such as files, URLs
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum JoinFilter<T = CId> {
    On(Vec<T>),
    Using(Vec<T>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

impl Expr {
    pub fn new_ident<S: ToString>(name: S, declared_at: usize) -> Expr {
        let mut node: Expr = ExprKind::Ident(Ident::from_name(name)).into();
        node.declared_at = Some(declared_at);
        node
    }

    pub fn null() -> Expr {
        Expr::from(ExprKind::Literal(Literal::Null))
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

    pub fn collect_and(mut exprs: Vec<Expr>) -> Expr {
        let mut aggregate = if let Some(first) = exprs.pop() {
            first
        } else {
            return Expr::from(ExprKind::Literal(Literal::Boolean(true)));
        };
        while let Some(e) = exprs.pop() {
            aggregate = Expr::from(ExprKind::Binary {
                left: Box::new(e),
                op: BinOp::And,
                right: Box::new(aggregate),
            })
        }
        aggregate
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

impl From<TransformKind> for TransformCall {
    fn from(kind: TransformKind) -> Self {
        TransformCall {
            kind: Box::new(kind),
            partition: Vec::new(),
            window: Window::default(),
            sort: Vec::new(),
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            kind: WindowKind::Rows,
            range: Range::unbounded(),
        }
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

impl From<TransformKind> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(kind: TransformKind) -> Self {
        // panic!("Failed to convert {item}")
        anyhow!("Failed to convert `{kind:?}`")
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(alias) = &self.alias {
            display_ident(f, alias)?;
            f.write_str(" = ")?;
        }

        match &self.kind {
            ExprKind::Ident(s) => {
                display_ident(f, s.to_string().as_str())?;
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
                UnOp::EqSelf => write!(f, "@{}", expr)?,
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
            ExprKind::TransformCall(transform) => {
                writeln!(f, "{} <unimplemented>", (*transform.kind).as_ref())?;
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
