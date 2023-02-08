//! Backend for translating RQ into SQL

mod anchor;
mod context;
mod dialect;
mod gen_expr;
mod gen_projection;
mod gen_query;
mod preprocess;
mod std;

pub use dialect::Dialect;
pub use dialect::DIALECT_NAMES;

use anyhow::Result;

use crate::{ast::rq::Query, Options, PRQL_VERSION};

use self::{context::AnchorContext, dialect::DialectHandler};

/// Translate a PRQL AST into a SQL string.
pub fn compile(query: Query, options: Options) -> Result<String> {
    let crate::Target::Sql(dialect) = options.target;
    let sql_ast = gen_query::translate_query(query, dialect)?;

    let sql = sql_ast.to_string();

    // formatting
    let sql = if options.format {
        let formatted = sqlformat::format(
            &sql,
            &sqlformat::QueryParams::default(),
            sqlformat::FormatOptions::default(),
        );

        // The sql formatter turns `{{` into `{ {`, and while that's reasonable SQL,
        // we want to allow jinja expressions through. So we (somewhat hackily) replace
        // any `{ {` with `{{`.
        formatted.replace("{ {", "{{").replace("} }", "}}") + "\n"
    } else {
        sql
    };

    // signature
    let sql = if options.signature_comment {
        let pre = if options.format { "\n" } else { " " };
        let post = if options.format { "\n" } else { "" };
        let signature = format!(
            "{pre}-- Generated by PRQL compiler version {} (https://prql-lang.org){post}",
            *PRQL_VERSION
        );
        sql + &signature
    } else {
        sql
    };

    Ok(sql)
}

struct Context {
    pub dialect: Box<dyn DialectHandler>,
    pub anchor: AnchorContext,

    pub omit_ident_prefix: bool,

    /// True iff codegen should generate expressions before SELECT's projection is applied.
    /// For example:
    /// - WHERE needs `pre_projection=true`, but
    /// - ORDER BY needs `pre_projection=false`.
    pub pre_projection: bool,
}

#[cfg(test)]
mod test {
    use crate::compile;
    use crate::Options;

    #[test]
    fn test_end_with_new_line() {
        let sql = compile("from a", Options::default().no_signature()).unwrap();
        assert_eq!(sql, "SELECT\n  *\nFROM\n  a\n")
    }
}
