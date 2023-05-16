use crate::ast::pl;

pub fn write(stmts: &Vec<pl::Stmt>) -> String {
    let mut r = String::new();
    let opt = WriteOpt::default();

    for stmt in stmts {
        // TODO: uncomment this
        // r += &stmt.kind.write(opt.clone()).unwrap();
        r += "\n\n";
    }
    r
}

pub trait WriteSource {
    /// Converts self to it's source representation according to specified
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
}

impl Default for WriteOpt {
    fn default() -> Self {
        Self {
            tab: "    ",
            max_width: 80,

            indent: 0,
            rem_width: 80,
        }
    }
}

impl WriteOpt {
    fn consume_width(mut self, width: u16) -> Option<Self> {
        self.rem_width = self.rem_width.checked_sub(width)?;
        Some(self)
    }

    fn reset_line(mut self) -> Option<Self> {
        let ident = self.tab.len() as u16 * self.indent;
        self.rem_width = self.max_width.checked_sub(ident)?;
        Some(self)
    }

    fn write_ident(&self) -> String {
        self.tab.repeat(self.indent as usize)
    }
}

impl WriteSource for pl::Expr {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        let mut r = String::new();
        if let Some(alias) = &self.alias {
            r += &write_ident_part(alias);
            r += " = ";
        }

        self.kind.write_between(r, "", opt)
    }
}

impl WriteSource for pl::ExprKind {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        use pl::ExprKind::*;

        match &self {
            Ident(ident) => ident.write(opt),
            Pipeline(pipeline) => pipeline.write_between("(", ")", opt),
            _ => Some("<todo>".to_string()),
        }
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
        format!("`{s}`")
    } else {
        format!("{s}")
    }
}

impl WriteSource for pl::Pipeline {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        if self.exprs.is_empty() {
            return Some(String::new());
        }

        // write each of the exprs, one per line
        let opt_line = opt.clone().reset_line()?;
        let mut exprs = Vec::new();
        for expr in &self.exprs {
            exprs.push(expr.write(opt_line.clone())?);
        }

        // try inline
        if exprs.iter().all(|e| !e.contains('\n')) {
            let inline_width = exprs.iter().map(|s| s.len()).sum::<usize>() + 3 * (exprs.len() - 1);
            if opt.rem_width > inline_width as u16 {
                return Some(exprs.join(" | "));
            }
        }

        // one per line
        let mut opt = opt;
        opt.indent += 1;

        let mut r = String::new();
        r += "\n";
        for expr in exprs {
            r += &opt.write_ident();
            r += &expr;
            r += "\n";
        }
        opt.indent -= 1;
        r += &opt.write_ident();

        Some(r)
    }
}

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use super::*;

    #[test]
    fn test_pipeline() {
        let short = pl::Expr::from(pl::ExprKind::Ident(pl::Ident::from_path(vec!["short"])));
        let long = pl::Expr::from(pl::ExprKind::Ident(pl::Ident::from_path(vec![
            "some_module",
            "submodule",
            "a_really_long_name",
        ])));

        let mut opt = WriteOpt::default();
        opt.indent = 1;

        // short pipelines should be inlined
        let pipeline = pl::Pipeline {
            exprs: vec![short.clone(), short.clone(), short.clone()],
        };
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @"short | short | short");

        // long pipelines should be indented
        let pipeline = pl::Pipeline {
            exprs: vec![short.clone(), long.clone(), long.clone(), short.clone()],
        };
        // colons are a workaround to avoid trimming
        assert_snapshot!(":".to_string() + &pipeline.write(opt.clone()).unwrap() + ":", @r###"
        :
                short
                some_module.submodule.a_really_long_name
                some_module.submodule.a_really_long_name
                short
            :
        "###);

        // sometimes, there is just not enough space
        opt.rem_width = 10;
        opt.indent = 100;
        let pipeline = pl::Pipeline {
            exprs: vec![short.clone()],
        };
        assert!(pipeline.write(opt).is_none());
    }
}
