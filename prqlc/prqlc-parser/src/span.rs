use std::ops::{Add, Deref, DerefMut, Sub};

use serde::{Deserialize, Serialize};

use crate::Span;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ParserSpan(pub crate::Span);

impl Deref for ParserSpan {
    type Target = Span;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ParserSpan {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Add<usize> for ParserSpan {
    type Output = ParserSpan;

    fn add(self, rhs: usize) -> ParserSpan {
        Self(Span {
            start: self.start + rhs,
            end: self.end + rhs,
            source_id: self.source_id,
        })
    }
}

impl Sub<usize> for ParserSpan {
    type Output = ParserSpan;

    fn sub(self, rhs: usize) -> ParserSpan {
        Self(Span {
            start: self.start - rhs,
            end: self.end - rhs,
            source_id: self.source_id,
        })
    }
}

impl chumsky::Span for ParserSpan {
    type Context = u16;

    type Offset = usize;

    fn new(context: Self::Context, range: std::ops::Range<Self::Offset>) -> Self {
        Self(Span {
            start: range.start,
            end: range.end,
            source_id: context,
        })
    }

    fn context(&self) -> Self::Context {
        self.source_id
    }

    fn start(&self) -> Self::Offset {
        self.start
    }

    fn end(&self) -> Self::Offset {
        self.end
    }
}
