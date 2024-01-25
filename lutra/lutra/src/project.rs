use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use prqlc::ir::pl::Ident;

#[derive(Debug, Default)]
pub struct ProjectTree {
    pub path: PathBuf,

    pub sources: HashMap<PathBuf, String>,

    pub exprs: HashMap<Ident, String>,

    pub data: HashSet<PathBuf>,
}

impl std::fmt::Display for ProjectTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut r = format!("path: {}\nsources:\n", self.path.to_string_lossy());

        for source in self.sources.keys() {
            r += "- ";
            r += &source.to_string_lossy();
            r += "\n";
        }

        f.write_str(&r)
    }
}
