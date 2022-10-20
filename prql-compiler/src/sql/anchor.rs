//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use std::collections::HashMap;

use anyhow::Result;

use crate::ast::TableRef;
use crate::ir::{
    fold_table, CId, ColumnDef, Expr, ExprKind, IdGenerator, IrFold, Query, TId, Table, TableExpr,
    Transform,
};

#[derive(Default)]
pub struct AnchorContext {
    pub(super) columns_defs: HashMap<CId, ColumnDef>,

    pub(super) columns_loc: HashMap<CId, TId>,

    pub(super) table_defs: HashMap<TId, TableDef>,

    next_col_name_id: u16,
    next_table_name_id: u16,

    pub(super) ids: IdGenerator,
}

pub struct TableDef {
    /// How to reference this table
    pub name: String,

    pub columns: Vec<ColumnDef>,

    /// How to materialize in FROM/WITH clauses
    pub expr: TableExpr,
}

impl AnchorContext {
    pub fn of(query: Query) -> (Self, Query) {
        let (ids, query) = IdGenerator::new_for(query);

        let context = AnchorContext {
            columns_defs: HashMap::new(),
            columns_loc: HashMap::new(),
            table_defs: HashMap::new(),
            next_col_name_id: 0,
            next_table_name_id: 0,
            ids,
        };
        QueryLoader::load(context, query)
    }

    pub fn get_column_name(&self, cid: &CId) -> Option<String> {
        let def = self.columns_defs.get(cid).unwrap();
        def.name.clone()
    }

    pub fn gen_table_name(&mut self) -> String {
        let id = self.next_table_name_id;
        self.next_table_name_id += 1;

        format!("table_{id}")
    }

    fn ensure_column_name(&mut self, cid: &CId) -> String {
        let def = self.columns_defs.get_mut(cid).unwrap();

        if def.name.is_none() {
            let id = self.next_col_name_id;
            self.next_col_name_id += 1;

            def.name = Some(format!("_expr_{id}"));
        }

        def.name.clone().unwrap()
    }

    pub fn materialize_expr(&self, cid: &CId) -> Expr {
        let def = self
            .columns_defs
            .get(cid)
            .unwrap_or_else(|| panic!("missing column id {cid:?}"));
        def.expr.clone()
    }

    pub fn materialize_exprs(&self, cids: &[CId]) -> Vec<Expr> {
        cids.iter().map(|cid| self.materialize_expr(cid)).collect()
    }

    pub fn materialize_name(&mut self, cid: &CId) -> String {
        // TODO: figure out which columns need name and call ensure_column_name in advance
        // let col_name = self
        //     .get_column_name(cid)
        //     .expect("a column is referred by name, but it doesn't have one");
        let col_name = self.ensure_column_name(cid);

        if let Some(tid) = self.columns_loc.get(cid) {
            let table = self.table_defs.get(tid).unwrap();

            format!("{}.{}", table.name, col_name)
        } else {
            col_name
        }
    }

    pub fn split_pipeline(
        &mut self,
        pipeline: Vec<Transform>,
        at_position: usize,
        new_table_name: &str,
    ) -> (Vec<Transform>, Vec<Transform>) {
        let new_tid = self.ids.gen_tid();

        // define columns of the new CTE
        let mut columns_redirect = HashMap::<CId, CId>::new();
        let old_columns = self.determine_select_columns(&pipeline[0..at_position]);
        let mut new_columns = Vec::new();
        for old_cid in old_columns {
            let new_cid = self.ids.gen_cid();
            columns_redirect.insert(old_cid, new_cid);

            let old_def = self.columns_defs.get(&old_cid).unwrap();

            let new_def = ColumnDef {
                id: new_cid,
                name: old_def.name.clone(),
                expr: Expr {
                    kind: ExprKind::ExternRef {
                        variable: self.ensure_column_name(&old_cid),
                        table: Some(new_tid),
                    },
                    span: None,
                },
            };
            self.columns_defs.insert(new_cid, new_def.clone());
            self.columns_loc.insert(new_cid, new_tid);
            new_columns.push(new_def);
        }

        let mut first = pipeline;
        let mut second = first.split_off(at_position);

        self.table_defs.insert(
            new_tid,
            TableDef {
                name: new_table_name.to_string(),
                expr: TableExpr::Ref(TableRef::LocalTable(new_table_name.to_string())),
                columns: new_columns,
            },
        );

        second.insert(0, Transform::From(new_tid));

        // TODO: redirect CID values in second pipeline

        (first, second)
    }

