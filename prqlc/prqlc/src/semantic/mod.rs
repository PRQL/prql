//! Semantic resolver (name resolution, type checking and lowering to RQ)

pub mod ast_expand;
mod eval;
mod lowering;
mod module;
pub mod reporting;
mod resolver;

pub use eval::eval;
pub use lowering::lower_to_ir;

use self::resolver::Resolver;
pub use self::resolver::ResolverOptions;
use crate::debug;
use crate::ir::constant::ConstExpr;
use crate::ir::decl::{Module, RootModule};
use crate::ir::pl::{self, Expr, ImportDef, ModuleDef, Stmt, StmtKind, TypeDef, VarDef};
use crate::ir::rq::RelationalQuery;
use crate::parser::is_mod_def_for;
use crate::pr;
use crate::WithErrorInfo;
use crate::{Error, Reason, Result};

/// Runs semantic analysis on the query and lowers PL to RQ.
pub fn resolve_and_lower(
    file_tree: pr::ModuleDef,
    main_path: &[String],
    database_module_path: Option<&[String]>,
) -> Result<RelationalQuery> {
    let root_mod = resolve(file_tree, Default::default())?;

    debug::log_stage(debug::Stage::Semantic(debug::StageSemantic::Lowering));
    let default_db = [NS_DEFAULT_DB.to_string()];
    let database_module_path = database_module_path.unwrap_or(&default_db);
    let (query, _) = lowering::lower_to_ir(root_mod, main_path, database_module_path)?;

    debug::log_entry(|| debug::DebugEntryKind::ReprRq(query.clone()));
    Ok(query)
}

/// Runs semantic analysis on the query.
pub fn resolve(mut module_tree: pr::ModuleDef, options: ResolverOptions) -> Result<RootModule> {
    load_std_lib(&mut module_tree);

    // expand AST into PL
    let root_module_def = ast_expand::expand_module_def(module_tree)?;
    debug::log_entry(|| debug::DebugEntryKind::ReprPl(root_module_def.clone()));

    debug::log_stage(debug::Stage::Semantic(debug::StageSemantic::Resolver));
    // init new root module
    let mut root_module = RootModule {
        module: Module::new_root(),
        ..Default::default()
    };
    let mut resolver = Resolver::new(&mut root_module, options);

    // resolve the module def into the root module
    resolver.fold_statements(root_module_def.stmts)?;
    debug::log_entry(|| debug::DebugEntryKind::ReprDecl(root_module.clone()));

    Ok(root_module)
}

/// Preferred way of injecting std module.
pub fn load_std_lib(module_tree: &mut pr::ModuleDef) {
    if !module_tree.stmts.iter().any(|s| is_mod_def_for(s, NS_STD)) {
        let std_source = include_str!("std.prql");
        match prqlc_parser::parse_source(std_source, 0) {
            Ok(stmts) => {
                let stmt = pr::Stmt::new(pr::StmtKind::ModuleDef(pr::ModuleDef {
                    name: "std".to_string(),
                    stmts,
                }));
                module_tree.stmts.insert(0, stmt);
            }
            Err(errs) => {
                panic!("std.prql failed to compile:\n{errs:?}");
            }
        }
    }
}

pub fn static_eval(expr: Expr, root_mod: &mut RootModule) -> Result<ConstExpr> {
    let mut resolver = Resolver::new(root_mod, ResolverOptions::default());

    resolver.static_eval_to_constant(expr)
}

pub fn is_ident_or_func_call(expr: &pl::Expr, name: &pr::Ident) -> bool {
    match &expr.kind {
        pl::ExprKind::Ident(i) if i == name => true,
        pl::ExprKind::FuncCall(pl::FuncCall { name: n_expr, .. })
            if n_expr.kind.as_ident().map_or(false, |i| i == name) =>
        {
            true
        }
        _ => false,
    }
}

pub const NS_STD: &str = "std";
pub const NS_THIS: &str = "this";
pub const NS_THAT: &str = "that";
pub const NS_PARAM: &str = "_param";
pub const NS_DEFAULT_DB: &str = "default_db";
pub const NS_QUERY_DEF: &str = "prql";
pub const NS_MAIN: &str = "main";

// refers to the containing module (direct parent)
pub const NS_SELF: &str = "_self";

// implies we can infer new non-module declarations in the containing module
pub const NS_INFER: &str = "_infer";

