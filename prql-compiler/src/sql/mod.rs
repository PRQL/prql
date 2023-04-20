//! Backend for translating RQ into SQL

mod anchor;
mod ast_srq;
mod context;
mod dialect;
mod gen_expr;
mod gen_projection;
mod gen_query;
mod preprocess;
pub(crate) mod std;
pub(crate) mod utils;

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

/// This module gives access to internal machinery that gives no stability guarantees.
pub mod internal {
    use super::*;
    use crate::ast::rq::{Query, Transform};

    pub use super::ast_srq::SqlTransform;

    fn init(query: Query) -> Result<(Vec<Transform>, Context)> {
        let (ctx, relation) = AnchorContext::of(query);
        let ctx = Context::new(dialect::Dialect::Generic.handler(), ctx);

        let pipeline = (relation.kind.into_pipeline())
            .map_err(|_| anyhow::anyhow!("Main RQ relation is not a pipeline."))?;
        Ok((pipeline, ctx))
    }

    /// Applies preprocessing to the main relation in RQ. Meant for debugging purposes.
    pub fn preprocess(query: Query) -> Result<Vec<SqlTransform>> {
        let (pipeline, mut ctx) = init(query)?;

        preprocess::preprocess(pipeline, &mut ctx)
    }

    /// Applies preprocessing and anchoring to the main relation in RQ. Meant for debugging purposes.
    pub fn anchor(query: Query) -> Result<Vec<Vec<SqlTransform>>> {
        let (pipeline, mut ctx) = init(query)?;
        let pipeline = preprocess::preprocess(pipeline, &mut ctx)?;

        Ok(anchor::extract_atomics_naive(pipeline, &mut ctx.anchor))
    }
}

#[derive(Debug)]
struct Context {
    pub dialect: Box<dyn DialectHandler>,
    pub anchor: AnchorContext,

    // stuff regarding current query
    query: QueryOpts,

    // stuff regarding parent queries
    query_stack: Vec<QueryOpts>,

    pub ctes: Vec<sqlparser::ast::Cte>,
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
}

impl Default for QueryOpts {
    fn default() -> Self {
        QueryOpts {
            omit_ident_prefix: false,
            pre_projection: false,
            allow_ctes: true,
            allow_stars: true,
        }
    }
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
