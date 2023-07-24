use std::collections::HashSet;

use once_cell::sync::Lazy;

use crate::{
    ir::pl::{self, BinaryExpr},
    utils::VALID_IDENT,
};

mod literal;
pub use literal::DisplayLiteral;

pub fn write(stmts: &Vec<pl::Stmt>) -> String {
    let mut opt = WriteOpt::default();

    loop {
        if let Some(s) = stmts.write(opt.clone()) {
            break s;
        } else {
            opt.max_width += opt.max_width / 2;
        }
    }
}

impl std::fmt::Display for pl::Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let opt = WriteOpt::new_width(u16::MAX);
        f.write_str(&self.write(opt).unwrap())
    }
}

impl std::fmt::Display for pl::Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let opt = WriteOpt::new_width(u16::MAX);
        f.write_str(&self.write(opt).unwrap())
    }
}

impl std::fmt::Display for pl::TyKind {
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

    fn write_within(&self, parent: &pl::ExprKind, mut opt: WriteOpt) -> Option<String> {
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
    fn new_width(max_width: u16) -> Self {
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

impl WriteSource for pl::Expr {
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

impl WriteSource for pl::ExprKind {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        use pl::ExprKind::*;

        match &self {
            Ident(ident) => ident.write(opt),
            All { within, except } => {
                let mut r = String::new();
                r += opt.consume(&within.write(opt.clone())?)?;

                r += ".!{";
                for e in except {
                    r += opt.consume(&e.write(opt.clone())?)?;
                    r += opt.consume(",")?;
                }
                r += "}";
                Some(r)
            }
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
            Binary(pl::BinaryExpr { op, left, right }) => {
                let mut r = String::new();

                let left = left.write_within(self, opt.clone())?;
                r += opt.consume(&left)?;

                r += opt.consume(" ")?;
                r += opt.consume(&op.to_string())?;
                r += opt.consume(" ")?;

                r += &right.write_within(self, opt)?;
                Some(r)
            }
            Unary(pl::UnaryExpr { op, expr }) => {
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

                if !c.args.is_empty() {
                    r = format!("({r})");
                    for args in &c.args {
                        r += " ";
                        r += &args.to_string();
                    }
                    r = format!("({r})");
                }
                Some(r)
            }
            SString(parts) => display_interpolation("s", parts, opt),
            FString(parts) => display_interpolation("f", parts, opt),
            TransformCall(transform) => {
                Some(format!("{} <unimplemented>", (*transform.kind).as_ref()))
            }
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
            RqOperator { .. } => Some("<built-in>".to_string()),
            Type(ty) => ty.write(opt),
            Param(id) => Some(format!("${id}")),
            Internal(operator_name) => Some(format!("internal {operator_name}")),
        }
    }
}

fn binding_strength(expr: &pl::ExprKind) -> u8 {
    match expr {
        // For example, if it's an Ident, it's basically infinite — a simple
        // ident never needs parentheses around it.
        pl::ExprKind::Ident(_) => 100,
        pl::ExprKind::All { .. } => 100,

        // Stronger than a range, since `-1..2` is `(-1)..2`
        // Stronger than binary op, since `-x == y` is `(-x) == y`
        // Stronger than a func call, since `exists !y` is `exists (!y)`
        pl::ExprKind::Unary(..) => 20,

        pl::ExprKind::Range(_) => 19,

        pl::ExprKind::Binary(BinaryExpr { op, .. }) => match op {
            pl::BinOp::Mul | pl::BinOp::DivInt | pl::BinOp::DivFloat | pl::BinOp::Mod => 18,
            pl::BinOp::Add | pl::BinOp::Sub => 17,
            pl::BinOp::Eq
            | pl::BinOp::Ne
            | pl::BinOp::Gt
            | pl::BinOp::Lt
            | pl::BinOp::Gte
            | pl::BinOp::Lte
            | pl::BinOp::RegexSearch => 16,
            pl::BinOp::Coalesce => 15,
            pl::BinOp::And => 14,
            pl::BinOp::Or => 13,
        },

        // Weaker than a child assign, since `select x = 1`
        // Weaker than a binary operator, since `filter x == 1`
        pl::ExprKind::FuncCall(_) => 10,
        // pl::ExprKind::FuncCall(_) if !is_parent => 2,
        pl::ExprKind::Func(_) => 7,

        // other nodes should not contain any inner exprs
        _ => 100,
    }
}

/// True if this expression could be mistakenly bound with an expression on the left.
fn can_bind_left(expr: &pl::ExprKind) -> bool {
    matches!(
        expr,
        pl::ExprKind::Unary(pl::UnaryExpr {
            op: pl::UnOp::EqSelf | pl::UnOp::Add | pl::UnOp::Neg,
            ..
        })
    )
}

impl WriteSource for pl::Ident {
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

impl WriteSource for Vec<pl::Stmt> {
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

impl WriteSource for pl::Stmt {
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
            pl::StmtKind::QueryDef(query) => {
                r += "prql";
                if let Some(version) = &query.version {
                    r += &format!(r#" version:"{}""#, version);
                }
                for (key, value) in &query.other {
                    r += &format!(" {key}:{value}");
                }
                r += "\n";
            }
            pl::StmtKind::VarDef(var_def) => match var_def.kind {
                pl::VarDefKind::Let => {
                    r += opt.consume(&format!("let {} = ", self.name()))?;

                    r += &var_def.value.write(opt)?;
                    r += "\n";
                }
                pl::VarDefKind::Into | pl::VarDefKind::Main => {
                    match &var_def.value.kind {
                        pl::ExprKind::Pipeline(pipeline) => {
                            for expr in &pipeline.exprs {
                                r += &expr.write(opt.clone())?;
                                r += "\n";
                            }
                        }
                        _ => {
                            r += &var_def.value.write(opt)?;
                        }
                    }

                    if let pl::VarDefKind::Into = var_def.kind {
                        r += &format!("into {}", self.name());
                        r += "\n";
                    }
                }
            },
            pl::StmtKind::TypeDef(type_def) => {
                r += opt.consume(&format!("let {}", self.name()))?;

                if let Some(value) = &type_def.value {
                    r += opt.consume(" = ")?;
                    r += &value.write(opt)?;
                }
                r += "\n";
            }
            pl::StmtKind::ModuleDef(module_def) => {
                r += &format!("module {} {{\n", self.name());
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

struct SeparatedExprs<'a, T: WriteSource> {
    exprs: &'a [T],
    inline: &'static str,
    line_end: &'static str,
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

fn display_interpolation(
    prefix: &str,
    parts: &[pl::InterpolateItem],
    opt: WriteOpt,
) -> Option<String> {
    let mut r = String::new();
    r += prefix;
    r += "\"";
    for part in parts {
        match &part {
            // We use double braces to escape braces
            pl::InterpolateItem::String(s) => r += s.replace('{', "{{").replace('}', "}}").as_str(),
            pl::InterpolateItem::Expr { expr, .. } => {
                r += "{";
                r += &expr.write(opt.clone())?;
                r += "}"
            }
        }
    }
    r += "\"";
    Some(r)
}

impl WriteSource for pl::SwitchCase {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        let mut r = String::new();
        r += &self.condition.write(opt.clone())?;
        r += " => ";
        r += &self.value.write(opt)?;
        Some(r)
    }
}

impl WriteSource for pl::Ty {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        if let Some(name) = &self.name {
            Some(name.clone())
        } else {
            self.kind.write(opt)
        }
    }
}

impl WriteSource for Option<&pl::Ty> {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        match self {
            Some(ty) => ty.write(opt),
            None => Some("infer".to_string()),
        }
    }
}

impl WriteSource for pl::TyKind {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        use pl::TyKind::*;

        match &self {
            Primitive(prim) => Some(prim.to_string()),
            Union(variants) => {
                let variants: Vec<_> = variants.iter().map(|x| &x.1).collect();

                SeparatedExprs {
                    exprs: &variants,
                    inline: " || ",
                    line_end: " ||",
                }
                .write(opt)
            }
            Singleton(lit) => Some(DisplayLiteral(lit).to_string()),
            Tuple(elements) => SeparatedExprs {
                exprs: elements,
                inline: ", ",
                line_end: ",",
            }
            .write_between("{", "}", opt),
            Set => Some("set".to_string()),
            Array(elem) => Some(format!("[{}]", elem.write(opt)?)),
            Function(func) => {
                let mut r = String::new();

                for t in &func.args {
                    r += &t.as_ref().write(opt.clone())?;
                    r += " ";
                }
                r += "-> ";
                r += &(*func.return_ty).as_ref().write(opt)?;
                Some(r)
            }
        }
    }
}

impl WriteSource for pl::TupleField {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        match self {
            Self::Wildcard(generic_el) => match generic_el {
                Some(el) => Some(format!("{}..", el.write(opt)?)),
                None => Some("*..".to_string()),
            },
            Self::Single(name, expr) => {
                let mut r = String::new();

                if let Some(name) = name {
                    r += name;
                    r += " = ";
                }
                if let Some(expr) = expr {
                    r += &expr.write(opt)?;
                } else {
                    r += "?";
                }
                Some(r)
            }
        }
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
        let short = pl::Expr::new(pl::ExprKind::Ident(pl::Ident::from_path(vec!["short"])));
        let long = pl::Expr::new(pl::ExprKind::Ident(pl::Ident::from_path(vec![
            "some_module",
            "submodule",
            "a_really_long_name",
        ])));

        let mut opt = WriteOpt {
            indent: 1,
            ..Default::default()
        };

        // short pipelines should be inlined
        let pipeline = pl::Expr::new(pl::ExprKind::Pipeline(pl::Pipeline {
            exprs: vec![short.clone(), short.clone(), short.clone()],
        }));
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @"(short | short | short)");

        // long pipelines should be indented
        let pipeline = pl::Expr::new(pl::ExprKind::Pipeline(pl::Pipeline {
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
        let pipeline = pl::Expr::new(pl::ExprKind::Pipeline(pl::Pipeline { exprs: vec![short] }));
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
