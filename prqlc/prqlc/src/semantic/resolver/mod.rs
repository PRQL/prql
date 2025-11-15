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
}

#[derive(Default, Clone)]
pub struct ResolverOptions {}

impl Resolver<'_> {
    pub fn new(root_mod: &mut RootModule) -> Resolver<'_> {
        Resolver {
            root_mod,
            current_module_path: Vec::new(),
            default_namespace: None,
            in_func_call_name: false,
            id: IdGenerator::new(),
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

    // Helper function to verify basic lineage structure after append
    fn verify_append_lineage_basics(
        final_lineage: &crate::ir::pl::Lineage,
        expected_inputs: &[&str],
    ) {
        let input_names: Vec<&str> = final_lineage
            .inputs
            .iter()
            .map(|i| i.name.as_str())
            .collect();

        for expected_input in expected_inputs {
            assert!(input_names.contains(expected_input));
            assert!(final_lineage.find_input_by_name(expected_input).is_some());
        }

        assert!(!final_lineage.columns.is_empty());
        for col in &final_lineage.columns {
            match col {
                crate::ir::pl::LineageColumn::Single {
                    name, target_id, ..
                } => {
                    assert!(target_id > &0);
                    assert!(name.is_some());
                }
                crate::ir::pl::LineageColumn::All { .. } => {}
            }
        }
    }

    // Helper function to find source frames by input name
    fn find_source_frames<'a>(
        fc: &'a crate::semantic::reporting::FrameCollector,
        top_input_name: &str,
        bottom_input_name: &str,
    ) -> (
        Option<&'a crate::ir::pl::Lineage>,
        Option<&'a crate::ir::pl::Lineage>,
    ) {
        let mut top_frame = None;
        let mut bottom_frame = None;

        for (_span, frame) in &fc.frames {
            if frame.inputs.len() == 1 {
                let input_name = &frame.inputs[0].name;
                if input_name == top_input_name && top_frame.is_none() {
                    top_frame = Some(frame);
                } else if input_name == bottom_input_name && bottom_frame.is_none() {
                    bottom_frame = Some(frame);
                }
            }
        }

        (top_frame, bottom_frame)
    }

    // Helper function to verify column-level lineage for Single columns
    fn verify_single_column_lineage(
        final_lineage: &crate::ir::pl::Lineage,
        fc: &crate::semantic::reporting::FrameCollector,
        top_frame: &crate::ir::pl::Lineage,
        bottom_frame: &crate::ir::pl::Lineage,
    ) {
        assert_eq!(final_lineage.columns.len(), top_frame.columns.len());
        assert_eq!(final_lineage.columns.len(), bottom_frame.columns.len());

        for ((union_col, top_col), bottom_col) in final_lineage
            .columns
            .iter()
            .zip(top_frame.columns.iter())
            .zip(bottom_frame.columns.iter())
        {
            if let (
                crate::ir::pl::LineageColumn::Single { .. },
                crate::ir::pl::LineageColumn::Single {
                    name: top_name,
                    target_id: top_target_id,
                    ..
                },
                crate::ir::pl::LineageColumn::Single {
                    name: bottom_name,
                    target_id: bottom_target_id,
                    ..
                },
            ) = (union_col, top_col, bottom_col)
            {
                if let (Some(top_name), Some(bottom_name)) = (top_name, bottom_name) {
                    assert_eq!(top_name.name, bottom_name.name);
                }

                assert!(fc.nodes.iter().any(|n| n.id == *top_target_id));
                assert!(fc.nodes.iter().any(|n| n.id == *bottom_target_id));
            }
        }

        for col in &final_lineage.columns {
            if let crate::ir::pl::LineageColumn::Single { target_id, .. } = col {
                assert!(fc.nodes.iter().any(|n| n.id == *target_id));
            }
        }
    }

    // Helper function to verify expression graph contains all expected nodes
    fn verify_expression_graph_nodes(
        fc: &crate::semantic::reporting::FrameCollector,
        final_lineage: &crate::ir::pl::Lineage,
        top_frame: &crate::ir::pl::Lineage,
        bottom_frame: &crate::ir::pl::Lineage,
    ) {
        for input in &final_lineage.inputs {
            assert!(fc.nodes.iter().any(|n| n.id == input.id));
        }

        let top_col_target_ids: Vec<usize> = top_frame
            .columns
            .iter()
            .filter_map(|c| match c {
                crate::ir::pl::LineageColumn::Single { target_id, .. } => Some(*target_id),
                _ => None,
            })
            .collect();

        let bottom_col_target_ids: Vec<usize> = bottom_frame
            .columns
            .iter()
            .filter_map(|c| match c {
                crate::ir::pl::LineageColumn::Single { target_id, .. } => Some(*target_id),
                _ => None,
            })
            .collect();

        for target_id in &top_col_target_ids {
            assert!(fc.nodes.iter().any(|n| n.id == *target_id));
        }

        for target_id in &bottom_col_target_ids {
            assert!(fc.nodes.iter().any(|n| n.id == *target_id));
        }
    }

    #[test]
    fn test_append_union_different_tables() {
        // This test verifies that lineage tracking for append/union operations
        // correctly tracks inputs from both tables and shows column-level lineage.
        use crate::internal::pl_to_lineage;

        let query = r#"
        from employees
        select { name, salary }
        append (
            from managers
            select { name, salary }
        )
        "#;

        let pl = crate::prql_to_pl(query).unwrap();
        let fc = pl_to_lineage(pl).unwrap();
        let final_lineage = &fc.frames.last().unwrap().1;

        assert_yaml_snapshot!(final_lineage);

        verify_append_lineage_basics(final_lineage, &["employees", "managers"]);

        let (top_frame, bottom_frame) = find_source_frames(&fc, "employees", "managers");
        let top_frame = top_frame.unwrap();
        let bottom_frame = bottom_frame.unwrap();

        verify_single_column_lineage(final_lineage, &fc, top_frame, bottom_frame);

        let employees_input = final_lineage.find_input_by_name("employees").unwrap();
        let managers_input = final_lineage.find_input_by_name("managers").unwrap();

        assert!(final_lineage
            .inputs
            .iter()
            .any(|inp| inp.id == employees_input.id));
        assert!(final_lineage
            .inputs
            .iter()
            .any(|inp| inp.id == managers_input.id));

        verify_expression_graph_nodes(&fc, final_lineage, top_frame, bottom_frame);
    }

    #[test]
    fn test_append_union_same_table_with_exclude() {
        // This test attempts to exercise the All columns path by unioning
        // the same table with itself using select with exclude.
        use crate::internal::pl_to_lineage;

        let query = r#"
        from employees
        select !{name}
        append (
            from employees
            select !{salary}
        )
        "#;

        let pl = crate::prql_to_pl(query).unwrap();
        let fc = pl_to_lineage(pl).unwrap();
        let final_lineage = &fc.frames.last().unwrap().1;

        verify_append_lineage_basics(final_lineage, &["employees"]);
    }

    #[test]
    fn test_append_union_all_columns_same_input() {
        // This test exercises the All columns path with same input_id (lines 765-766)
        // to ensure code coverage for merging except sets when both All columns
        // come from the same input.
        use crate::ir::pl::{
            Expr, ExprKind, Lineage, LineageColumn, LineageInput, TransformCall, TransformKind,
        };
        use std::collections::HashSet;

        let input = LineageInput {
            id: 100,
            name: "employees".to_string(),
            table: crate::ir::pl::Ident {
                path: vec!["default_db".to_string()],
                name: "employees".to_string(),
            },
        };

        let mut top_lineage = Lineage::default();
        top_lineage.inputs.push(input.clone());
        top_lineage.columns.push(LineageColumn::All {
            input_id: 100,
            except: {
                let mut set = HashSet::new();
                set.insert("name".to_string());
                set
            },
        });

        let mut bottom_lineage = Lineage::default();
        bottom_lineage.inputs.push(input.clone());
        bottom_lineage.columns.push(LineageColumn::All {
            input_id: 100,
            except: {
                let mut set = HashSet::new();
                set.insert("salary".to_string());
                set
            },
        });

        let mut top_expr = Expr::new(ExprKind::Ident(crate::ir::pl::Ident::from_name("top")));
        top_expr.lineage = Some(top_lineage);

        let mut bottom_expr = Expr::new(ExprKind::Ident(crate::ir::pl::Ident::from_name("bottom")));
        bottom_expr.lineage = Some(bottom_lineage);

        let transform_call = TransformCall {
            kind: Box::new(TransformKind::Append(Box::new(bottom_expr))),
            input: Box::new(top_expr),
            partition: None,
            frame: crate::ir::pl::WindowFrame::default(),
            sort: Vec::new(),
        };

        let result = transform_call.infer_lineage().unwrap();

        match &result.columns[0] {
            LineageColumn::All { input_id, except } => {
                assert_eq!(*input_id, 100);
                assert!(except.contains("name"));
                assert!(except.contains("salary"));
            }
            _ => panic!("Expected All column"),
        }
    }
}
