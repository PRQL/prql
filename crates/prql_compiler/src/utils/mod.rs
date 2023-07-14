mod id_gen;
mod toposort;

pub use id_gen::{IdGenerator, NameGenerator};
use once_cell::sync::Lazy;
use regex::Regex;
pub use toposort::toposort;

use anyhow::Result;
use itertools::Itertools;

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

/// Common operations on iterators over [Option<bool>]
pub trait IterOfOptBool {
    /// If any of the items is `None`, this returns `None`.
    /// If any of the items is `Some(false)`, this returns `Some(false)`.
    /// Otherwise it returns `Some(true)`.
    fn all_true(self) -> Option<bool>;

    /// If any of the items is `None`, this returns `None`.
    /// If any of the items is `Some(true)`, this returns `Some(true)`.
    /// Otherwise it returns `Some(false)`.
    fn any_true(self) -> Option<bool>;
}

impl<'a, I> IterOfOptBool for I
where
    I: Iterator<Item = &'a Option<bool>>,
{
    fn all_true(self) -> Option<bool> {
        self.cloned()
            .fold(Some(true), |a, x| a.zip(x).map(|(a, b)| a && b))
    }

    fn any_true(self) -> Option<bool> {
        self.cloned()
            .fold(Some(true), |a, x| a.zip(x).map(|(a, b)| a && b))
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