    pub fn determine_select_columns(&self, pipeline: &[Transform]) -> Vec<CId> {
        let mut columns = Vec::new();

        for transform in pipeline {
            columns = match transform {
                Transform::From(tid) => {
                    let table_def = &self.table_defs.get(tid).unwrap();
                    table_def.columns.iter().map(|c| c.id).collect()
                }
                Transform::Select(cols) => cols.clone(),
                Transform::Aggregate(cols) => cols.to_vec(),
                _ => continue,
            }
        }

        columns
    }
}

/// Loads info about [Query] into [AnchorContext]
struct QueryLoader {
    context: AnchorContext,

    current_table: Option<TId>,
}

impl QueryLoader {
    fn load(context: AnchorContext, query: Query) -> (AnchorContext, Query) {
        let mut loader = QueryLoader {
            context,
            current_table: None,
        };
        // fold query
        let query = loader.fold_query(query).unwrap();
        let mut context = loader.context;

        // move tables into Context
        for table in query.tables.clone() {
            let name = table.name.as_ref().unwrap();

            let star_col = ColumnDef {
                id: context.ids.gen_cid(),
                expr: Expr {
                    kind: ExprKind::ExternRef {
                        variable: "*".to_string(),
                        table: Some(table.id),
                    },
                    span: None,
                },
                name: None,
            };
            context.columns_loc.insert(star_col.id, table.id);
            context.columns_defs.insert(star_col.id, star_col.clone());

            let table_def = TableDef {
                name: name.clone(),
                columns: vec![star_col],
                expr: table.expr,
            };
            context.table_defs.insert(table.id, table_def);
        }

        (context, query)
    }
}

impl IrFold for QueryLoader {
    fn fold_table(&mut self, table: Table) -> Result<Table> {
        self.current_table = Some(table.id);

        fold_table(self, table)
    }

    fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
        self.context.columns_defs.insert(cd.id, cd.clone());

        if let Some(current_table) = self.current_table {
            self.context.columns_loc.insert(cd.id, current_table);
        }

        Ok(cd)
    }
}

struct CidRedirector {
    redirects: HashMap<CId, CId>,
}

impl IrFold for CidRedirector {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(self.redirects.get(&cid).cloned().unwrap_or(cid))
    }
}

#[cfg(asxas)]
mod test {

    use super::*;
    use crate::{ast::Stmt, ir::Transform, parse, semantic::resolve, utils::diff};
    use anyhow::Result;
    use insta::{assert_display_snapshot, assert_snapshot, assert_yaml_snapshot};
    use serde_yaml::to_string;

    fn resolve_and_materialize(stmts: Vec<Stmt>) -> Result<Vec<Transform>> {
        let (res, context) = resolve(stmts, None)?;

        let pipeline = res.main_pipeline;

        let context = AnchorContext::default();
        let (mat, _) = anchor_sql_select(pipeline, context, None)?;
        Ok(mat)
    }

