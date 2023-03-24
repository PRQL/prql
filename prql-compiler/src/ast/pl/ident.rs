use std::fmt::Write;

use itertools::Itertools;
use serde::{self, ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};

/// A name. Generally columns, tables, functions, variables.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdentParts {
    pub parts: Vec<String>,
}
impl std::fmt::Display for IdentParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_ident(f, self.clone())
    }
}
impl IdentParts {
    pub fn from_path<S: ToString>(mut path: Vec<S>) -> Self {
        let name = path.pop().unwrap().to_string();
        IdentParts {
            parts: path
                .into_iter()
                .map(|x| x.to_string())
                .chain(vec![name].into_iter())
                .collect(),
        }
    }
    pub fn from_name<S: ToString>(name: S) -> Self {
        IdentParts {
            parts: vec![name.to_string()],
        }
    }
    pub fn from_path_name<S: ToString>(path: Vec<S>, name: S) -> Self {
        IdentParts {
            parts: path
                .into_iter()
                .map(|x| x.to_string())
                .chain(vec![name.to_string()].into_iter())
                .collect(),
        }
    }
    pub fn name(&self) -> String {
        self.parts.last().unwrap().clone()
    }
    pub fn path(&self) -> Vec<String> {
        self.parts[..self.parts.len() - 1].to_vec()
    }
    pub fn pop(self) -> Option<Self> {
        let mut path = self.path();
        path.pop().map(|name| IdentParts {
            parts: path.into_iter().chain(std::iter::once(name)).collect_vec(),
        })
    }
    pub fn pop_front(mut self) -> (String, Option<IdentParts>) {
        if self.path().is_empty() {
            (self.name(), None)
        } else {
            let first = self.parts.remove(0);
            (first, Some(IdentParts { parts: self.parts }))
        }
    }
    pub fn starts_with(&self, prefix: &IdentParts) -> bool {
        self.parts.starts_with(&prefix.parts)
    }
}

#[test]
fn test_starts_with() {
    // Over-testing, from co-pilot, can remove some of them.
    let a = IdentParts::from_path(vec!["a", "b", "c"]);
    let b = IdentParts::from_path(vec!["a", "b"]);
    let c = IdentParts::from_path(vec!["a", "b", "c", "d"]);
    let d = IdentParts::from_path(vec!["a", "b", "d"]);
    let e = IdentParts::from_path(vec!["a", "c"]);
    let f = IdentParts::from_path(vec!["b", "c"]);
    assert!(a.starts_with(&b));
    assert!(a.starts_with(&a));
    assert!(!a.starts_with(&c));
    assert!(!a.starts_with(&d));
    assert!(!a.starts_with(&e));
    assert!(!a.starts_with(&f));
}

impl std::ops::Add<IdentParts> for IdentParts {
    type Output = IdentParts;

    fn add(self, rhs: IdentParts) -> Self::Output {
        IdentParts {
            parts: self.parts.into_iter().chain(rhs.parts).collect(),
        }
    }
}

// impl Serialize for Ident {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let mut seq = serializer.serialize_seq(Some(self.path.len() + 1))?;
//         for part in &self.path {
//             seq.serialize_element(part)?;
//         }
//         seq.serialize_element(&self.name)?;
//         seq.end()
//     }
// }

// impl<'de> Deserialize<'de> for Ident {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         <Vec<String> as Deserialize>::deserialize(deserializer).map(Ident::from_path)
//     }
// }

pub fn display_ident<T>(f: &mut std::fmt::Formatter, ident: T) -> Result<(), std::fmt::Error>
where
    T: Into<IdentParts>,
{
    let ident = ident.into();
    for part in &ident.path() {
        display_ident_part(f, part)?;
        f.write_char('.')?;
    }
    display_ident_part(f, &ident.name())?;
    Ok(())
}

pub fn display_ident_part(f: &mut std::fmt::Formatter, s: &str) -> Result<(), std::fmt::Error> {
    fn forbidden_start(c: char) -> bool {
        !(('a'..='z').contains(&c) || matches!(c, '_' | '$'))
    }
    fn forbidden_subsequent(c: char) -> bool {
        !(('a'..='z').contains(&c) || ('0'..='9').contains(&c) || matches!(c, '_'))
    }
    let needs_escape = s.is_empty()
        || s.starts_with(forbidden_start)
        || (s.len() > 1 && s.chars().skip(1).any(forbidden_subsequent));

    if needs_escape {
        write!(f, "`{s}`")
    } else {
        write!(f, "{s}")
    }
}
