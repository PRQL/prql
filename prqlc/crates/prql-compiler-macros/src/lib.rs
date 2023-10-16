//! Macros for PRQL compilation at build time.
//!
//! ```
//! use prql_compiler_macros::prql_to_sql;
//!
//! let sql: &str = prql_to_sql!("from albums | select {title, artist_id}");
//! assert_eq!(sql, "SELECT title, artist_id FROM albums");
//! ```
//!
//! "at build time" means that PRQL will be compiled during Rust compilation,
//! producing errors alongside Rust errors. Limited to string literals.
use proc_macro::{Literal, TokenStream, TokenTree};
use syn::{Expr, ExprLit, Lit};

#[proc_macro]
pub fn prql_to_sql(input: TokenStream) -> TokenStream {
    let input: Expr = syn::parse(input).unwrap();

    let prql_string = match input {
        Expr::Lit(ExprLit {
            lit: Lit::Str(lit_str),
            ..
        }) => lit_str.value(),
        _ => panic!("prql! proc macro expected a string"),
    };

    let opts = prql_compiler::Options::default().no_format().no_signature();

    let sql_string = match prql_compiler::compile(&prql_string, &opts) {
        Ok(r) => r,
        Err(err) => {
            panic!("{}", err);
        }
    };

    TokenStream::from_iter(vec![TokenTree::Literal(Literal::string(&sql_string))])
}
