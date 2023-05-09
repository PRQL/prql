//! SQL Relational Query
//!
//! This in an internal intermediate representation between RQ and SQL AST.
//!
//! For example, RQ does not have a separate node for DISTINCT, but uses [crate::ast::rq::Take] 1 with
//! `partition`. In [super::preprocess] module, [crate::ast::rq::Transform] take is wrapped into
//! [SqlTransform], which does have [SqlTransform::Distinct].
//!
//! This module also contains compiler from RQ to SRQ.

mod anchor;
pub mod ast;
pub mod context;
mod gen_query;
pub mod preprocess;

pub(super) use gen_query::compile_query;

#[cfg(test)]
mod test {
    use anyhow::Result;

    use super::ast::SqlQuery;
    use super::*;

    use crate::sql::{Context, Dialect};
    use crate::{parser::parse, semantic::resolve};

    fn parse_and_resolve(prql: &str) -> Result<(SqlQuery, Context)> {
        let query = resolve(parse(prql)?)?;

        compile_query(query, Some(Dialect::Generic))
    }

    fn count_atomics(prql: &str) -> usize {
        let (query, _ctx) = parse_and_resolve(prql).unwrap();

        query.ctes.len() + 1
    }

    #[test]
    fn test_ctes_of_pipeline() {
        // One aggregate, take at the end
        let prql: &str = r###"
        from employees
        filter country == "USA"
        aggregate [sal = average salary]
        sort sal
        take 20
        "###;

        assert_eq!(count_atomics(prql), 1);

        // One aggregate, but take at the top
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        sort sal
        "###;

        assert_eq!(count_atomics(prql), 2);

        // A take, then two aggregates
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        aggregate [sal2 = average sal]
        sort sal2
        "###;

        assert_eq!(count_atomics(prql), 3);

        // A take, then a select
        let prql: &str = r###"
        from employees
        take 20
        select first_name
        "###;

        assert_eq!(count_atomics(prql), 1);
    }
}
