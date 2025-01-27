use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use super::{WriteOpt, WriteSource};
use crate::codegen::SeparatedExprs;
use crate::pr;

pub(crate) fn write_expr(expr: &pr::Expr) -> String {
    expr.write(WriteOpt::new_width(u16::MAX)).unwrap()
}

fn write_within<T: WriteSource>(
    node: &T,
    parent: &pr::ExprKind,
    mut opt: WriteOpt,
) -> Option<String> {
    let parent_strength = binding_strength(parent);
    opt.context_strength = opt.context_strength.max(parent_strength);

    node.write(opt)
}

impl WriteSource for pr::Expr {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        let mut r = String::new();

        if let Some(alias) = &self.alias {
            r += opt.consume(&write_ident_part(alias))?;
            r += opt.consume(" = ")?;
            opt.unbound_expr = false;
        }

        if !needs_parenthesis(self, &opt) {
            r += &self.kind.write(opt.clone())?;
        } else {
            let value = self.kind.write_between("(", ")", opt.clone());

            if let Some(value) = value {
                r += &value;
            } else {
                r += &break_line_within_parenthesis(&self.kind, opt)?;
            }
        };
        Some(r)
    }
}

fn needs_parenthesis(this: &pr::Expr, opt: &WriteOpt) -> bool {
    if opt.unbound_expr && can_bind_left(&this.kind) {
        return true;
    }

    let binding_strength = binding_strength(&this.kind);
    if opt.context_strength > binding_strength {
        // parent has higher binding strength, which means it would "steal" operand of this expr
        // => parenthesis are needed
        return true;
    }

    if opt.context_strength < binding_strength {
        // parent has higher binding strength, which means it would "steal" operand of this expr
        // => parenthesis are needed
        return false;
    }

    // parent has equal binding strength, which means that now associativity of this expr counts
    // for example:
    //   this=(a + b), parent=(a + b) + c
    //   asoc of + is left
    //   this is the left operand of parent
    //   => assoc_matches=true => we don't need parenthesis

    //   this=(a + b), parent=c + (a + b)
    //   asoc of + is left
    //   this is the right operand of parent
    //   => assoc_matches=false => we need parenthesis
    let assoc_matches = match opt.binary_position {
        super::Position::Left => associativity(&this.kind) == super::Position::Left,
        super::Position::Right => associativity(&this.kind) == super::Position::Right,
        super::Position::Unspecified => false,
    };

    !assoc_matches
}

impl WriteSource for pr::ExprKind {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        use pr::ExprKind::*;

