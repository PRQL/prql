use std::fmt::{self, Debug, Formatter};
use std::ops::{Add, Range, Sub};

use chumsky::Stream;
use schemars::JsonSchema;
use serde::de::Visitor;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Copy, JsonSchema)]
pub struct Span {
    pub start: usize,
    pub end: usize,

    /// A key representing the path of the source. Value is stored in prqlc's SourceTree::source_ids.
    pub source_id: u16,
}

impl Span {
    pub fn merge_opt(a: Option<Span>, b: Option<Span>) -> Option<Span> {
        match (a, b) {
            (None, None) => None,
            (None, Some(s)) => Some(s),
            (Some(s), None) => Some(s),
            (Some(a), Some(b)) => Some(Span::merge(a, b)),
        }
    }

    pub fn merge(a: Span, b: Span) -> Span {
        assert_eq!(a.source_id, b.source_id);
        Span {
            start: usize::min(a.start, b.start),
            end: usize::max(a.end, b.end),
            source_id: a.source_id,
        }
    }
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

impl PartialOrd for Span {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // We could expand this to compare source_id too, starting with minimum surprise
        match other.source_id.partial_cmp(&self.source_id) {
            Some(std::cmp::Ordering::Equal) => {
                debug_assert!((self.start <= other.start) == (self.end <= other.end));
                self.start.partial_cmp(&other.start)
            }
            _ => None,
        }
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

impl chumsky::Span for Span {
    type Context = u16;

    type Offset = usize;

    fn new(context: Self::Context, range: std::ops::Range<Self::Offset>) -> Self {
        Self {
            start: range.start,
            end: range.end,
            source_id: context,
        }
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

impl Add<usize> for Span {
    type Output = Span;

    fn add(self, rhs: usize) -> Span {
        Self {
            start: self.start + rhs,
            end: self.end + rhs,
            source_id: self.source_id,
        }
    }
}

impl Sub<usize> for Span {
    type Output = Span;

    fn sub(self, rhs: usize) -> Span {
        Self {
            start: self.start - rhs,
            end: self.end - rhs,
            source_id: self.source_id,
        }
    }
}

pub fn string_stream<'a>(
    s: String,
    span_base: Span,
) -> Stream<'a, char, Span, Box<dyn Iterator<Item = (char, Span)>>> {
    let chars = s.chars().collect::<Vec<_>>();

    Stream::from_iter(
        Span {
            start: span_base.start + chars.len(),
            end: span_base.start + chars.len(),
            source_id: span_base.source_id,
        },
        Box::new(chars.into_iter().enumerate().map(move |(i, c)| {
            (
                c,
                Span {
                    start: span_base.start + i,
                    end: span_base.start + i + 1,
                    source_id: span_base.source_id,
                },
            )
        })),
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_span_serde() {
        let span = Span {
            start: 12,
            end: 15,
            source_id: 45,
        };
        let span_serialized = serde_json::to_string(&span).unwrap();
        insta::assert_snapshot!(span_serialized, @r###""45:12-15""###);
        let span_deserialized: Span = serde_json::from_str(&span_serialized).unwrap();
        assert_eq!(span_deserialized, span);
    }

    #[test]
    fn test_span_partial_cmp() {
        let span1 = Span {
            start: 10,
            end: 20,
            source_id: 1,
        };
        let span2 = Span {
            start: 15,
            end: 25,
            source_id: 1,
        };
        let span3 = Span {
            start: 5,
            end: 15,
            source_id: 2,
        };

        // span1 and span2 have the same source_id, so their start values are compared
        assert_eq!(span1.partial_cmp(&span2), Some(std::cmp::Ordering::Less));
        assert_eq!(span2.partial_cmp(&span1), Some(std::cmp::Ordering::Greater));

        // span1 and span3 have different source_id, so their source_id values are compared
        assert_eq!(span1.partial_cmp(&span3), None);
        assert_eq!(span3.partial_cmp(&span1), None);
    }
}
