use std::collections::HashSet;

use once_cell::sync::Lazy;

use crate::{
    expr::{
        BinOp, BinaryExpr, Expr, ExprKind, Extension, InterpolateItem, SwitchCase, UnOp, UnaryExpr,
    },
    is_valid_ident,
    stmt::{Stmt, StmtKind, VarDefKind},
    Ident,
};

pub fn write<T: Extension>(stmts: &Vec<Stmt<T>>) -> String {
    let mut opt = WriteOpt::default();

    loop {
        if let Some(s) = stmts.write(opt.clone()) {
            break s;
        } else {
            opt.max_width += opt.max_width / 2;
        }
    }
}

impl<T: Extension> std::fmt::Display for Expr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let opt = WriteOpt::new_width(u16::MAX);
        f.write_str(&self.write(opt).unwrap())
    }
}

pub trait WriteSource {
    /// Converts self to its source representation according to specified
    /// options.
    ///
    /// Returns `None` if source does not fit into [WriteOpt::rem_width].
    fn write(&self, opt: WriteOpt) -> Option<String>;

    fn write_between<S: ToString>(
        &self,
        prefix: S,
        suffix: &str,
        mut opt: WriteOpt,
    ) -> Option<String> {
        let mut r = String::new();
        r += opt.consume(&prefix.to_string())?;
        opt.context_strength = 0;
        opt.unbound_expr = false;

        let source = self.write(opt.clone())?;
        r += opt.consume(&source)?;

        r += opt.consume(suffix)?;
        Some(r)
    }

    fn write_within<T: Extension>(
        &self,
        parent: &ExprKind<T>,
        mut opt: WriteOpt,
    ) -> Option<String> {
        let parent_strength = binding_strength(parent);
        opt.context_strength = opt.context_strength.max(parent_strength);

        self.write(opt)
    }
}

impl<T: WriteSource> WriteSource for &T {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        (*self).write(opt)
    }
}

#[derive(Clone)]
pub struct WriteOpt {
    /// String to emit as one indentation level
    pub tab: &'static str,

    /// Maximum number of characters per line
    pub max_width: u16,

    /// Current indent used when emitting lines
    pub indent: u16,

    /// Current remaining number of characters in line
    pub rem_width: u16,

    /// Strength of the context
    /// For top-level exprs or exprs in parenthesis, this will be 0.
    /// For exprs in function calls, this will be 10.
    pub context_strength: u8,

    /// True iff preceding source ends in an expression that could
    /// be mistakenly bound into a binary op by appending an unary op.
    ///
    /// For example:
    /// `join foo` has an unbound expr, since `join foo ==bar` produced a binary op.
    pub unbound_expr: bool,
}

impl Default for WriteOpt {
    fn default() -> Self {
        Self {
            tab: "  ",
            max_width: 50,

            indent: 0,
            rem_width: 50,
            context_strength: 0,
            unbound_expr: false,
        }
    }
}

impl WriteOpt {
    pub fn new_width(max_width: u16) -> Self {
        WriteOpt {
            max_width,
            rem_width: max_width,
            ..WriteOpt::default()
        }
    }

    fn consume_width(&mut self, width: u16) -> Option<()> {
        self.rem_width = self.rem_width.checked_sub(width)?;
        Some(())
    }

    fn reset_line(&mut self) -> Option<()> {
        let ident = self.tab.len() as u16 * self.indent;
        self.rem_width = self.max_width.checked_sub(ident)?;
        Some(())
    }

    fn consume<'a>(&mut self, source: &'a str) -> Option<&'a str> {
        let width = if let Some(new_line) = source.rfind('\n') {
            source.len() - new_line
        } else {
            source.len()
        };
        self.consume_width(width as u16);
        Some(source)
    }

    fn write_indent(&self) -> String {
        self.tab.repeat(self.indent as usize)
    }
}

impl<T: Extension> WriteSource for Expr<T> {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        let mut r = String::new();

        if let Some(alias) = &self.alias {
            r += alias;
            r += " = ";
            opt.unbound_expr = false;
        }

        let needs_parenthesis = (opt.unbound_expr && can_bind_left(&self.kind))
            || (opt.context_strength >= binding_strength(&self.kind));

        if needs_parenthesis {
            r += &self.kind.write_between("(", ")", opt)?;
        } else {
            r += &self.kind.write(opt)?;
        }
        Some(r)
    }
}

impl<T: Extension> WriteSource for ExprKind<T> {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        use crate::expr::ExprKind::*;