        match &self {
            Ident(ident) => Some(ident.to_string()),

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
            Binary(pr::BinaryExpr { op, left, right }) => {
                let mut r = String::new();

                let mut opt_left = opt.clone();
                opt_left.binary_position = super::Position::Left;
                let left = write_within(left.as_ref(), self, opt_left)?;
                r += opt.consume(&left)?;

                r += opt.consume(" ")?;
                r += opt.consume(&op.to_string())?;
                r += opt.consume(" ")?;

                let mut opt_right = opt;
                opt_right.binary_position = super::Position::Right;
                r += &write_within(right.as_ref(), self, opt_right)?;
                Some(r)
            }
            Unary(pr::UnaryExpr { op, expr }) => {
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
                let mut r = "func ".to_string();

                for param in &c.params {
                    r += opt.consume(&write_ident_part(&param.name))?;
                    r += opt.consume(" ")?;
                    if let Some(ty) = &param.ty {
                        let ty = ty.write_between("<", ">", opt.clone())?;
                        r += opt.consume(&ty)?;
                        r += opt.consume(" ")?;
                    }
                }
                for param in &c.named_params {
                    r += opt.consume(&write_ident_part(&param.name))?;
                    r += opt.consume(":")?;
                    r += opt.consume(&param.default_value.as_ref().unwrap().write(opt.clone())?)?;
                    r += opt.consume(" ")?;
                }
                r += opt.consume("-> ")?;

                if let Some(ty) = &c.return_ty {
                    let ty = ty.write_between("<", ">", opt.clone())?;
                    r += opt.consume(&ty)?;
                    r += opt.consume(" ")?;
                }

                // try a single line
                if let Some(body) = c.body.write(opt.clone()) {
                    r += &body;
                } else {
                    r += &break_line_within_parenthesis(c.body.as_ref(), opt)?;
                }

                Some(r)
            }
            SString(parts) => display_interpolation("s", parts, opt),
            FString(parts) => display_interpolation("f", parts, opt),
            Literal(literal) => opt.consume(literal.to_string()),
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

fn break_line_within_parenthesis<T: WriteSource>(expr: &T, mut opt: WriteOpt) -> Option<String> {
    let mut r = "(\n".to_string();
    opt.indent += 1;
    r += &opt.write_indent();
    opt.reset_line()?;
    r += &expr.write(opt.clone())?;
    r += "\n";
    opt.indent -= 1;
    r += &opt.write_indent();
    r += ")";
    Some(r)
}

fn binding_strength(expr: &pr::ExprKind) -> u8 {
    match expr {
        // For example, if it's an Ident, it's basically infinite — a simple
        // ident never needs parentheses around it.
        pr::ExprKind::Ident(_) => 100,

        // Stronger than a range, since `-1..2` is `(-1)..2`
        // Stronger than binary op, since `-x == y` is `(-x) == y`
        // Stronger than a func call, since `exists !y` is `exists (!y)`
        pr::ExprKind::Unary(..) => 20,

        pr::ExprKind::Range(_) => 19,

        pr::ExprKind::Binary(pr::BinaryExpr { op, .. }) => match op {
            pr::BinOp::Pow => 19,
            pr::BinOp::Mul | pr::BinOp::DivInt | pr::BinOp::DivFloat | pr::BinOp::Mod => 18,
            pr::BinOp::Add | pr::BinOp::Sub => 17,
            pr::BinOp::Eq
            | pr::BinOp::Ne
            | pr::BinOp::Gt
            | pr::BinOp::Lt
            | pr::BinOp::Gte
            | pr::BinOp::Lte
            | pr::BinOp::RegexSearch => 16,
            pr::BinOp::Coalesce => 15,
            pr::BinOp::And => 14,
            pr::BinOp::Or => 13,
        },

        // Weaker than a child assign, since `select x = 1`
        // Weaker than a binary operator, since `filter x == 1`
        pr::ExprKind::FuncCall(_) => 10,
        // ExprKind::FuncCall(_) if !is_parent => 2,
        pr::ExprKind::Func(_) => 7,

        // other nodes should not contain any inner exprs
        _ => 100,
    }
}

fn associativity(expr: &pr::ExprKind) -> super::Position {
    match expr {
        pr::ExprKind::Binary(pr::BinaryExpr { op, .. }) => match op {
            pr::BinOp::Pow => super::Position::Right,
            pr::BinOp::Eq
            | pr::BinOp::Ne
            | pr::BinOp::Gt
            | pr::BinOp::Lt
            | pr::BinOp::Gte
            | pr::BinOp::Lte
            | pr::BinOp::RegexSearch => super::Position::Unspecified,
            _ => super::Position::Left,
        },

        _ => super::Position::Unspecified,
    }
}

/// True if this expression could be mistakenly bound with an expression on the left.
fn can_bind_left(expr: &pr::ExprKind) -> bool {
    matches!(
        expr,
        pr::ExprKind::Unary(pr::UnaryExpr {
            op: pr::UnOp::EqSelf | pr::UnOp::Add | pr::UnOp::Neg,
            ..
        })
    )
}

impl WriteSource for pr::Ident {
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

fn keywords() -> &'static HashSet<&'static str> {
    static KEYWORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    KEYWORDS.get_or_init(|| {
        HashSet::from_iter([
            "let", "into", "case", "prql", "type", "module", "internal", "func",
        ])
    })
}

fn valid_prql_ident() -> &'static Regex {
    static VALID_PRQL_IDENT: OnceLock<Regex> = OnceLock::new();
    VALID_PRQL_IDENT.get_or_init(|| {
        // Pomsky expression (regex is to Pomsky what SQL is to PRQL):
        // ^ ('*' | [ascii_alpha '_$'] [ascii_alpha ascii_digit '_$']* ) $
        Regex::new(r"^(?:\*|[a-zA-Z_$][a-zA-Z0-9_$]*)$").unwrap()
    })
}

pub fn write_ident_part(s: &str) -> Cow<str> {
    if valid_prql_ident().is_match(s) && !keywords().contains(s) {
        s.into()
    } else {
        format!("`{}`", s).into()
    }
}

impl WriteSource for Vec<pr::Stmt> {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        opt.reset_line()?;

        let mut r = String::new();
        for stmt in self {
            if !r.is_empty() {
                r += "\n";
            }

            r += &opt.write_indent();
            r += &stmt.write_or_expand(opt.clone());
        }
        Some(r)
    }
}

impl WriteSource for pr::Stmt {
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
            pr::StmtKind::QueryDef(query) => {
                r += "prql";
                if let Some(version) = &query.version {
                    r += &format!(r#" version:"{}""#, version);
                }
                for (key, value) in &query.other {
                    r += &format!(" {key}:{value}");
                }
                r += "\n";
            }
            pr::StmtKind::VarDef(var_def) => match var_def.kind {
                _ if var_def.value.is_none() || var_def.ty.is_some() => {
                    let typ = if let Some(ty) = &var_def.ty {
                        format!("<{}> ", ty.write(opt.clone())?)
                    } else {
                        "".to_string()
                    };

                    r += opt.consume(&format!("let {} {}", var_def.name, typ))?;

                    if let Some(val) = &var_def.value {
                        r += opt.consume("= ")?;
                        r += &val.write(opt)?;
                    }
                    r += "\n";
                }

                pr::VarDefKind::Let => {
                    r += opt.consume(&format!("let {} = ", var_def.name))?;

                    r += &var_def.value.as_ref().unwrap().write(opt)?;
                    r += "\n";
                }
                pr::VarDefKind::Into | pr::VarDefKind::Main => {
                    let val = var_def.value.as_ref().unwrap();
                    match &val.kind {
                        pr::ExprKind::Pipeline(pipeline) => {
                            for expr in &pipeline.exprs {
                                r += &expr.write(opt.clone())?;
                                r += "\n";
                            }
                        }
                        _ => {
                            r += &val.write(opt)?;
                            r += "\n";
                        }
                    }

                    if var_def.kind == pr::VarDefKind::Into {
                        r += &format!("into {}", var_def.name);
                        r += "\n";
                    }
                }
            },
            pr::StmtKind::TypeDef(type_def) => {
                r += opt.consume(&format!("type {}", type_def.name))?;
                r += opt.consume(" = ")?;
                r += &type_def.value.kind.write(opt)?;
                r += "\n";
            }
            pr::StmtKind::ModuleDef(module_def) => {
                r += &format!("module {} {{\n", module_def.name);
                opt.indent += 1;

                r += &module_def.stmts.write(opt.clone())?;

                opt.indent -= 1;
                r += &opt.write_indent();
                r += "}\n";
            }
            pr::StmtKind::ImportDef(import_def) => {
                r += "import ";
                if let Some(alias) = &import_def.alias {
                    r += &write_ident_part(alias);
                    r += " = ";
                }
                r += &import_def.name.write(opt)?;
                r += "\n";
            }
        }
        Some(r)
    }
}

