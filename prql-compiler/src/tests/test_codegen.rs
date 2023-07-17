#[cfg(test)]
mod test {
    use insta::assert_snapshot;
    use similar_asserts::assert_eq;

    use crate::ast::pl;
    use crate::codegen::{WriteOpt, WriteSource};

    fn assert_is_formatted(input: &str) {
        let stmt = format_single_stmt(input);

        assert_eq!(input.trim(), stmt.trim());
    }

    fn format_single_stmt(query: &str) -> String {
        use itertools::Itertools;
        let stmt = crate::prql_to_pl(query)
            .unwrap()
            .into_iter()
            .exactly_one()
            .unwrap();
        stmt.write(WriteOpt::default()).unwrap()
    }

    #[test]
    fn test_pipeline() {
        let short = pl::Expr::new(pl::ExprKind::Ident(pl::Ident::from_path(vec!["short"])));
        let long = pl::Expr::new(pl::ExprKind::Ident(pl::Ident::from_path(vec![
            "some_module",
            "submodule",
            "a_really_long_name",
        ])));

        let mut opt = WriteOpt {
            indent: 1,
            ..Default::default()
        };

        // short pipelines should be inlined
        let pipeline = pl::Expr::new(pl::ExprKind::Pipeline(pl::Pipeline {
            exprs: vec![short.clone(), short.clone(), short.clone()],
        }));
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @"(short | short | short)");

        // long pipelines should be indented
        let pipeline = pl::Expr::new(pl::ExprKind::Pipeline(pl::Pipeline {
            exprs: vec![short.clone(), long.clone(), long, short.clone()],
        }));
        // colons are a workaround to avoid trimming
        assert_snapshot!(pipeline.write(opt.clone()).unwrap(), @r###"
        (
            short
            some_module.submodule.a_really_long_name
            some_module.submodule.a_really_long_name
            short
          )
        "###);

        // sometimes, there is just not enough space
        opt.rem_width = 4;
        opt.indent = 100;
        let pipeline = pl::Expr::new(pl::ExprKind::Pipeline(pl::Pipeline { exprs: vec![short] }));
        assert!(pipeline.write(opt).is_none());
    }

    #[test]
    fn test_escaped_string() {
        assert_is_formatted(r#"filter name ~= "\\(I Can't Help\\) Falling""#);
    }

    #[test]
    fn test_double_braces() {
        assert_is_formatted(
            r#"let has_valid_title = s"regexp_contains(title, '([a-z0-9]*-){{2,}}')""#,
        );
    }

    #[test]
    fn test_unary() {
        assert_is_formatted(r#"sort {-duration}"#);

        assert_is_formatted(r#"select a = -b"#);
        assert_is_formatted(r#"join `project-bar.dataset.table` (==col_bax)"#)
    }

    #[test]
    fn test_simple() {
        assert_is_formatted(r#"aggregate average_country_salary = (average salary)"#);
    }

    #[test]
    fn test_assign() {
        assert_is_formatted(
            r#"
group {title, country} (aggregate {
  average salary,
  average gross_salary,
  sum salary,
  sum gross_salary,
  average gross_cost,
  sum_gross_cost = sum gross_cost,
  ct = count salary,
})"#,
        );
    }
    #[test]
    fn test_range() {
        assert_is_formatted(
            r#"
from foo
is_negative = -100..0
"#,
        );

        assert_is_formatted(
            r#"
from foo
is_negative = -(100..0)
"#,
        );
    }
}
