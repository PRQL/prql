use serde::{Deserialize, Serialize};

use enum_as_inner::EnumAsInner;
use schemars::JsonSchema;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Tokens(pub Vec<Token>);

#[derive(Clone, PartialEq, Serialize, Deserialize, Eq, JsonSchema)]
pub struct Token {
    pub kind: TokenKind,
    pub span: std::ops::Range<usize>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, JsonSchema)]
pub enum TokenKind {
    NewLine,

    Ident(String),
    Keyword(String),
    #[cfg_attr(
        feature = "serde_yaml",
        serde(with = "serde_yaml::with::singleton_map"),
        schemars(with = "Literal")
    )]
    Literal(Literal),
    /// A parameter such as `$1`
    Param(String),

    Range {
        /// Whether the left side of the range is bound by the previous token
        /// (but it's not contained in this token)
        bind_left: bool,
        bind_right: bool,
    },
    Interpolation(char, String),

    /// single-char control tokens
    Control(char),

    ArrowThin,   // ->
    ArrowFat,    // =>
    Eq,          // ==
    Ne,          // !=
    Gte,         // >=
    Lte,         // <=
    RegexSearch, // ~=
    And,         // &&
    Or,          // ||
    Coalesce,    // ??
    DivInt,      // //
    Pow,         // **
    Annotate,    // @

    // Aesthetics only
    Comment(String),
    DocComment(String),
    /// Vec containing comments between the newline and the line wrap
    // Currently we include the comments with the LineWrap token. This isn't
    // ideal, but I'm not sure of an easy way of having them be separate.
    // - The line wrap span technically includes the comments — on a newline,
    //   we need to look ahead to _after_ the comments to see if there's a
    //   line wrap, and exclude the newline if there is.
    // - We can only pass one token back
    //
    // Alternatives:
    // - Post-process the stream, removing the newline prior to a line wrap.
    //   But requires a whole extra pass.
    // - Change the functionality. But it's very nice to be able to comment
    //   something out and have line-wraps still work.
    LineWrap(Vec<TokenKind>),

    /// A token we manually insert at the start of the input, which later stages
    /// can treat as a newline.
    Start,
}

#[derive(
    Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, JsonSchema,
)]
pub enum Literal {
    Null,
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    RawString(String),
    Date(String),
    Time(String),
    Timestamp(String),
    ValueAndUnit(ValueAndUnit),
}

