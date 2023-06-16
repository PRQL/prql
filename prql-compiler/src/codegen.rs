use crate::{
    ast::pl::{self, BinaryExpr},
    utils::VALID_IDENT,
};

pub fn write(stmts: &Vec<pl::Stmt>) -> String {
    let mut r = String::new();
    let mut opt = WriteOpt::default();

    'lp: loop {
        for stmt in stmts {
            match stmt.write(opt) {
                Some(s) => {
                    r += &s;
                }
                None => {
                    r.clear();
                    opt.max_width += opt.max_width / 2;
                    continue 'lp;
                }
            }
        }
        return r;
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

    fn write_between<S: ToString>(&self, prefix: S, suffix: &str, opt: WriteOpt) -> Option<String> {
        let mut r = prefix.to_string();
        let opt = opt.consume_width((r.len() + suffix.len()) as u16)?;

        r += &self.write(opt)?;

        r += suffix;
        Some(r)
    }
}

impl<T: WriteSource> WriteSource for &T {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        (*self).write(opt)
    }
}

#[derive(Clone, Copy)]
pub struct WriteOpt {
    /// String to emit as one indentation level
    pub tab: &'static str,

    /// Maximum number of characters per line
    pub max_width: u16,

    /// Current indent used when emitting lines
    pub indent: u16,

    /// Current remaining number of characters in line
    pub rem_width: u16,
}

impl Default for WriteOpt {
    fn default() -> Self {
        Self {
            tab: "  ",
            max_width: 50,

            indent: 0,
            rem_width: 50,
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

    fn consume_width(mut self, width: u16) -> Option<Self> {
        self.rem_width = self.rem_width.checked_sub(width)?;
        Some(self)
    }

    fn reset_line(mut self) -> Option<Self> {
        let ident = self.tab.len() as u16 * self.indent;
        self.rem_width = self.max_width.checked_sub(ident)?;
        Some(self)
    }

    fn write_indent(&self) -> String {
        self.tab.repeat(self.indent as usize)
    }
}

impl WriteSource for pl::Expr {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        self.write_expr(None, opt)
    }
}

impl WriteSource for pl::ExprKind {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        use pl::ExprKind::*;

        match &self {
            Ident(ident) => ident.write(opt),
            All { within, except } => {
                let mut r = String::new();
                r += &within.write(opt)?;
                r += ".!{";
                for e in except {
                    r += &e.write(opt)?;
                    r += ",";
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
                    r += &start.write_expr(Some(self), opt)?;
                }
                r += "..";
                if let Some(end) = &range.end {
                    r += &end.write_expr(Some(self), opt)?;
                }
                Some(r)
            }
            Binary(pl::BinaryExpr { op, left, right }) => {
                let mut r = String::new();

                r += &left.write_expr(Some(self), opt)?;

                r += " ";
                r += &op.to_string();
                r += " ";

                r += &right.write_expr(Some(self), opt)?;
                Some(r)
            }
            Unary(pl::UnaryExpr { op, expr }) => {
                let mut r = String::new();

                r += &op.to_string();
                r += &expr.write_expr(Some(self), opt)?;
                Some(r)
            }
            FuncCall(func_call) => {
                let mut r = String::new();
                r += &func_call.name.write_expr(Some(self), opt)?;

                for (name, arg) in &func_call.named_args {
                    r += " ";
                    r += name;
                    r += ":";
                    r += &arg.write_expr(Some(self), opt)?;
                }
                for arg in &func_call.args {
                    r += " ";
                    r += &arg.write_expr(Some(self), opt)?;
                }
                Some(r)
            }
            Func(c) => {
                let mut r = String::new();
                for param in &c.params {
                    r += &param.name;
                    r += " ";
                }
                for param in &c.named_params {
                    r += &param.name;
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
            RqOperator { .. } => Some("<built-in>".to_string()),
            Type(ty) => ty.write(opt),
            Param(id) => Some(format!("${id}")),
            Internal(operator_name) => Some(format!("internal {operator_name}")),
        }
    }
}

impl pl::Expr {
    /// Writes an optionally parenthesized expression based on the relative binding
    /// strength of the expression and its parent.
    fn write_expr(&self, parent: Option<&pl::ExprKind>, opt: WriteOpt) -> Option<String> {
        if self.alias.is_some() {
            return self.write_alias_expr(parent, opt);
        }
        let strength_self = binding_strength(&self.kind, false);
        let strength_parent = match parent {
            Some(p) => binding_strength(p, true),
            // If there's no parent, then we're at the top level, so we don't need
            // parentheses, we can act like the parent has a low binding strength.
            None => i32::MIN,
        };

        if strength_parent >= strength_self {
            self.kind.write_between("(", ")", opt)
        } else {
            self.kind.write(opt)
        }
    }