// implies we can infer new module declarations in the containing module
pub const NS_INFER_MODULE: &str = "_infer_module";

pub const NS_GENERIC: &str = "_generic";

impl Stmt {
    pub fn new(kind: StmtKind) -> Stmt {
        Stmt {
            id: None,
            kind,
            span: None,
            annotations: Vec::new(),
        }
    }

    pub(crate) fn name(&self) -> &str {
        match &self.kind {
            StmtKind::QueryDef(_) => NS_QUERY_DEF,
            StmtKind::VarDef(VarDef { name, .. }) => name,
            StmtKind::TypeDef(TypeDef { name, .. }) => name,
            StmtKind::ModuleDef(ModuleDef { name, .. }) => name,
            StmtKind::ImportDef(ImportDef { name, alias }) => alias.as_ref().unwrap_or(&name.name),
        }
    }
}

impl pl::Expr {
    fn try_cast<T, F, S2: ToString>(self, f: F, who: Option<&str>, expected: S2) -> Result<T, Error>
    where
        F: FnOnce(pl::ExprKind) -> Result<T, pl::ExprKind>,
    {
        f(self.kind).map_err(|i| {
            Error::new(Reason::Expected {
                who: who.map(|s| s.to_string()),
                expected: expected.to_string(),
                found: format!("`{}`", write_pl(pl::Expr::new(i))),
            })
            .with_span(self.span)
        })
    }
}

/// Write a PL IR to string.
///
/// Because PL needs to be restricted back to AST, ownerships of expr is required.
pub fn write_pl(expr: pl::Expr) -> String {
    let expr = ast_expand::restrict_expr(expr);

    crate::codegen::write_expr(&expr)
}
#[cfg(test)]
pub mod test {
    use insta::assert_yaml_snapshot;

    use super::{resolve, resolve_and_lower, RootModule};
    use crate::ir::rq::RelationalQuery;
    use crate::parser::parse;
    use crate::Errors;

    pub fn parse_resolve_and_lower(query: &str) -> Result<RelationalQuery, Errors> {
        let source_tree = query.into();
        Ok(resolve_and_lower(parse(&source_tree)?, &[], None)?)
    }

    pub fn parse_and_resolve(query: &str) -> Result<RootModule, Errors> {
        let source_tree = query.into();
        Ok(resolve(parse(&source_tree)?, Default::default())?)
    }

    #[test]
    fn test_resolve_01() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from employees
        select !{foo}
        "###).unwrap().relation.columns, @r###"
        ---
        - Wildcard
        "###)
    }

    #[test]
    fn test_resolve_02() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from foo
        sort day
        window range:-4..4 (
            derive {next_four_days = sum b}
        )
        "###).unwrap().relation.columns, @r###"
        ---
        - Single: day
        - Single: b
        - Wildcard
        - Single: next_four_days
        "###)
    }

    #[test]
    fn test_resolve_03() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from a=albums
        filter is_sponsored
        select {a.*}
        "###).unwrap().relation.columns, @r###"
        ---
        - Single: is_sponsored
        - Wildcard
        "###)
    }

    #[test]
    fn test_resolve_04() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r###"
        from x
        select {a, a, a = a + 1}
        "###).unwrap().relation.columns, @r###"
        ---
        - Single: ~
        - Single: ~
        - Single: a
        "###)
    }

    #[test]
    fn test_header() {
        assert_yaml_snapshot!(parse_resolve_and_lower(r#"
        prql target:sql.mssql version:"0"

        from employees
        "#).unwrap(), @r###"
        ---
        def:
          version: ^0
          other:
            target: sql.mssql
        tables:
          - id: 0
            name: ~
            relation:
              kind:
                ExternRef:
                  LocalTable:
                    - employees
              columns:
                - Wildcard
        relation:
          kind:
            Pipeline:
              - From:
                  source: 0
                  columns:
                    - - Wildcard
                      - 0
                  name: employees
              - Select:
                  - 0
          columns:
            - Wildcard
        "### );

        assert!(parse_resolve_and_lower(
            r###"
        prql target:sql.bigquery version:foo
        from employees
        "###,
        )
        .is_err());

        assert!(parse_resolve_and_lower(
            r#"
        prql target:sql.bigquery version:"25"
        from employees
        "#,
        )
        .is_err());

        assert!(parse_resolve_and_lower(
            r###"
        prql target:sql.yah version:foo
        from employees
        "###,
        )
        .is_err());
    }
}
