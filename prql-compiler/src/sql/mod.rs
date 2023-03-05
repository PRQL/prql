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

use anyhow::Result;

use crate::{ast::rq::Query, Options, PRQL_VERSION};

use self::{context::AnchorContext, dialect::DialectHandler};

/// Translate a PRQL AST into a SQL string.
pub fn compile(query: Query, options: &Options) -> Result<String> {
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

        formatted + "\n"
    } else {
        sql
    };

    // signature
    let sql = if options.signature_comment {
        let pre = if options.format { "\n" } else { " " };
        let post = if options.format { "\n" } else { "" };
        let target = dialect
            .map(|d| format!("target:sql.{d} "))
            .unwrap_or_default();
        let signature = format!(
            "{pre}-- Generated by PRQL compiler version:{} {}(https://prql-lang.org){post}",
            *PRQL_VERSION, target,
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

    // stuff regarding current query
    query: QueryOpts,

    // stuff regarding parent queries
    query_stack: Vec<QueryOpts>,

    pub ctes: Vec<sqlparser::ast::Cte>,
}

#[derive(Default, Clone)]
struct QueryOpts {
    /// When true, column references will not include table names prefixes.
    pub omit_ident_prefix: bool,

    /// True iff codegen should generate expressions before SELECT's projection is applied.
    /// For example:
    /// - WHERE needs `pre_projection=true`, but
    /// - ORDER BY needs `pre_projection=false`.
    pub pre_projection: bool,

    /// When true, queries will contain nested sub-queries instead of WITH CTEs.
    pub forbid_ctes: bool,

    /// When true, * are not allowed.
    pub forbid_stars: bool,
}

impl Context {
    fn new(dialect: Box<dyn DialectHandler>, anchor: AnchorContext) -> Self {
        Context {
            dialect,
            anchor,
            query: QueryOpts::default(),
            query_stack: Vec::new(),
            ctes: Vec::new(),
        }
    }

    fn push_query(&mut self) {
        self.query_stack.push(self.query.clone());
    }

    fn pop_query(&mut self) {
        self.query = self.query_stack.pop().unwrap();
    }
}

#[cfg(test)]
mod test {
    use crate::compile;
    use crate::Options;

    #[test]
    fn test_end_with_new_line() {
        let sql = compile("from a", &Options::default().no_signature()).unwrap();
        assert_eq!(sql, "SELECT\n  *\nFROM\n  a\n")
    }
}