    #[test]
    fn test_replace_variables_1() -> Result<()> {
        let query = parse(
            r#"from employees
        derive [                                         # This adds columns / variables.
            gross_salary = salary + payroll_tax,
            gross_cost =   gross_salary + benefits_cost     # Variables can use other variables.
        ]
        "#,
        )?;

        let (res, context) = resolve(query, None)?;

        let context = AnchorContext::default();
        let (mat, _) = anchor_sql_select(res.main_pipeline.clone(), context, None)?;

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        assert_display_snapshot!(diff(
            &to_string(&res.main_pipeline)?,
            &to_string(&mat)?
        ),
        @r###"
        @@ -18,25 +18,3 @@
           span:
             start: 0
             end: 14
        -- kind: !Derive
        -  - Ident: gross_salary
        -    ty: !Literal Column
        -  - Ident: gross_cost
        -    ty: !Literal Column
        -  is_complex: false
        -  partition: []
        -  window: null
        -  ty:
        -    columns:
        -    - !All 29
        -    - !Named
        -      - gross_salary
        -      - 32
        -    - !Named
        -      - gross_cost
        -      - 34
        -    sort: []
        -    tables: []
        -  span:
        -    start: 19
        -    end: 240
        "###);

        Ok(())
    }

    #[test]
    fn test_replace_variables_2() -> Result<()> {
        let query = parse(
            r#"
        from employees
        filter country == "USA"                           # Each line transforms the previous result.
        derive [                                         # This adds columns / variables.
            gross_salary = salary + payroll_tax,
            gross_cost   = gross_salary + benefits_cost    # Variables can use other variables.
        ]
        filter gross_cost > 0
        group [title, country] (
            aggregate [                  # `by` are the columns to group by.
                average salary,                              # These are aggregation calcs run on each group.
                sum     salary,
                average gross_salary,
                sum     gross_salary,
                average gross_cost,
                sum_gross_cost = sum gross_cost,
                ct = count,
            ]
        )
        sort sum_gross_cost
        filter sum_gross_cost > 200
        take 20
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(&mat);

        Ok(())
    }

    #[test]
    fn test_non_existent_function() -> Result<()> {
        let query = parse(r#"from mytable | filter (myfunc col1)"#)?;
        assert!(resolve(query, None).is_err());

        Ok(())
    }

    #[test]
    fn test_run_functions_args() -> Result<()> {
        let stmts = parse(
            r#"
        from employees
        aggregate [
            sum salary
        ]
        "#,
        )?;

        assert_yaml_snapshot!(stmts, @r###"
        ---
        - Pipeline:
            Pipeline:
              exprs:
                - FuncCall:
                    name:
                      Ident: from
                    args:
                      - Ident: employees
                    named_args: {}
                - FuncCall:
                    name:
                      Ident: aggregate
                    args:
                      - List:
                          - FuncCall:
                              name:
                                Ident: sum
                              args:
                                - Ident: salary
                              named_args: {}
                    named_args: {}
        "###);

        let (res, context) = resolve(stmts, None)?;

        let (mat, _) =
            anchor_sql_select(res.main_pipeline.clone(), AnchorContext::default(), None)?;

        // We could make a convenience function for this. It's useful for
        // showing the diffs of an operation.
        let diff = diff(&to_string(&res.main_pipeline)?, &to_string(&mat)?);
        assert!(!diff.is_empty());
        assert_display_snapshot!(diff, @r###"
        @@ -24,7 +24,6 @@
               - !String SUM(
               - !Expr
                 Ident: salary
        -        ty: Infer
               - !String )
               ty: !Literal Column
             by: []
        "###);

        Ok(())
    }

    #[test]
    fn test_run_functions_nested() -> Result<()> {
        let stmts = parse(
            r#"
        func lag_day x ->  s"lag_day_todo({x})"
        func ret x dividend_return ->  x / (lag_day x) - 1 + dividend_return

        from a
        select (ret b c)
        "#,
        )?;

        assert_yaml_snapshot!(stmts[2], @r###"
        ---
        Pipeline:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                  args:
                    - Ident: a
                  named_args: {}
              - FuncCall:
                  name:
                    Ident: select
                  args:
                    - FuncCall:
                        name:
                          Ident: ret
                        args:
                          - Ident: b
                          - Ident: c
                        named_args: {}
                  named_args: {}
        "###);

        let mat = resolve_and_materialize(stmts).unwrap();
        assert_yaml_snapshot!(mat, @r###"
        ---
        - Transform:
            From:
              name: a
              alias: ~
              declared_at: 84
        "###);

        Ok(())
    }

    #[test]
    fn test_run_inline_pipelines() -> Result<()> {
        let query = parse(
            r#"
        from a
        aggregate [one = (foo | sum), two = (foo | sum)]
        "#,
        )?;

        let (res, context) = resolve(query, None)?;

        let (mat, _) =
            anchor_sql_select(res.main_pipeline.clone(), AnchorContext::default(), None)?;

        assert_snapshot!(diff(&to_string(&res.main_pipeline)?, &to_string(&mat)?), @r###"
        @@ -20,10 +20,20 @@
             end: 15
         - kind: !Aggregate
             assigns:
        -    - Ident: one
        +    - SString:
        +      - !String SUM(
        +      - !Expr
        +        Ident: foo
        +      - !String )
               ty: !Literal Column
        -    - Ident: two
        +      alias: one
        +    - SString:
        +      - !String SUM(
        +      - !Expr
        +        Ident: foo
        +      - !String )
               ty: !Literal Column
        +      alias: two
             by: []
           is_complex: false
           partition: []
        "###);

        // Test it'll run the `sum foo` function first.
        let query = parse(
            r#"
        func plus_one x ->  x + 1

        from a
        aggregate [a = (sum foo | plus_one)]
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();

        assert_yaml_snapshot!(mat, @r###"
        ---
        - From:
            name: a
            alias: ~
            declared_at: 30
            ty:
              Table:
                columns:
                  - All: 30
                sort: []
                tables: []
        - Aggregate:
            assigns:
              - Binary:
                  left:
                    SString:
                      - String: SUM(
                      - Expr:
                          Ident: foo
                      - String: )
                    ty: Infer
                  op: Add
                  right:
                    Literal:
                      Integer: 1
                    ty:
                      Literal: Integer
                ty:
                  Literal: Column
                alias: a
            by: []
        "###);

        Ok(())
    }

    #[test]
    fn test_named_args() -> Result<()> {
        let query = parse(
            r#"
        func add x to:1 ->  x + to

        from foo_table
        derive [
            added = add bar to:3,
            added_default = add bar
        ]
        "#,
        )?;
        let mat = resolve_and_materialize(query).unwrap();

        assert_yaml_snapshot!(mat, @r###"
        ---
        - From:
            name: foo_table
            alias: ~
            declared_at: 30
            ty:
              Table:
                columns:
                  - All: 30
                sort: []
                tables: []
        "###);

        Ok(())
    }

    #[test]
    fn test_materialize_1() -> Result<()> {
        let query = parse(
            r#"
        from employees
        aggregate [
            sum salary
        ]
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat,
            @r###"
        ---
        - From:
            name: employees
            alias: ~
            declared_at: 29
            ty:
              Table:
                columns:
                  - All: 29
                sort: []
                tables: []
        - Aggregate:
            assigns:
              - SString:
                  - String: SUM(
                  - Expr:
                      Ident: salary
                  - String: )
                ty:
                  Literal: Column
            by: []
        "###
        );
        Ok(())
    }

    #[test]
    fn test_materialize_2() -> Result<()> {
        let query = parse(
            r#"
        from employees
        filter country == "USA"                           # Each line transforms the previous result.
        derive [                                         # This adds columns / variables.
            gross_salary = salary + payroll_tax,
            gross_cost =   gross_salary + benefits_cost    # Variables can use other variables.
        ]
        filter gross_cost > 0
        group [title, country] (
            aggregate [                  # `by` are the columns to group by.
                average salary,                              # These are aggregation calcs run on each group.
                sum     salary,
                average gross_salary,
                sum     gross_salary,
                average gross_cost,
                sum_gross_cost = sum gross_cost,
                ct = count,
            ]
        )
        sort sum_gross_cost
        filter sum_gross_cost > 200
        take 20
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat);
        Ok(())
    }

    #[test]
    fn test_materialize_3() -> Result<()> {
        let query = parse(
            r#"
        func interest_rate ->  0.2

        func lag_day x ->  s"lag_day_todo({x})"
        func ret x dividend_return ->  x / (lag_day x) - 1 + dividend_return
        func excess x ->  (x - interest_rate) / 252
        func if_valid x ->  s"IF(is_valid_price, {x}, NULL)"

        from prices
        derive [
            return_total      = if_valid (ret prices_adj div_ret),
            return_usd        = if_valid (ret prices_usd div_ret),
            return_excess     = excess return_total,
            return_usd_excess = excess return_usd,
        ]
        select [
            date,
            sec_id,
            return_total,
            return_usd,
            return_excess,
            return_usd_excess,
        ]
        "#,
        )?;
        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat);

        Ok(())
    }

    #[test]
    fn test_variable_after_aggregate() -> Result<()> {
        let query = parse(
            r#"
        from employees
        group [title, emp_no] (
            aggregate [emp_salary = average salary]
        )
        group [title] (
            aggregate [avg_salary = average emp_salary]
        )
        "#,
        )?;

        let mat = resolve_and_materialize(query).unwrap();
        assert_yaml_snapshot!(mat, @r###"
        ---
        - From:
            name: employees
            alias: ~
            declared_at: 29
            ty:
              Table:
                columns:
                  - All: 29
                sort: []
                tables: []
        - Group:
            by:
              - Ident: title
              - Ident: emp_no
            pipeline:
              - kind:
                  Aggregate:
                    assigns:
                      - SString:
                          - String: AVG(
                          - Expr:
                              Ident: salary
                          - String: )
                        ty:
                          Literal: Column
                        alias: emp_salary
                    by: []
                is_complex: false
                partition: []
                window: ~
                ty:
                  columns:
                    - Named:
                        - emp_salary
                        - 33
                  sort: []
                  tables: []
                span: ~
        - Group:
            by:
              - Ident: employees.title
            pipeline:
              - kind:
                  Aggregate:
                    assigns:
                      - SString:
                          - String: AVG(
                          - Expr:
                              SString:
                                - String: AVG(
                                - Expr:
                                    Ident: salary
                                - String: )
                              ty:
                                Literal: Column
                          - String: )
                        ty:
                          Literal: Column
                        alias: avg_salary
                    by: []
                is_complex: false
                partition: []
                window: ~
                ty:
                  columns:
                    - Named:
                        - avg_salary
                        - 35
                  sort: []
                  tables: []
                span: ~
        "###);

        Ok(())
    }

    #[test]
    fn test_frames_and_names() -> Result<()> {
        let query1 = parse(
            r#"
        from orders
        select [customer_no, gross, tax, gross - tax]
        take 20
        "#,
        )?;

        let query2 = parse(
            r#"
        from table_1
        join customers [customer_no]
        "#,
        )?;

        let (res1, context) = resolve(query1, None)?;
        let (res2, context) = resolve(query2, Some(context))?;

        let context = AnchorContext::default();
        let (mat, context) = anchor_sql_select(res1.main_pipeline, context, None)?;

        assert_yaml_snapshot!(mat, @r###"
        ---
        - kind:
            From:
              name: orders
              alias: ~
              declared_at: 29
              ty:
                Table:
                  columns:
                    - All: 29
                  sort: []
                  tables: []
          is_complex: false
          partition: []
          window: ~
          ty:
            columns:
              - All: 29
            sort: []
            tables: []
          span:
            start: 9
            end: 20
        - kind:
            Take:
              range:
                start: ~
                end:
                  Literal:
                    Integer: 20
              by: []
              sort: []
          is_complex: false
          partition: []
          window: ~
          ty:
            columns:
              - Named:
                  - customer_no
                  - 30
              - Named:
                  - gross
                  - 31
              - Named:
                  - tax
                  - 32
              - Unnamed: 35
            sort: []
            tables: []
          span:
            start: 83
            end: 90
        "###);

        let (mat, _) = anchor_sql_select(res2.main_pipeline, context, None)?;

        assert_yaml_snapshot!(mat, @r###"
        ---
        - kind:
            From:
              name: table_1
              alias: ~
              declared_at: 36
              ty:
                Table:
                  columns:
                    - All: 36
                  sort: []
                  tables: []
          is_complex: false
          partition: []
          window: ~
          ty:
            columns:
              - All: 36
            sort: []
            tables: []
          span:
            start: 9
            end: 21
        - kind:
            Join:
              side: Inner
              with:
                name: customers
                alias: ~
                declared_at: 38
                ty:
                  Table:
                    columns:
                      - All: 38
                    sort: []
                    tables: []
              filter:
                Using:
                  - Ident: customer_no
          is_complex: false
          partition: []
          window: ~
          ty:
            columns:
              - All: 36
              - All: 38
              - Named:
                  - customer_no
                  - 37
            sort: []
            tables:
              - 38
          span:
            start: 30
            end: 58
        "###);

        Ok(())
    }
}