        match &self {
            Ident(ident) => ident.write(opt),
            Pipeline(pipeline) => SeparatedExprs {
                inline: " | ",
                line_end: "",
                exprs: &pipeline.exprs,
            }
            .write_between("(", ")", opt),

            Tuple(fields) => SeparatedExprs {
                exprs: fields,
                inline: ", ",
                line_end: ",",
            }
            .write_between("{", "}", opt),

            Array(items) => SeparatedExprs {
                exprs: items,
                inline: ", ",
                line_end: ",",
            }
            .write_between("[", "]", opt),

            Range(range) => {
                let mut r = String::new();
                if let Some(start) = &range.start {
                    let start = start.write_within(self, opt.clone())?;
                    r += opt.consume(&start)?;
                }

                r += opt.consume("..")?;

                if let Some(end) = &range.end {
                    r += &end.write_within(self, opt)?;
                }
                Some(r)
            }
            Binary(BinaryExpr { op, left, right }) => {
                let mut r = String::new();

                let left = left.write_within(self, opt.clone())?;
                r += opt.consume(&left)?;

                r += opt.consume(" ")?;
                r += opt.consume(&op.to_string())?;
                r += opt.consume(" ")?;

                r += &right.write_within(self, opt)?;
                Some(r)
            }
            Unary(UnaryExpr { op, expr }) => {
                let mut r = String::new();

                r += opt.consume(&op.to_string())?;
                r += &expr.write_within(self, opt)?;
                Some(r)
            }
            FuncCall(func_call) => {
                let mut r = String::new();

                let name = func_call.name.write_within(self, opt.clone())?;
                r += opt.consume(&name)?;
                opt.unbound_expr = true;

                for (name, arg) in &func_call.named_args {
                    r += opt.consume(" ")?;

                    r += opt.consume(name)?;

                    r += opt.consume(":")?;

                    let arg = arg.write_within(self, opt.clone())?;
                    r += opt.consume(&arg)?;
                }
                for arg in &func_call.args {
                    r += opt.consume(" ")?;

                    let arg = arg.write_within(self, opt.clone())?;
                    r += opt.consume(&arg)?;
                }
                Some(r)
            }
            Func(c) => {
                let mut r = String::new();
                for param in &c.params {
                    r += &write_ident_part(&param.name);
                    r += " ";
                }
                for param in &c.named_params {
                    r += &write_ident_part(&param.name);
                    r += ":";
                    r += &param.default_value.as_ref().unwrap().to_string();
                    r += " ";
                }
                r += "-> ";
                r += &c.body.to_string();

                Some(r)
            }
            SString(parts) => display_interpolation("s", parts, opt),
            FString(parts) => display_interpolation("f", parts, opt),
            Literal(literal) => Some(literal.to_string()),
            Case(cases) => {
                let mut r = String::new();
                r += "case ";
                r += &SeparatedExprs {
                    exprs: cases,
                    inline: ", ",
                    line_end: ",",
                }
                .write_between("{", "}", opt)?;
                Some(r)
            }
            Param(id) => Some(format!("${id}")),
            Internal(operator_name) => Some(format!("internal {operator_name}")),
            Other(_) => None,
        }
    }
}

fn binding_strength<T: Extension>(expr: &ExprKind<T>) -> u8 {
    match expr {
        // For example, if it's an Ident, it's basically infinite — a simple
        // ident never needs parentheses around it.
        ExprKind::Ident(_) => 100,

        // Stronger than a range, since `-1..2` is `(-1)..2`
        // Stronger than binary op, since `-x == y` is `(-x) == y`
        // Stronger than a func call, since `exists !y` is `exists (!y)`
        ExprKind::Unary(..) => 20,

        ExprKind::Range(_) => 19,

        ExprKind::Binary(BinaryExpr { op, .. }) => match op {
            BinOp::Mul | BinOp::DivInt | BinOp::DivFloat | BinOp::Mod => 18,
            BinOp::Add | BinOp::Sub => 17,
            BinOp::Eq
            | BinOp::Ne
            | BinOp::Gt
            | BinOp::Lt
            | BinOp::Gte
            | BinOp::Lte
            | BinOp::RegexSearch => 16,
            BinOp::Coalesce => 15,
            BinOp::And => 14,
            BinOp::Or => 13,
        },

        // Weaker than a child assign, since `select x = 1`
        // Weaker than a binary operator, since `filter x == 1`
        ExprKind::FuncCall(_) => 10,
        // ExprKind::FuncCall(_) if !is_parent => 2,
        ExprKind::Func(_) => 7,

        // other nodes should not contain any inner exprs
        _ => 100,
    }
}

/// True if this expression could be mistakenly bound with an expression on the left.
fn can_bind_left<T: Extension>(expr: &ExprKind<T>) -> bool {
    matches!(
        expr,
        ExprKind::Unary(UnaryExpr {
            op: UnOp::EqSelf | UnOp::Add | UnOp::Neg,
            ..
        })
    )
}

impl WriteSource for Ident {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        let width = self.path.iter().map(|p| p.len() + 1).sum::<usize>() + self.name.len();
        opt.consume_width(width as u16)?;

        let mut r = String::new();
        for part in &self.path {
            r += &write_ident_part(part);
            r += ".";
        }
        r += &write_ident_part(&self.name);
        Some(r)
    }
}

