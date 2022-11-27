mod id_gen;
mod only;
mod table_counter;

pub use id_gen::IdGenerator;
pub use only::*;
pub use table_counter::TableCounter;

use anyhow::Result;

#[cfg(test)]
#[allow(dead_code)]
pub fn diff(a: &str, b: &str) -> String {
    use similar::TextDiff;
    TextDiff::from_lines(a, b).unified_diff().to_string()
}

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
