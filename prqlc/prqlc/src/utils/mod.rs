mod id_gen;
mod toposort;

use std::{io::stderr, sync::OnceLock};

use anstream::adapter::strip_str;
pub use id_gen::{IdGenerator, NameGenerator};
use itertools::Itertools;
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

pub(crate) fn valid_ident() -> &'static Regex {
    static VALID_IDENT: OnceLock<Regex> = OnceLock::new();
    VALID_IDENT.get_or_init(|| {
        // One of:
        // - `*`
        // - An ident starting with `a-z_\$` and containing other characters `a-z0-9_\$`
        //
        // We could replace this with pomsky (regex<>pomsky : sql<>prql)
        // ^ ('*' | [ascii_lower '_$'] [ascii_lower ascii_digit '_$']* ) $
        Regex::new(r"^((\*)|(^[a-z_\$][a-z0-9_\$]*))$").unwrap()
    })
}

fn should_use_color() -> bool {
    match anstream::AutoStream::choice(&stderr()) {
        anstream::ColorChoice::Auto => true,
        anstream::ColorChoice::Always => true,
        anstream::ColorChoice::AlwaysAnsi => true,
        anstream::ColorChoice::Never => false,
    }
}

/// Strip colors, for external libraries which don't yet strip themselves, and
/// for insta snapshot tests. This will respond to environment variables such as
/// `CLI_COLOR`.
pub(crate) fn maybe_strip_colors(s: &str) -> String {
    if !should_use_color() {
        strip_str(s).to_string()
    } else {
        s.to_string()
    }
}

#[test]
fn test_write_ident_part() {
    assert!(!valid_ident().is_match(""));
}