fn display_interpolation(
    prefix: &str,
    parts: &[pr::InterpolateItem],
    opt: WriteOpt,
) -> Option<String> {
    let mut r = String::new();
    r += prefix;
    r += "\"";
    for part in parts {
        match &part {
            // We use double braces to escape braces
            pr::InterpolateItem::String(s) => r += s.replace('{', "{{").replace('}', "}}").as_str(),
            pr::InterpolateItem::Expr { expr, .. } => {
                r += "{";
                r += &expr.write(opt.clone())?;
                r += "}"
            }
        }
    }
    r += "\"";
    Some(r)
}

impl WriteSource for pr::SwitchCase {
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

    use super::*;

    #[track_caller]
    fn assert_is_formatted(input: &str) {
        let formatted = format_single_stmt(input);
        similar_asserts::assert_eq!(input.trim(), formatted.trim());
    }

    fn format_single_stmt(query: &str) -> String {
        use itertools::Itertools;
        let stmt = crate::prql_to_pl(query)
            .unwrap()
            .stmts
            .into_iter()
            .exactly_one()
            .unwrap();
        stmt.write(WriteOpt::default()).unwrap()
    }

    #[test]
    fn test_pipeline() {
        let short = pr::Expr::new(pr::ExprKind::Ident(pr::Ident::from_name(
            "short".to_string(),
        )));
        let long = pr::Expr::new(pr::ExprKind::Ident(pr::Ident::from_name(
            "some_really_long_and_really_long_name".to_string(),
        )));

        let mut opt = WriteOpt {
            indent: 1,
            ..Default::default()
        };

        // short pipelines should be inlined
        let pipeline = pr::Expr::new(pr::ExprKind::Pipeline(pr::Pipeline {
            exprs: vec![short.clone(), short.clone(), short.clone()],
        }));
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @"(short | short | short)");

