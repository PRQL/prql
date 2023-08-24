use serde::de::Visitor;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug, Formatter};
use std::ops::Range;

#[derive(Clone, PartialEq, Eq, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,

    pub source_id: u16,
}

impl From<Span> for Range<usize> {
    fn from(a: Span) -> Self {
        a.start..a.end
    }
}

impl Debug for Span {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}-{}", self.source_id, self.start, self.end)
    }
}

impl Serialize for Span {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let str = format!("{self:?}");
        serializer.serialize_str(&str)
    }
}

impl<'de> Deserialize<'de> for Span {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SpanVisitor {}

        impl<'de> Visitor<'de> for SpanVisitor {
            type Value = Span;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "A span string of form `file_id:x-y`")
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                use serde::de;

                if let Some((file_id, char_span)) = v.split_once(':') {
                    let file_id = file_id
                        .parse::<u16>()
                        .map_err(|e| de::Error::custom(e.to_string()))?;

                    if let Some((start, end)) = char_span.split_once('-') {
                        let start = start
                            .parse::<usize>()
                            .map_err(|e| de::Error::custom(e.to_string()))?;
                        let end = end
                            .parse::<usize>()
                            .map_err(|e| de::Error::custom(e.to_string()))?;

                        return Ok(Span {
                            start,
                            end,
                            source_id: file_id,
                        });
                    }
                }

                Err(de::Error::custom("malformed span"))
            }

            fn visit_string<E>(self, v: String) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&v)
            }
        }

        deserializer.deserialize_string(SpanVisitor {})
    }
}
