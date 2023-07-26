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
