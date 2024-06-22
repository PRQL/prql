use std::collections::HashMap;

use crate::ir::decl::RootModule;
use crate::utils::IdGenerator;

mod expr;
mod flatten;
mod functions;
mod inference;
mod names;
mod static_eval;
mod stmt;
mod transforms;
mod types;

/// Can fold (walk) over AST and for each function call or variable find what they are referencing.
pub struct Resolver<'a> {
    root_mod: &'a mut RootModule,

    current_module_path: Vec<String>,

    default_namespace: Option<String>,

    /// Sometimes ident closures must be resolved and sometimes not. See [test::test_func_call_resolve].
    in_func_call_name: bool,

    pub id: IdGenerator<usize>,

    pub options: ResolverOptions,

    pub generics: HashMap<(usize, String), Vec<crate::ast::Ty>>,
}

#[derive(Default, Clone)]
pub struct ResolverOptions {}

impl Resolver<'_> {
    pub fn new(root_mod: &mut RootModule, options: ResolverOptions) -> Resolver {
        Resolver {
            root_mod,
            options,
            current_module_path: Vec::new(),
            default_namespace: None,
            in_func_call_name: false,
            id: IdGenerator::new(),
            generics: Default::default(),
        }
    }
}

#[cfg(test)]
pub(super) mod test {
    use insta::assert_yaml_snapshot;

    use crate::ir::pl::{Expr, Lineage, PlFold};
    use crate::{Errors, Result};

    pub fn erase_ids(expr: Expr) -> Expr {
        IdEraser {}.fold_expr(expr).unwrap()
    }

    struct IdEraser {}

    impl PlFold for IdEraser {
        fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
            expr.kind = self.fold_expr_kind(expr.kind)?;
            expr.id = None;
            expr.target_id = None;
            Ok(expr)
        }
    }

    fn parse_and_resolve(query: &str) -> Result<Expr, Errors> {
        let ctx = crate::semantic::test::parse_and_resolve(query)?;
        let (main, _) = ctx.find_main_rel(&[]).unwrap();
        Ok(*main.clone().into_relation_var().unwrap())
    }

    fn resolve_lineage(query: &str) -> Result<Lineage, Errors> {
        Ok(parse_and_resolve(query)?.lineage.unwrap())
    }

    fn resolve_derive(query: &str) -> Result<Vec<Expr>, Errors> {
        let expr = parse_and_resolve(query)?;
        let derive = expr.kind.into_transform_call().unwrap();
        let exprs = derive
            .kind
            .into_derive()
            .unwrap_or_else(|e| panic!("Failed to convert `{e:?}`"))
            .kind
            .into_tuple()
            .unwrap_or_else(|e| panic!("Failed to convert `{e:?}`"));

        let exprs = IdEraser {}.fold_exprs(exprs).unwrap();
        Ok(exprs)
    }

    #[test]
    fn test_variables_1() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            from employees
            derive {
                gross_salary = salary + payroll_tax,
                gross_cost =   gross_salary + benefits_cost
            }
            "#
        )
        .unwrap());
    }

    #[test]
    #[ignore]
    fn test_non_existent_function() {
        // `myfunc` is a valid reference to a column and
        // a column can be a function, right?
        // If not, how would we express that with type system?
        parse_and_resolve(r#"from mytable | filter (myfunc col1)"#).unwrap_err();
    }

    #[test]
    fn test_functions_1() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            let subtract = a b -> a - b

            from employees
            derive {
                net_salary = subtract gross_salary tax
            }
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_functions_nested() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            let lag_day = x -> s"lag_day_todo({x})"
            let ret = x dividend_return ->  x / (lag_day x) - 1 + dividend_return

            from a
            derive (ret b c)
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_functions_pipeline() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            from a
            derive one = (foo | sum)
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_derive(
            r#"
            let plus_one = x -> x + 1
            let plus = x y -> x + y

            from a
            derive {b = (sum foo | plus_one | plus 2)}
            "#
        )
        .unwrap());
    }
    #[test]
    fn test_named_args() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            let add_one = x to:1 -> x + to

            from foo_table
            derive {
                added = add_one bar to:3,
                added_default = add_one bar
            }
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_frames_and_names() {
        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from orders
            select {customer_no, gross, tax, gross - tax}
            take 20
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from table_1
            join customers (==customer_no)
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from e = employees
            join salaries (==emp_no)
            group {e.emp_no, e.gender} (
                aggregate {
                    emp_salary = average salaries.salary
                }
            )
            "#
        )
        .unwrap());
    }
}