    // We split out how aliases are written, since they follow slightly different
    // rules. Possibly there's a simpler way to organize them; contributions
    // welcome.
    fn write_alias_expr(&self, parent: Option<&pl::ExprKind>, opt: WriteOpt) -> Option<String> {
        assert!(self.alias.is_some());
        // When it's a child, the `=` has a high binding strength — we almost never
        // need to wrap `(a = b)` in parens.
        let strength_self = 12;
        let strength_parent = match parent {
            Some(p) => binding_strength(p, true),
            // If there's no parent, then we're at the top level, so we don't need
            // parentheses, we can act like the parent has a low binding strength.
            None => i32::MIN,
        };

        let s = format!(
            "{lhs} = {rhs}",
            lhs = &write_ident_part(self.alias.as_ref().unwrap()),
            rhs = pl::Expr::write_alias_rhs_expr(&self.kind, opt)?
        );
        if strength_parent >= strength_self {
            // TODO: we should use `write_between`, but the function types don't
            // work atm.
            Some(format!("({s})"))
        } else {
            Some(s)
        }
    }

    fn write_alias_rhs_expr(expr: &pl::ExprKind, opt: WriteOpt) -> Option<String> {
        let strength_self = binding_strength(expr, false);
        // When a parent, `=` has a fairly low binding strength:
        // - Weaker than a binop, since `x = y + 1`
        // - Stronger than a child funccall, since `x = (y z)`
        let strength_parent = 0;

        if strength_parent >= strength_self {
            expr.write_between("(", ")", opt)
        } else {
            expr.write(opt)
        }
    }
}

fn binding_strength(expr: &pl::ExprKind, is_parent: bool) -> i32 {
    match expr {
        // For example, if it's an Ident, it's basically infinite — a simple
        // ident never needs parentheses around it.
        pl::ExprKind::Ident(_) => 100,
        pl::ExprKind::All { .. } => 100,

        pl::ExprKind::Range(_) => 6,
        pl::ExprKind::Binary(BinaryExpr { op, .. }) => match op {
            pl::BinOp::Mul | pl::BinOp::DivInt | pl::BinOp::DivFloat | pl::BinOp::Mod => 5,
            pl::BinOp::Add | pl::BinOp::Sub => 4,
            pl::BinOp::Eq
            | pl::BinOp::Ne
            | pl::BinOp::Gt
            | pl::BinOp::Lt
            | pl::BinOp::Gte
            | pl::BinOp::Lte
            | pl::BinOp::RegexSearch => 3,
            pl::BinOp::And => 2,
            pl::BinOp::Or => 1,
            pl::BinOp::Coalesce => 2,
        },
        // Weaker than a parent funccall, since `join x (==y)`
        // Weaker than a range, since `(-100)..1` (alternatively a range could
        // inherit and we could do `(-100..1)`)
        pl::ExprKind::Unary(..) => 1,
        // Weaker than a child assign, since `select x = 1`
        // Weaker than a binary operator, since `filter x == 1`
        pl::ExprKind::FuncCall(_) if is_parent => 2,
        pl::ExprKind::FuncCall(_) if !is_parent => -1,
        pl::ExprKind::Func(_) => 0,

        _ => 11,
    }
}
impl WriteSource for pl::Ident {
    fn write(&self, opt: WriteOpt) -> Option<String> {
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

pub fn write_ident_part(s: &str) -> String {
    if VALID_IDENT.is_match(s) {
        s.to_string()
    } else {
        format!("`{}`", s)
    }
}

impl WriteSource for pl::Stmt {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        match &self.kind {
            pl::StmtKind::QueryDef(query) => {
                let mut r = String::new();
                r += "prql";
                if let Some(version) = &query.version {
                    r += &format!(r#" version:"{}""#, version);
                }
                for (key, value) in &query.other {
                    r += &format!(" {key}:{value}");
                }
                r += "\n";
                Some(r)
            }
            pl::StmtKind::VarDef(var_def) => {
                let mut r = String::new();

                match var_def.kind {
                    pl::VarDefKind::Let => {
                        r += &format!("let {} = ", self.name);
                        r += &var_def.value.write(opt)?;
                        r += "\n";
                    }
                    pl::VarDefKind::Into | pl::VarDefKind::Main => {
                        match &var_def.value.kind {
                            pl::ExprKind::Pipeline(pipeline) => {
                                for expr in &pipeline.exprs {
                                    r += &expr.write(opt)?;
                                    r += "\n";
                                }
                            }
                            _ => {
                                r += &var_def.value.write(opt)?;
                            }
                        }

                        if let pl::VarDefKind::Into = var_def.kind {
                            r += &format!("into {}", self.name);
                            r += "\n";
                        }
                    }
                }
                Some(r)
            }
            pl::StmtKind::TypeDef(_) => todo!(),
            pl::StmtKind::ModuleDef(_) => todo!(),
        }
    }
}

struct SeparatedExprs<'a, T: WriteSource> {
    exprs: &'a [T],
    inline: &'static str,
    line_end: &'static str,
}

impl<'a, T: WriteSource> WriteSource for SeparatedExprs<'a, T> {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        // try inline
        {
            // write each of the exprs, one per line
            let opt_line = opt.reset_line()?;
            let mut exprs = Vec::new();
            for field in self.exprs {
                exprs.push(field.write(opt_line)?);
            }

