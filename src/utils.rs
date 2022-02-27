use anyhow::{anyhow, Result};

// Inspired by version in sqlparser-rs; I'm surprised there isn't a version in
// the stdlib / Itertools.
/// Returns the only element of an Iterator, or an error if it has more than one element.
pub trait IntoOnly {
    fn into_only(self) -> Result<Self::Item>
    where
        Self: IntoIterator;
}

impl<T, I> IntoOnly for I
where
    I: IntoIterator<Item = T>,
    I: std::fmt::Debug,
    <I as IntoIterator>::IntoIter: std::fmt::Debug,
{
    fn into_only(self) -> Result<T> {
        let mut iter = self.into_iter();
        if let (Some(item), None) = (iter.next(), iter.next()) {
            Ok(item)
        } else {
            Err(anyhow!(
                // Can't get the debug of the iterator because it's already
                // consumed; is there a way around this? I guess we could show
                // the items after the second, which is kinda weird.
                "`into_only` called on collection without exactly one item",
            ))
        }
    }
}

pub trait Only<T> {
    fn only(&self) -> Result<&T>;
}

impl<T> Only<T> for Vec<T>
where
    T: std::fmt::Debug,
{
    fn only(&self) -> Result<&T> {
        if self.len() == 1 {
            Ok(&self[0])
        } else {
            Err(anyhow!("Expected 1 item, got {}; {:?}", self.len(), self))
        }
    }
}
