use std::collections::HashSet;

use once_cell::sync::Lazy;

use prql_ast::expr::*;
use prql_ast::stmt::*;

use crate::codegen::DisplayLiteral;
use crate::codegen::SeparatedExprs;
use crate::utils::VALID_IDENT;

use super::{WriteOpt, WriteSource};

fn write_within<T: WriteSource>(node: &T, parent: &ExprKind, mut opt: WriteOpt) -> Option<String> {
    let parent_strength = binding_strength(parent);
    opt.context_strength = opt.context_strength.max(parent_strength);

    node.write(opt)
}

impl WriteSource for Expr {
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

impl WriteSource for ExprKind {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        use ExprKind::*;

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
                    let start = write_within(start.as_ref(), self, opt.clone())?;
                    r += opt.consume(&start)?;
                }

                r += opt.consume("..")?;

                if let Some(end) = &range.end {
                    r += &write_within(end.as_ref(), self, opt)?;
                }
                Some(r)
            }
            Binary(BinaryExpr { op, left, right }) => {
                let mut r = String::new();

                let left = write_within(left.as_ref(), self, opt.clone())?;
                r += opt.consume(&left)?;

                r += opt.consume(" ")?;
                r += opt.consume(&op.to_string())?;
                r += opt.consume(" ")?;

                r += &write_within(right.as_ref(), self, opt)?;
                Some(r)
            }
            Unary(UnaryExpr { op, expr }) => {
                let mut r = String::new();

                r += opt.consume(&op.to_string())?;
                r += &write_within(expr.as_ref(), self, opt)?;
                Some(r)
            }
            FuncCall(func_call) => {
                let mut r = String::new();

                let name = write_within(func_call.name.as_ref(), self, opt.clone())?;
                r += opt.consume(&name)?;
                opt.unbound_expr = true;

                for (name, arg) in &func_call.named_args {
                    r += opt.consume(" ")?;

                    r += opt.consume(name)?;

                    r += opt.consume(":")?;

                    let arg = write_within(arg, self, opt.clone())?;
                    r += opt.consume(&arg)?;
                }
                for arg in &func_call.args {
                    r += opt.consume(" ")?;

                    let arg = write_within(arg, self, opt.clone())?;
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
                    r += &param.default_value.as_ref().unwrap().write(opt.clone())?;
                    r += " ";
                }
                r += "-> ";
                r += &c.body.write(opt)?;

                Some(r)
            }
            SString(parts) => display_interpolation("s", parts, opt),
            FString(parts) => display_interpolation("f", parts, opt),
            Literal(literal) => Some(DisplayLiteral(literal).to_string()),
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
        }
    }
}

fn binding_strength(expr: &ExprKind) -> u8 {
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
fn can_bind_left(expr: &ExprKind) -> bool {
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
    if VALID_IDENT.is_match(s) && !KEYWORDS.contains(s) {
        s.to_string()
    } else {
        format!("`{}`", s)
    }
}

impl WriteSource for Vec<Stmt> {
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

impl WriteSource for Stmt {
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
                    r += opt.consume(&format!("let {} = ", var_def.name))?;

                    r += &var_def.value.write(opt)?;
                    r += "\n";
                }
                VarDefKind::Into => {
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

                    r += &format!("into {}", var_def.name);
                    r += "\n";
                }
            },
            StmtKind::Main(value) => match &value.kind {
                ExprKind::Pipeline(pipeline) => {
                    for expr in &pipeline.exprs {
                        r += &expr.write(opt.clone())?;
                        r += "\n";
                    }
                }
                _ => {
                    r += &value.write(opt)?;
                }
            },
            StmtKind::TypeDef(type_def) => {
                r += opt.consume(&format!("let {}", type_def.name))?;

                if let Some(value) = &type_def.value {
                    r += opt.consume(" = ")?;
                    r += &value.write(opt)?;
                }
                r += "\n";
            }
            StmtKind::ModuleDef(module_def) => {
                r += &format!("module {} {{\n", module_def.name);
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

fn display_interpolation(prefix: &str, parts: &[InterpolateItem], opt: WriteOpt) -> Option<String> {
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

impl WriteSource for SwitchCase {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        let mut r = String::new();
        r += &self.condition.write(opt.clone())?;
        r += " => ";
        r += &self.value.write(opt)?;
        Some(r)
    }
}
