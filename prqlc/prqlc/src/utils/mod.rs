mod id_gen;
mod toposort;

pub use id_gen::{IdGenerator, NameGenerator};
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
pub use toposort::toposort;

use crate::Result;

pub trait OrMap<T> {
    /// Merges two options into one using `f`.
    /// If one of the options is None, results defaults to the other one.
    fn or_map<F>(self, b: Self, f: F) -> Self
    where
        F: FnOnce(T, T) -> T;
}

impl<T> OrMap<T> for Option<T> {
    fn or_map<F>(self, b: Self, f: F) -> Self
    where
        F: FnOnce(T, T) -> T,
    {
        match (self, b) {
            (Some(a), Some(b)) => Some(f(a, b)),
            (a, None) => a,
            (None, b) => b,
        }
    }
}

pub trait Pluck<T> {
    fn pluck<R, F>(&mut self, f: F) -> Vec<R>
    where
        F: Fn(T) -> Result<R, T>;
}

impl<T> Pluck<T> for Vec<T> {
    fn pluck<R, F>(&mut self, f: F) -> Vec<R>
    where
        F: Fn(T) -> Result<R, T>,
    {
        let mut matched = Vec::new();
        let mut not_matched = Vec::new();

        for transform in self.drain(..) {
            match f(transform) {
                Ok(t) => matched.push(t),
                Err(transform) => not_matched.push(transform),
            }
        }

        self.extend(not_matched);
        matched
    }
}

/// Breaks up a [Vec] into two parts at the position of first matching element.
/// Matching element is placed into the second part.
///
/// Zero clones.
pub trait BreakUp<T> {
    fn break_up<F>(self, f: F) -> (Vec<T>, Vec<T>)
    where
        F: FnMut(&T) -> bool;
}

impl<T> BreakUp<T> for Vec<T> {
    fn break_up<F>(mut self, f: F) -> (Vec<T>, Vec<T>)
    where
        F: FnMut(&T) -> bool,
    {
        let position = self.iter().position(f).unwrap_or(self.len());
        let after = self.drain(position..).collect_vec();
        (self, after)
    }
}

pub static VALID_IDENT: Lazy<Regex> = Lazy::new(|| {
    // One of:
    // - `*`
    // - An ident starting with `a-z_\$` and containing other characters `a-z0-9_\$`
    //
    // We could replace this with pomsky (regex<>pomsky : sql<>prql)
    // ^ ('*' | [ascii_lower '_$'] [ascii_lower ascii_digit '_$']* ) $
    Regex::new(r"^((\*)|(^[a-z_\$][a-z0-9_\$]*))$").unwrap()
});

#[test]
fn test_write_ident_part() {
    assert!(!VALID_IDENT.is_match(""));
}
