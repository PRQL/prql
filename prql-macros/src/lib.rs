use proc_macro::{Literal, TokenStream, TokenTree};
use prql_compiler::{compile, format_error};
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

    let sql_string = match compile(&prql_string) {
        Ok(r) => r,
        Err(err) => {
            let err = format_error(err, "<prql_macro>", &prql_string, true);
            panic!("{}", err.message);
        }
    };

    TokenStream::from_iter(vec![TokenTree::Literal(Literal::string(&sql_string))].into_iter())
}
