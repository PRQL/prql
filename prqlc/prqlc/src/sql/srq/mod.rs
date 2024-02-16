//! SQL Relational Query
//!
//! This in an internal intermediate representation between RQ and SQL AST.
//!
//! For example, RQ does not have a separate node for DISTINCT, but uses [crate::ast::rq::Take] 1 with
//! `partition`. In [super::srq::preprocess] module, [crate::ast::rq::Transform] take is wrapped into
//! [ast::SqlTransform], which does have [`ast::SqlTransform::Distinct`].
//!
//! This module also contains the compiler from RQ to SRQ.

mod anchor;
pub mod ast;
pub mod context;
mod gen_query;
mod postprocess;
pub mod preprocess;

pub(super) use gen_query::compile_query;

#[cfg(test)]
mod test {
    use crate::{Errors, Result};

    use super::ast::SqlQuery;
    use super::*;

    use crate::sql::Dialect;

    fn parse_and_resolve(source: &str) -> Result<SqlQuery, Errors> {
        let query = crate::semantic::test::parse_resolve_and_lower(source)?;

        let (sql, _) = compile_query(query, Some(Dialect::Generic))?;
        Ok(sql)
    }

    fn count_atomics(prql: &str) -> Result<usize, Errors> {
        let query = parse_and_resolve(prql)?;

        Ok(query.ctes.len() + 1)
    }

    #[test]
    fn test_ctes_of_pipeline() {
        // One aggregate, take at the end
        let prql: &str = r#"
        from db.employees
        filter country == "USA"
        aggregate {sal = average salary}
        sort sal
        take 20
        "#;

        assert!(count_atomics(prql).unwrap() == 1);

        // One aggregate, but take at the top
        let prql: &str = r#"
        from db.employees
        take 20
        filter country == "USA"
        aggregate {sal = average salary}
        sort sal
        "#;

        assert!(count_atomics(prql).unwrap() == 2);

        // A take, then two aggregates
        let prql: &str = r#"
        from db.employees
        take 20
        filter country == "USA"
        aggregate {sal = average salary}
        aggregate {sal2 = average sal}
        sort sal2
        "#;

        assert!(count_atomics(prql).unwrap() == 3);

        // A take, then a select
        let prql: &str = r###"
        from db.employees
        take 20
        select first_name
        "###;

        assert!(count_atomics(prql).unwrap() == 1);
    }
}