        // long pipelines should be indented
        let pipeline = pr::Expr::new(pr::ExprKind::Pipeline(pr::Pipeline {
            exprs: vec![short.clone(), long.clone(), long, short.clone()],
        }));
        // colons are a workaround to avoid trimming
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @r"
        (
            short
            some_really_long_and_really_long_name
            some_really_long_and_really_long_name
            short
          )
        ");

        // sometimes, there is just not enough space
        opt.rem_width = 4;
        opt.indent = 100;
        let pipeline = pr::Expr::new(pr::ExprKind::Pipeline(pr::Pipeline { exprs: vec![short] }));
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
        assert_is_formatted(r#"join `project-bar.dataset.table` (==col_bax)"#);
    }

    #[test]
    fn test_binary() {
        assert_is_formatted(r#"let a = 5 * (4 + 3) ?? 5 / 2 // 2 == 1 and true"#);

        assert_is_formatted(r#"let a = 5 / 2 / 2"#);
        assert_is_formatted(r#"let a = 5 / (2 / 2)"#);

        assert_is_formatted(r#"let a = (5 ** 2) ** 2"#);
        assert_is_formatted(r#"let a = 5 ** 2 ** 2"#);
    }

    #[test]
    fn test_func() {
        assert_is_formatted(r#"let a = func x y:false -> x and y"#);
    }

    #[test]
    fn test_simple() {
        assert_is_formatted(
            r#"
aggregate average_country_salary = (
  average salary
)"#,
        );
    }

    #[test]
    fn test_alias() {
        assert_is_formatted(
            r#"
from artists
select {`customer name` = foo, x = bar.baz}"#,
        );
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
let negative = -100..0
"#,
        );

        assert_is_formatted(
            r#"
let negative = -(100..0)
"#,
        );

        assert_is_formatted(
            r#"
let negative = -100..
"#,
        );

        assert_is_formatted(
            r#"
let negative = ..-100
"#,
        );
    }

    #[test]
    fn test_annotation() {
        assert_is_formatted(
            r#"
@deprecated
module hello {
}
"#,
        );
    }

    #[test]
    fn test_var_def() {
        assert_is_formatted(
            r#"
let a
"#,
        );

        assert_is_formatted(
            r#"
let a <int>
"#,
        );

        assert_is_formatted(
            r#"
let a = 5
"#,
        );

        assert_is_formatted(
            r#"
5
into a
"#,
        );
    }

    #[test]
    fn test_query_def() {
        assert_is_formatted(
            r#"
prql version:"^0.9" target:sql.sqlite
"#,
        );
    }
}