pub static KEYWORDS: Lazy<HashSet<&str>> = Lazy::new(|| {
    HashSet::from_iter([
        "let", "into", "case", "prql", "type", "module", "internal", "func",
    ])
});

pub fn write_ident_part(s: &str) -> String {
    if is_valid_ident(s) && !KEYWORDS.contains(s) {
        s.to_string()
    } else {
        format!("`{}`", s)
    }
}

impl<T: Extension> WriteSource for Vec<Stmt<T>> {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        opt.reset_line()?;

        let mut r = String::new();
        for stmt in self {
            if !r.is_empty() {
                r += "\n";
            }

            r += &opt.write_indent();
            r += &stmt.write(opt.clone())?;
        }
        Some(r)
    }
}

impl<T: Extension> WriteSource for Stmt<T> {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        let mut r = String::new();

        for annotation in &self.annotations {
            r += "@";
            r += &annotation.expr.write(opt.clone())?;
            r += "\n";
            r += &opt.write_indent();
            opt.reset_line()?;
        }

        match &self.kind {
            StmtKind::QueryDef(query) => {
                r += "prql";
                if let Some(version) = &query.version {
                    r += &format!(r#" version:"{}""#, version);
                }
                for (key, value) in &query.other {
                    r += &format!(" {key}:{value}");
                }
                r += "\n";
            }
            StmtKind::VarDef(var_def) => match var_def.kind {
                VarDefKind::Let => {
                    r += opt.consume(&format!("let {} = ", self.name))?;

                    r += &var_def.value.write(opt)?;
                    r += "\n";
                }
                VarDefKind::Into | VarDefKind::Main => {
                    match &var_def.value.kind {
                        ExprKind::Pipeline(pipeline) => {
                            for expr in &pipeline.exprs {
                                r += &expr.write(opt.clone())?;
                                r += "\n";
                            }
                        }
                        _ => {
                            r += &var_def.value.write(opt)?;
                        }
                    }

                    if let VarDefKind::Into = var_def.kind {
                        r += &format!("into {}", self.name);
                        r += "\n";
                    }
                }
            },
            StmtKind::TypeDef(type_def) => {
                r += opt.consume(&format!("let {}", self.name))?;

                if let Some(value) = &type_def.value {
                    r += opt.consume(" = ")?;
                    r += &value.write(opt)?;
                }
                r += "\n";
            }
            StmtKind::ModuleDef(module_def) => {
                r += &format!("module {} {{\n", self.name);
                opt.indent += 1;

                r += &module_def.stmts.write(opt.clone())?;

                opt.indent -= 1;
                r += &opt.write_indent();
                r += "}\n";
            }
        }
        Some(r)
    }
}

pub struct SeparatedExprs<'a, T: WriteSource> {
    pub exprs: &'a [T],
    pub inline: &'static str,
    pub line_end: &'static str,
}

impl<'a, T: WriteSource> WriteSource for SeparatedExprs<'a, T> {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        // try inline
        if let Some(inline) = self.write_inline(opt.clone()) {
            return Some(inline);
        }

        // one per line
        {
            opt.indent += 1;

            let mut r = String::new();

            for expr in self.exprs {
                r += "\n";
                r += &opt.write_indent();
                opt.reset_line()?;
                opt.rem_width.checked_sub(self.line_end.len() as u16)?;

                r += &expr.write(opt.clone())?;
                r += self.line_end;
            }
            opt.indent -= 1;
            r += "\n";
            r += &opt.write_indent();

            Some(r)
        }
    }
}

impl<'a, T: WriteSource> SeparatedExprs<'a, T> {
    fn write_inline(&self, mut opt: WriteOpt) -> Option<String> {
        let mut exprs = Vec::new();
        for expr in self.exprs {
            let expr = expr.write(opt.clone())?;

            if expr.contains('\n') {
                return None;
            }
            opt.consume_width(expr.len() as u16)?;

            exprs.push(expr);
        }

        let separators = self.inline.len() * (exprs.len().checked_sub(1).unwrap_or_default());
        opt.consume_width(separators as u16)?;

        Some(exprs.join(self.inline))
    }
}

fn display_interpolation<T: Extension>(
    prefix: &str,
    parts: &[InterpolateItem<Expr<T>>],
    opt: WriteOpt,
) -> Option<String> {
    let mut r = String::new();
    r += prefix;
    r += "\"";
    for part in parts {
        match &part {
            // We use double braces to escape braces
            InterpolateItem::String(s) => r += s.replace('{', "{{").replace('}', "}}").as_str(),
            InterpolateItem::Expr { expr, .. } => {
                r += "{";
                r += &expr.write(opt.clone())?;
                r += "}"
            }
        }
    }
    r += "\"";
    Some(r)
}

impl<T: Extension> WriteSource for SwitchCase<Box<Expr<T>>> {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        let mut r = String::new();
        r += &self.condition.write(opt.clone())?;
        r += " => ";
        r += &self.value.write(opt)?;
        Some(r)
    }
}
