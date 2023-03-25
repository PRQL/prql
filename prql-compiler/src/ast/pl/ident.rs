use std::fmt::Write;

use serde::{self, Deserialize, Serialize};

/// A name. Generally columns, tables, functions, variables.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ident {
    pub parts: Vec<String>,
}
impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_ident(f, self.clone())
    }
}
impl Ident {
    pub fn from_path<S: ToString>(mut path: Vec<S>) -> Self {
        let name = path.pop().unwrap().to_string();
        Ident {
            parts: path
                .into_iter()
                .map(|x| x.to_string())
                .chain(vec![name].into_iter())
                .collect(),
        }
    }
    pub fn from_name<S: ToString>(name: S) -> Self {
        Ident {
            parts: vec![name.to_string()],
        }
    }
    pub fn from_path_name<S: ToString>(path: Vec<S>, name: S) -> Self {
        Ident {
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
        path.pop().map(|name| Ident::from_path_name(path, name))
    }
    pub fn pop_front(mut self) -> (String, Option<Ident>) {
        if self.path().is_empty() {
            (self.name(), None)
        } else {
            let first = self.parts.remove(0);
            (first, Some(Ident { parts: self.parts }))
        }
    }
    pub fn starts_with(&self, prefix: &Ident) -> bool {
        self.parts.starts_with(&prefix.parts)
    }
}

impl std::ops::Add<Ident> for Ident {
    type Output = Ident;

    fn add(self, rhs: Ident) -> Self::Output {
        Ident {
            parts: self.parts.into_iter().chain(rhs.parts).collect(),
        }
    }
}

// Q: @aljazerzen do we need these now? I couldn't immediately see what they
// do (but can look more if needed).
//
// use serde::{self, ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};
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
    T: Into<Ident>,
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
