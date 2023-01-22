mod id_gen;
mod only;
mod toposort;

pub use id_gen::{IdGenerator, NameGenerator};
use itertools::Itertools;
pub use only::*;
pub use toposort::toposort;

use anyhow::Result;

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