impl TokenKind {
    pub fn range(bind_left: bool, bind_right: bool) -> Self {
        TokenKind::Range {
            bind_left,
            bind_right,
        }
    }
}
// Compound units, such as "2 days 3 hours" can be represented as `2days + 3hours`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
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
                write!(f, "{}", quote_string(escape_all_except_quotes(s).as_str()))?;
            }

            Literal::RawString(s) => {
                write!(f, "r{}", quote_string(s))?;
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

fn quote_string(s: &str) -> String {
    if !s.contains('"') {
        return format!(r#""{}""#, s);
    }

    if !s.contains('\'') {
        return format!("'{}'", s);
    }

    // If the string starts or ends with a quote, use the other quote to delimit
    // the string. Otherwise default to double quotes.

    // TODO: this doesn't cover a string that starts with a single quote and
    // ends with a double quote; I think in that case it's necessary to escape
    // the quote. We need to add tests here.

    let quote = if s.starts_with('"') || s.ends_with('"') {
        '\''
    } else {
        '"'
    };

    // When string contains both single and double quotes find the longest
    // sequence of consecutive quotes, and then use the next highest odd number
    // of quotes (quotes must be odd; even number of quotes are empty strings).
    // i.e.:
    // 0 -> 1
    // 1 -> 3
    // 2 -> 3
    // 3 -> 5
    let max_consecutive = s
        .split(|c| c != quote)
        .map(|quote_sequence| quote_sequence.len())
        .max()
        .unwrap_or(0);
    let next_odd = (max_consecutive + 1) / 2 * 2 + 1;
    let delim = quote.to_string().repeat(next_odd);

    format!("{}{}{}", delim, s, delim)
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

// This is here because Literal::Float(f64) does not implement Hash, so we cannot simply derive it.
// There are reasons for that, but chumsky::Error needs Hash for the TokenKind, so it can deduplicate
// tokens in error.
// So this hack could lead to duplicated tokens in error messages. Oh no.
#[allow(clippy::derived_hash_with_manual_eq)]
impl std::hash::Hash for TokenKind {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl std::cmp::Eq for TokenKind {}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::NewLine => write!(f, "new line"),
            TokenKind::Ident(s) => {
                if s.is_empty() {
                    // FYI this shows up in errors
                    write!(f, "an identifier")
                } else {
                    write!(f, "{s}")
                }
            }
            TokenKind::Keyword(s) => write!(f, "keyword {s}"),
            TokenKind::Literal(lit) => write!(f, "{}", lit),
            TokenKind::Control(c) => write!(f, "{c}"),

            TokenKind::ArrowThin => f.write_str("->"),
            TokenKind::ArrowFat => f.write_str("=>"),
            TokenKind::Eq => f.write_str("=="),
            TokenKind::Ne => f.write_str("!="),
            TokenKind::Gte => f.write_str(">="),
            TokenKind::Lte => f.write_str("<="),
            TokenKind::RegexSearch => f.write_str("~="),
            TokenKind::And => f.write_str("&&"),
            TokenKind::Or => f.write_str("||"),
            TokenKind::Coalesce => f.write_str("??"),
            TokenKind::DivInt => f.write_str("//"),
            TokenKind::Pow => f.write_str("**"),
            TokenKind::Annotate => f.write_str("@{"),

            TokenKind::Param(id) => write!(f, "${id}"),

            TokenKind::Range {
                bind_left,
                bind_right,
            } => write!(
                f,
                "'{}..{}'",
                if *bind_left { "" } else { " " },
                if *bind_right { "" } else { " " }
            ),
            TokenKind::Interpolation(c, s) => {
                write!(f, "{c}\"{}\"", s)
            }
            TokenKind::Comment(s) => {
                writeln!(f, "#{}", s)
            }
            TokenKind::DocComment(s) => {
                writeln!(f, "#!{}", s)
            }
            TokenKind::LineWrap(comments) => {
                write!(f, "\n\\ ")?;
                for comment in comments {
                    write!(f, "{}", comment)?;
                }
                Ok(())
            }
            TokenKind::Start => write!(f, "start of input"),
        }
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}..{}: {:?}", self.span.start, self.span.end, self.kind)
    }
}

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use super::*;

    #[test]
    fn test_string_quoting() {
        fn make_str(s: &str) -> Literal {
            Literal::String(s.to_string())
        }

        assert_snapshot!(
            make_str("hello").to_string(),
            @r###""hello""###
        );

        assert_snapshot!(
            make_str(r#"he's nice"#).to_string(),
            @r###""he's nice""###
        );

        assert_snapshot!(
            make_str(r#"he said "what up""#).to_string(),
            @r###"'he said "what up"'"###
        );

        assert_snapshot!(
            make_str(r#"he said "what's up""#).to_string(),
            @r###"'''he said "what's up"'''"###
        );

        assert_snapshot!(
            make_str(r#" single' three double""" four double"""" "#).to_string(),
            @r###"""""" single' three double""" four double"""" """"""###

        );

        assert_snapshot!(
            make_str(r#""Starts with a double quote and ' contains a single quote"#).to_string(),
            @r###"'''"Starts with a double quote and ' contains a single quote'''"###
        );
    }

    #[test]
    fn test_string_escapes() {
        assert_snapshot!(
            Literal::String(r#"hello\nworld"#.to_string()).to_string(),
            @r###""hello\\nworld""###
        );

        assert_snapshot!(
            Literal::String(r#"hello\tworld"#.to_string()).to_string(),
            @r###""hello\\tworld""###
        );

        assert_snapshot!(
            Literal::String(r#"hello
            world"#.to_string()).to_string(),
            @r###""hello\n            world""###
        );
    }

    #[test]
    fn test_raw_string_quoting() {
        // TODO: add some test for escapes
        fn make_str(s: &str) -> Literal {
            Literal::RawString(s.to_string())
        }

        assert_snapshot!(
            make_str("hello").to_string(),
            @r###"r"hello""###
        );
    }
}
