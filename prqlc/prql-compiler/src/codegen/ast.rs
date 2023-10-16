use std::collections::HashSet;

use once_cell::sync::Lazy;

use prqlc_ast::expr::*;
use prqlc_ast::stmt::*;

use crate::codegen::SeparatedExprs;
use crate::utils::VALID_IDENT;

use super::{WriteOpt, WriteSource};

pub fn write_stmts(stmts: &Vec<Stmt>) -> String {
    let mut opt = WriteOpt::default();

    loop {
        if let Some(s) = stmts.write(opt.clone()) {
            break s;
        } else {
            opt.max_width += opt.max_width / 2;
        }
    }
}

pub fn write_expr(expr: &Expr) -> String {
    let opt = WriteOpt::new_width(u16::MAX);
    expr.write(opt).unwrap()
}

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
            Literal(literal) => Some(literal.to_string()),
            Case(cases) => {
                let mut r = String::new();
                r += "case ";
                r += &SeparatedExprs {
                    exprs: cases,
                    inline: ", ",
                    line_end: ",",
                }
                .write_between("[", "]", opt)?;
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

#[cfg(test)]
mod test {
    use insta::assert_snapshot;
    use similar_asserts::assert_eq;

    use super::*;

    fn assert_is_formatted(input: &str) {
        let stmt = format_single_stmt(input);

        assert_eq!(input.trim(), stmt.trim());
    }

    fn format_single_stmt(query: &str) -> String {
        use itertools::Itertools;
        let stmt = crate::prql_to_pl(query)
            .unwrap()
            .into_iter()
            .exactly_one()
            .unwrap();
        stmt.write(WriteOpt::default()).unwrap()
    }

    #[test]
    fn test_pipeline() {
        let short = Expr::new(ExprKind::Ident(Ident::from_path(vec!["short"])));
        let long = Expr::new(ExprKind::Ident(Ident::from_path(vec![
            "some_module",
            "submodule",
            "a_really_long_name",
        ])));

        let mut opt = WriteOpt {
            indent: 1,
            ..Default::default()
        };

        // short pipelines should be inlined
        let pipeline = Expr::new(ExprKind::Pipeline(Pipeline {
            exprs: vec![short.clone(), short.clone(), short.clone()],
        }));
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @"(short | short | short)");

        // long pipelines should be indented
        let pipeline = Expr::new(ExprKind::Pipeline(Pipeline {
            exprs: vec![short.clone(), long.clone(), long, short.clone()],
        }));
        // colons are a workaround to avoid trimming
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @r###"
        (
            short
            some_module.submodule.a_really_long_name
            some_module.submodule.a_really_long_name
            short
          )
        "###);

        // sometimes, there is just not enough space
        opt.rem_width = 4;
        opt.indent = 100;
        let pipeline = Expr::new(ExprKind::Pipeline(Pipeline { exprs: vec![short] }));
        assert!(pipeline.write(opt).is_none());
    }

    #[test]
    fn test_escaped_string() {
        assert_is_formatted(r#"filter name ~= "\\(I Can't Help\\) Falling""#);
    }

    #[test]
    fn test_double_braces() {
        assert_is_formatted(
            r#"let has_valid_title = s"regexp_contains(title, '([a-z0-9]*-){{2,}}')""#,
        );
    }

    #[test]
    fn test_unary() {
        assert_is_formatted(r#"sort {-duration}"#);

        assert_is_formatted(r#"select a = -b"#);
        assert_is_formatted(r#"join `project-bar.dataset.table` (==col_bax)"#)
    }

    #[test]
    fn test_simple() {
        assert_is_formatted(r#"aggregate average_country_salary = (average salary)"#);
    }

    #[test]
    fn test_assign() {
        assert_is_formatted(
            r#"
group {title, country} (aggregate {
  average salary,
  average gross_salary,
  sum salary,
  sum gross_salary,
  average gross_cost,
  sum_gross_cost = sum gross_cost,
  ct = count salary,
})"#,
        );
    }
    #[test]
    fn test_range() {
        assert_is_formatted(
            r#"
from foo
is_negative = -100..0
"#,
        );

        assert_is_formatted(
            r#"
from foo
is_negative = -(100..0)
"#,
        );
    }
}
