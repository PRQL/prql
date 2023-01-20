use anyhow::anyhow;
use std::fmt::Display;

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum Literal {
    Null,
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Date(String),
    Time(String),
    Timestamp(String),
    ValueAndUnit(ValueAndUnit),
    Relation(RelationLiteral),
}

// Compound units, such as "2 days 3 hours" can be represented as `2days + 3hours`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValueAndUnit {
    pub n: i64,       // Do any DBs use floats or decimals for this?
    pub unit: String, // Could be an enum IntervalType,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct RelationLiteral {
    /// Column names
    pub columns: Vec<String>,
    /// Row-oriented data
    pub rows: Vec<Vec<Literal>>,
}

impl From<Literal> for anyhow::Error {
    fn from(item: Literal) -> Self {
        anyhow!("Failed to convert `{item}`")
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Null => write!(f, "null")?,
            Literal::Integer(i) => write!(f, "{i}")?,
            Literal::Float(i) => write!(f, "{i}")?,

            Literal::String(s) => {
                match s.find('"') {
                    Some(_) => {
                        match s.find('\'') {
                            Some(_) => {
                                let mut min_quote = 3;
                                // find minimum number of double quotes when string contains
                                // both single and double quotes
                                loop {
                                    let stop = s
                                        .find(&"\"".repeat(min_quote))
                                        .map(|_| s.contains(&"\"".repeat(min_quote + 1)))
                                        .unwrap_or(true);
                                    if stop {
                                        break;
                                    } else {
                                        min_quote += 1;
                                    }
                                }
                                let quotes = "\"".repeat(min_quote);
                                write!(f, "{quotes}{s}{quotes}")?;
                            }

                            None => write!(f, "'{s}'")?,
                        };
                    }

                    None => write!(f, "\"{s}\"")?,
                };
            }

            Literal::Boolean(b) => {
                f.write_str(if *b { "true" } else { "false" })?;
            }

            Literal::Date(inner) | Literal::Time(inner) | Literal::Timestamp(inner) => {
                write!(f, "@{inner}")?;
            }

            Literal::ValueAndUnit(i) => {
                write!(f, "{}{}", i.n, i.unit)?;
            }

            Literal::Relation(_) => {
                write!(f, "<unimplemented relation>")?;
            }
        }
        Ok(())
    }
}
