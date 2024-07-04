// Generic definitions of various AST items.
//
// This was added in a big refactor by a generous-but-new contributor, and
// hasn't been used much since, and I'm not sure carries its weight. So we
// could consider rolling back to only concrete implementations to delayer the
// code.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Inclusive-inclusive range.
/// Missing bound means unbounded range.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct Range<T> {
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

    pub fn try_map<U, E, F: Fn(T) -> Result<U, E>>(self, f: F) -> Result<Range<U>, E> {
        Ok(Range {
            start: self.start.map(&f).transpose()?,
            end: self.end.map(f).transpose()?,
        })
    }

    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> Range<U> {
        Range {
            start: self.start.map(&f),
            end: self.end.map(f),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, JsonSchema)]
pub enum InterpolateItem<T> {
    String(String),
    Expr {
        expr: Box<T>,
        format: Option<String>,
    },
}

impl<T> InterpolateItem<T> {
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> InterpolateItem<U> {
        match self {
            Self::String(s) => InterpolateItem::String(s),
            Self::Expr { expr, format } => InterpolateItem::Expr {
                expr: Box::new(f(*expr)),
                format,
            },
        }
    }

    pub fn try_map<U, E, F: Fn(T) -> Result<U, E>>(self, f: F) -> Result<InterpolateItem<U>, E> {
        Ok(match self {
            Self::String(s) => InterpolateItem::String(s),
            Self::Expr { expr, format } => InterpolateItem::Expr {
                expr: Box::new(f(*expr)?),
                format,
            },
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SwitchCase<T> {
    pub condition: T,
    pub value: T,
}
