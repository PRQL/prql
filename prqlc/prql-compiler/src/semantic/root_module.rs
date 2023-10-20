use std::collections::HashMap;

use anyhow::Result;
use prqlc_ast::{expr::Ident, stmt::QueryDef, Span};

use super::{NS_MAIN, NS_PARAM, NS_QUERY_DEF, NS_STD, NS_THAT, NS_THIS};
use crate::ir::decl::{Decl, DeclKind, Module, RootModule, TableExpr};

type HintAndSpan = (Option<String>, Option<Span>);

impl RootModule {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        // Each module starts with a default namespace that contains a wildcard
        // and the standard library.
        RootModule {
            module: Module {
                names: HashMap::from([
                    (
                        "default_db".to_string(),
                        Decl::from(DeclKind::Module(Module::new_database())),
                    ),
                    (NS_STD.to_string(), Decl::from(DeclKind::default())),
                ]),
                shadowed: None,
                redirects: vec![
                    Ident::from_name(NS_THIS),
                    Ident::from_name(NS_THAT),
                    Ident::from_name(NS_PARAM),
                    Ident::from_name(NS_STD),
                ],
            },
            span_map: HashMap::new(),
        }
    }

    /// Finds that main pipeline given a path to either main itself or its parent module.
    /// Returns main expr and fq ident of the decl.
    pub fn find_main_rel(&self, path: &[String]) -> Result<(&TableExpr, Ident), HintAndSpan> {
        let (decl, ident) = self.find_main(path).map_err(|x| (x, None))?;

        let span = decl
            .declared_at
            .and_then(|id| self.span_map.get(&id))
            .cloned();

        let decl = (decl.kind.as_table_decl())
            .ok_or((Some(format!("{ident} is not a relational variable")), span))?;

        Ok((&decl.expr, ident))
    }

    pub fn find_main(&self, path: &[String]) -> Result<(&Decl, Ident), Option<String>> {
        let mut tried_idents = Vec::new();

        // is path referencing the relational var directly?
        if !path.is_empty() {
            let ident = Ident::from_path(path.to_vec());
            let decl = self.module.get(&ident);

            if let Some(decl) = decl {
                return Ok((decl, ident));
            } else {
                tried_idents.push(ident.to_string());
            }
        }

        // is path referencing the parent module?
        {
            let mut path = path.to_vec();
            path.push(NS_MAIN.to_string());

            let ident = Ident::from_path(path);
            let decl = self.module.get(&ident);

            if let Some(decl) = decl {
                return Ok((decl, ident));
            } else {
                tried_idents.push(ident.to_string());
            }
        }

        Err(Some(format!(
            "Expected a declaration at {}",
            tried_idents.join(" or ")
        )))
    }

    pub fn find_query_def(&self, main: &Ident) -> Option<&QueryDef> {
        let ident = Ident {
            path: main.path.clone(),
            name: NS_QUERY_DEF.to_string(),
        };

        let decl = self.module.get(&ident)?;
        decl.kind.as_query_def()
    }

    /// Finds all main pipelines.
    pub fn find_mains(&self) -> Vec<Ident> {
        self.module.find_by_suffix(NS_MAIN)
    }
}
