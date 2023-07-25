use std::fmt::Display;

use crate::ir::pl::Literal;

pub struct DisplayLiteral<'a>(pub &'a Literal);

impl Display for DisplayLiteral<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Literal::Null => write!(f, "null")?,
            Literal::Integer(i) => write!(f, "{i}")?,
            Literal::Float(i) => write!(f, "{i}")?,

            Literal::String(s) => {
                // We escape all characters apart from quotes, since those we
                // use the other sort of quotes.
                // https://github.com/PRQL/prql/issues/1682 has a case which
                // isn't quite covered; this could be expanded.
                let s: String = escape_all_except_quotes(s);

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
        }
        Ok(())
    }
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
