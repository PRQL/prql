use std::collections::HashMap;
use std::str::FromStr;

use anyhow::Result;
use prqlc::ir::decl::RootModule;
use prqlc::ir::pl::{Ident, Literal};
use prqlc::sql::Dialect;
use prqlc::{semantic, Error, ErrorMessages, Errors, Options, SourceTree, Target, WithErrorInfo};

use crate::project::{DatabaseModule, ProjectCompiled, ProjectDiscovered, SqliteConnectionParams};

#[cfg_attr(feature = "clap", derive(clap::Parser))]
#[derive(Default)]
pub struct CompileParams {}

pub fn compile(mut project: ProjectDiscovered, _: CompileParams) -> Result<ProjectCompiled> {
    let files = std::mem::take(&mut project.sources);
    let source_tree = SourceTree::new(files, Some(project.root_path.clone()));

    let mut project = parse_and_compile(&source_tree).map_err(|e| e.composed(&source_tree))?;

    project.sources = source_tree;
    Ok(project)
}

fn parse_and_compile(source_tree: &SourceTree) -> Result<ProjectCompiled, ErrorMessages> {
    let options = Options::default()
        .with_target(Target::Sql(Some(Dialect::SQLite)))
        .no_format()
        .no_signature();

    // parse and resolve
    let ast_tree = prqlc::prql_to_pl_tree(source_tree)?;
    let mut root_module = semantic::resolve(ast_tree, Default::default())?;

    // find the database module
    let database_module = find_database_module(&mut root_module)?;

    // compile all main queries
    let mut queries = HashMap::new();
    let main_idents = root_module.find_mains();
    for main_ident in main_idents {
        let main_path: Vec<_> = main_ident.iter().cloned().collect();

        let rq;
        (rq, root_module) = semantic::lower_to_ir(root_module, &main_path, &database_module.path)?;
        let sql = prqlc::rq_to_sql(rq, &options)?;

        queries.insert(main_ident, sql);
    }
    Ok(ProjectCompiled {
        sources: SourceTree::default(), // placeholder
        queries,
        database_module,
        root_module,
    })
}

fn find_database_module(root_module: &mut RootModule) -> Result<DatabaseModule, Errors> {
    let lutra_sqlite = Ident::from_path(vec!["lutra", "sqlite"]);
    let db_modules_fq = root_module.find_by_annotation_name(&lutra_sqlite);

    let db_module_fq = match db_modules_fq.len() {
        0 => {
            return Err(Error::new_simple("cannot find the database module.")
                .push_hint("define a module annotated with `@lutra.sqlite`")
                .into());
        }
        1 => db_modules_fq.into_iter().next().unwrap(),
        _ => {
            return Err(Error::new_simple("cannot query multiple databases")
                .push_hint("you can define only one module annotated with `@lutra.sqlite`")
                .push_hint("this will be supported in the future")
                .into());
        }
    };

    // extract the declaration and retrieve its annotation
    let decl = root_module.module.get(&db_module_fq).unwrap();
    let annotation = decl
        .annotations
        .iter()
        .find(|x| prqlc::semantic::is_ident_or_func_call(&x.expr, &lutra_sqlite))
        .unwrap();

    let def_id = decl.declared_at;

    // make sure that there is exactly one arg
    let arg = match &annotation.expr.kind {
        prqlc::ir::pl::ExprKind::Ident(_) => {
            return Err(Error::new_simple("missing connection parameters")
                .push_hint("add `{file='sqlite.db'}`")
                .with_span(annotation.expr.span)
                .into());
        }
        prqlc::ir::pl::ExprKind::FuncCall(call) => {
            // TODO: maybe this should be checked by actual type-checker
            if call.args.len() != 1 {
                Err(Error::new_simple("expected exactly one argument")
                    .with_span(annotation.expr.span))?;
            }
            call.args.first().unwrap()
        }
        _ => unreachable!(),
    };

    let params = prqlc::semantic::static_eval(arg.clone(), root_module)?;
    let prqlc::ir::constant::ConstExprKind::Tuple(params) = params.kind else {
        return Err(Error::new_simple("expected exactly one argument")
            .with_span(params.span)
            .into());
    };

    let file = params.into_iter().next().unwrap();
    let prqlc::ir::constant::ConstExprKind::Literal(Literal::String(file_str)) = file.kind else {
        return Err(Error::new_simple("expected a string")
            .with_span(file.span)
            .into());
    };

    let file_relative = std::path::PathBuf::from_str(&file_str)
        .map_err(|e| Error::new_simple(e.to_string()).with_span(file.span))?;
    if !file_relative.is_relative() {
        Err(
            Error::new_simple("expected a relative path to the SQLite database file")
                .with_span(file.span),
        )?;
    }

    Ok(DatabaseModule {
        path: db_module_fq.into_iter().collect(),
        def_id,
        connection_params: SqliteConnectionParams { file_relative },
    })
}
