//! Backend for translating RQ into SQL

mod dialect;
mod gen_expr;
mod gen_projection;
mod gen_query;
mod keywords;
mod operators;
mod pq;

pub use dialect::{Dialect, SupportLevel};
pub use pq::ast as pq_ast;

use self::dialect::DialectHandler;
use self::pq::ast::Cte;
use self::pq::context::AnchorContext;
use crate::debug;
use crate::ir::rq;
use crate::Result;
use crate::{compiler_version, Options};

/// Translate a PRQL AST into a SQL string.
pub fn compile(query: rq::RelationalQuery, options: &Options) -> Result<String> {
    let crate::Target::Sql(dialect) = options.target;
    let sql_ast = gen_query::translate_query(query, dialect)?;

    let sql = sql_ast.to_string();

    // formatting
    let sql = if options.format {
        let formatted = sqlformat::format(
            &sql,
            &sqlformat::QueryParams::default(),
            &sqlformat::FormatOptions::default(),
        );

        formatted + "\n"
    } else {
        sql
    };

    debug::log_entry(|| debug::DebugEntryKind::ReprSql(sql.clone()));

    // signature
    let sql = if options.signature_comment {
        let pre = if options.format { "\n" } else { " " };
        let post = if options.format { "\n" } else { "" };
        let target = dialect
            .map(|d| format!("target:sql.{d} "))
            .unwrap_or_default();
        let signature = format!(
            "{pre}-- Generated by PRQL compiler version:{} {}(https://prql-lang.org){post}",
            compiler_version(),
            target,
        );
        sql + &signature
    } else {
        sql
    };

    Ok(sql)
}

#[derive(Debug)]
struct Context {
    pub dialect: Box<dyn DialectHandler>,
    pub dialect_enum: Dialect,

    pub anchor: AnchorContext,

    // stuff regarding current query
    query: QueryOpts,

    // stuff regarding parent queries
    query_stack: Vec<QueryOpts>,

    pub ctes: Vec<Cte>,
}

#[derive(Clone, Debug)]
struct QueryOpts {
    /// When true, column references will not include table names prefixes.
    pub omit_ident_prefix: bool,

    /// True iff codegen should generate expressions before SELECT's projection is applied.
    /// For example:
    /// - WHERE needs `pre_projection=true`, but
    /// - ORDER BY needs `pre_projection=false`.
    pub pre_projection: bool,

    /// When false, queries will contain nested sub-queries instead of WITH CTEs.
    pub allow_ctes: bool,

    /// When false, * are not allowed.
    pub allow_stars: bool,

    /// True when translating function that will have an OVER clause.
    pub window_function: bool,
}

impl Default for QueryOpts {
    fn default() -> Self {
        QueryOpts {
            omit_ident_prefix: false,
            pre_projection: false,
            allow_ctes: true,
            allow_stars: true,
            window_function: false,
        }
    }
}

impl Context {
    fn new(dialect: Dialect, anchor: AnchorContext) -> Self {
        Context {
            dialect: dialect.handler(),
            dialect_enum: dialect,
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
