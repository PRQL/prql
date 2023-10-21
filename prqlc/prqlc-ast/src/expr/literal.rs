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
}

// Compound units, such as "2 days 3 hours" can be represented as `2days + 3hours`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValueAndUnit {
    pub n: i64,       // Do any DBs use floats or decimals for this?
    pub unit: String, // Could be an enum IntervalType,
}

impl std::fmt::Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Null => write!(f, "null")?,
            Literal::Integer(i) => write!(f, "{i}")?,
            Literal::Float(i) => write!(f, "{i}")?,

            Literal::String(s) => {
                quote_string(s, f)?;
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
        }
        Ok(())
    }
}

fn quote_string(s: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let s = escape_all_except_quotes(s);

    if !s.contains('"') {
        return write!(f, r#""{s}""#);
    }

    if !s.contains('\'') {
        return write!(f, "'{s}'");
    }

    // when string contains both single and double quotes
    // find minimum number of double quotes
    let mut quotes = "\"\"".to_string();
    while s.contains(&quotes) {
        quotes += "\"";
    }
    write!(f, "{quotes}{s}{quotes}")
}

fn escape_all_except_quotes(s: &str) -> String {
    let mut result = String::new();
    for ch in s.chars() {
        if ch == '"' || ch == '\'' {
            result.push(ch);
        } else {
            result.extend(ch.escape_default());
        }
    }
    result
}
