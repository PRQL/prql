pub(crate) use ast::write_expr;
pub(crate) use types::{write_ty, write_ty_kind};

mod ast;
mod types;

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

    fn write_or_expand(&self, mut opt: WriteOpt) -> String {
        loop {
            if let Some(s) = self.write(opt.clone()) {
                return s;
            } else {
                opt.max_width += opt.max_width / 2;
                opt.reset_line();
            }
        }
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

    /// Position within binary operators.
    /// Needed for omitting parenthesis in following expressions: `(a + b) + c`.
    pub binary_position: Position,

    /// True iff preceding source ends in an expression that could
    /// be mistakenly bound into a binary op by appending an unary op.
    ///
    /// For example:
    /// `join foo` has an unbound expr, since `join foo ==bar` produced a binary op.
    pub unbound_expr: bool,
}

#[derive(Clone, PartialEq)]
pub enum Position {
    Unspecified,
    Left,
    Right,
}

impl Default for WriteOpt {
    fn default() -> Self {
        Self {
            tab: "  ",
            max_width: 50,

            indent: 0,
            rem_width: 50,
            context_strength: 0,
            binary_position: Position::Unspecified,
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

    /// Subtracts the width of the source from the remaining width and returns the source unchanged.
    fn consume<S: AsRef<str>>(&mut self, source: S) -> Option<S> {
        let width = if let Some(new_line) = source.as_ref().rfind('\n') {
            source.as_ref().len() - new_line
        } else {
            source.as_ref().len()
        };
        self.consume_width(width as u16)?;
        Some(source)
    }

    fn write_indent(&self) -> String {
        self.tab.repeat(self.indent as usize)
    }
}

/// Holds a list of (generally) expressions, attempting to write them in a
/// single line, or falling back to one-per-line
#[derive(Debug, Clone)]
struct SeparatedExprs<'a, T: WriteSource> {
    exprs: &'a [T],
    /// The separator to use when writing the expressions on a single line; for
    /// example `", "`.
    inline: &'static str,
    /// The separator to use when writing the expressions on separate lines, for
    /// example `","` (`/n` is implied)
    line_end: &'static str,
}

impl<T: WriteSource> WriteSource for SeparatedExprs<'_, T> {
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

impl<T: WriteSource> SeparatedExprs<'_, T> {
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
