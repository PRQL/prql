use std::collections::HashMap;

use semver::VersionReq;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct QueryDef {
    pub version: Option<VersionReq>,
    #[serde(default)]
    pub other: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum VarDefKind {
    Let,
    Into,
    Main,
}