            if !exprs.iter().any(|e| e.contains('\n')) {
                let inline_width = exprs.iter().map(|s| s.len()).sum::<usize>()
                    + self.inline.len() * (exprs.len().checked_sub(1).unwrap_or_default());
                if opt.rem_width > inline_width as u16 {
                    return Some(exprs.join(self.inline));
                }
            }
        }

        // one per line
        {
            let mut opt = opt;
            opt.indent += 1;

            let mut r = String::new();

            for expr in self.exprs {
                r += "\n";
                r += &opt.write_indent();
                opt = opt.reset_line()?;
                opt.rem_width.checked_sub(self.line_end.len() as u16)?;

                r += &expr.write(opt)?;
                r += self.line_end;
            }
            opt.indent -= 1;
            r += "\n";
            r += &opt.write_indent();

            Some(r)
        }
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
                r += &expr.write(opt)?;
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
        r += &self.condition.write(opt)?;
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
            Singleton(lit) => Some(lit.to_string()),
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
                    r += &t.as_ref().write(opt)?;
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

    fn assert_fmt_matches(input: &str) {
        let stmt = generate_single_stmt(input);

        assert_eq!(input.trim(), stmt.trim());
    }

    // TODO: should we have a library function that can do this?
    fn generate_single_stmt(query: &str) -> String {
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
        let short = pl::Expr::from(pl::ExprKind::Ident(pl::Ident::from_path(vec!["short"])));
        let long = pl::Expr::from(pl::ExprKind::Ident(pl::Ident::from_path(vec![
            "some_module",
            "submodule",
            "a_really_long_name",
        ])));

        let mut opt = WriteOpt {
            indent: 1,
            ..Default::default()
        };

        // short pipelines should be inlined
        let pipeline = pl::Expr::from(pl::ExprKind::Pipeline(pl::Pipeline {
            exprs: vec![short.clone(), short.clone(), short.clone()],
        }));
        assert_snapshot!(pipeline.write(opt).unwrap(), @"(short | short | short)");

        // long pipelines should be indented
        let pipeline = pl::Expr::from(pl::ExprKind::Pipeline(pl::Pipeline {
            exprs: vec![short.clone(), long.clone(), long, short.clone()],
        }));
        // colons are a workaround to avoid trimming
        assert_snapshot!(pipeline.write(opt).unwrap(), @r###"
        (
            short
            some_module.submodule.a_really_long_name
            some_module.submodule.a_really_long_name
            short
          )
        "###);

        // sometimes, there is just not enough space
        opt.rem_width = 10;
        opt.indent = 100;
        let pipeline = pl::Expr::from(pl::ExprKind::Pipeline(pl::Pipeline { exprs: vec![short] }));
        assert!(pipeline.write(opt).is_none());
    }

    #[test]
    fn test_escaped_string() {
        assert_fmt_matches(r#"filter name ~= "\\(I Can't Help\\) Falling""#);
    }

    #[test]
    fn test_double_braces() {
        assert_fmt_matches(
            r#"let has_valid_title = s"regexp_contains(title, '([a-z0-9]*-){{2,}}')""#,
        );
    }

    #[test]
    fn test_unary() {
        assert_fmt_matches(r#"sort {-duration}"#);

        assert_fmt_matches(r#"select a = -b"#);
        assert_fmt_matches(r#"join `project-bar.dataset.table` (==col_bax)"#)
    }

    #[test]
    fn test_simple() {
        assert_fmt_matches(r#"aggregate average_country_salary = (average salary)"#);
    }

    #[test]
    fn test_assign() {
        assert_fmt_matches(
            r#"
group {title, country} (aggregate {
  average salary,
  average gross_salary,
  sum salary,
  sum gross_salary,
  average gross_cost,
  sum_gross_cost = (sum gross_cost),
  ct = (count s"*"),
})"#,
        );
    }
    #[test]
    fn test_range() {
        assert_fmt_matches(
            r#"
from foo
is_negative = (-100)..0
"#,
        );
    }
}
