use anyhow::Result;

use crate::ast::pl::Expr;
use crate::error::{Error, Reason};

pub trait ExactlyOneNode {
    fn exactly_one_node(self, who: &str, occupation: &str) -> Result<Expr, Error>;
}

impl ExactlyOneNode for Vec<Expr> {
    fn exactly_one_node(mut self, who: &str, occupation: &str) -> Result<Expr, Error> {
        match self.len() {
            1 => Ok(self.remove(0)),
            0 => Err(Error {
                reason: Reason::Expected {
                    who: Some(who.to_string()),
                    expected: format!("only one {occupation}"),
                    found: "none".to_string(),
                },
                span: None,
                help: None,
            }),
            _ => Err(Error {
                reason: Reason::Expected {
                    who: Some(who.to_string()),
                    expected: format!("only one {occupation}"),
                    found: "more".to_string(),
                },
                span: self[1].span,
                help: None,
            }),
        }
    }
}
