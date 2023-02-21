use anyhow::{anyhow, Result};

use crate::ast::pl::Expr;
use crate::error::{Error, Reason};

pub trait IntoOnly {
    type Item;

    fn into_only(self) -> Result<Self::Item>;
}

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

pub trait Only<T> {
    fn only(&self) -> Result<&T>;
}

impl<T> Only<T> for [T]
where
    T: std::fmt::Debug,
{
    fn only(&self) -> Result<&T> {
        if self.len() == 1 {
            Ok(self.first().unwrap())
        } else {
            Err(anyhow!("Expected 1 item, got {}; {:?}", self.len(), self))
        }
    }
}
