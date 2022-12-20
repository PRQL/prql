use proc_macro::{Literal, TokenStream, TokenTree};
use syn::{Expr, ExprLit, Lit};

#[proc_macro]
pub fn prql(input: TokenStream) -> TokenStream {
    let input: Expr = syn::parse(input).unwrap();

    let prql_string = match input {
        Expr::Lit(ExprLit {
            lit: Lit::Str(lit_str),
            ..
        }) => lit_str.value(),
        _ => panic!("prql! proc macro expected a string"),
    };

    let sql_string = match prql_compiler::compile(&prql_string) {
        Ok(r) => r,
        Err(err) => {
            panic!("{}", err);
        }
    };

    TokenStream::from_iter(vec![TokenTree::Literal(Literal::string(&sql_string))].into_iter())
}
