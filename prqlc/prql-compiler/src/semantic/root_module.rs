use std::collections::HashMap;

use prqlc_ast::expr::Ident;

use super::{NS_PARAM, NS_STD, NS_THAT, NS_THIS};
use crate::ir::decl::{Decl, DeclKind, Module, RootModule};

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
}
