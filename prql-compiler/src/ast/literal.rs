use anyhow::anyhow;
use std::fmt::Display;

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum Literal {
    Null,
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Date(String),
    Time(String),
    Timestamp(String),
}

impl From<Literal> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(item: Literal) -> Self {
        anyhow!("Failed to convert `{item}`")
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Null => {}
            Literal::Integer(i) => write!(f, "{i}")?,
            Literal::Float(i) => write!(f, "{i}")?,

            Literal::String(s) => {
                write!(f, "\"{s}\"")?;
            }

            Literal::Boolean(b) => {
                f.write_str(if *b { "true" } else { "false" })?;
            }

            Literal::Date(inner) | Literal::Time(inner) | Literal::Timestamp(inner) => {
                write!(f, "@{inner}")?;
            }
        }
        Ok(())
    }
}
