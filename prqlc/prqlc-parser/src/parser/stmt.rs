use std::collections::HashMap;

use chumsky::prelude::*;
use itertools::Itertools;
use semver::VersionReq;

use super::common::{ctrl, ident_part, into_stmt, keyword, new_line, with_doc_comment};
use super::expr::{expr, expr_call, ident, pipeline};
use crate::lexer::lr::{Literal, TokenKind};
use crate::parser::perror::PError;
use crate::parser::pr::*;
use crate::parser::types::type_expr;

pub fn source() -> impl Parser<TokenKind, Vec<Stmt>, Error = PError> {
    with_doc_comment(query_def())
        .or_not()
        .chain(module_contents())
        // This is the only instance we can consume newlines at the end of something, since
        // this is the end of the module
        .then_ignore(new_line().repeated())
        .then_ignore(end())
}

fn module_contents() -> impl Parser<TokenKind, Vec<Stmt>, Error = PError> {
    recursive(|module_contents| {
        let module_def = keyword("module")
            .ignore_then(ident_part())
            .then(module_contents.delimited_by(ctrl('{'), ctrl('}')))
            .map(|(name, stmts)| StmtKind::ModuleDef(ModuleDef { name, stmts }))
            .labelled("module definition");

        let annotation = new_line()
            .repeated()
            // TODO: we could enforce annotations starting on a new line?
            // .at_least(1)
            .ignore_then(
                just(TokenKind::Annotate)
                    .ignore_then(expr())
                    // .then_ignore(new_line().repeated())
                    .map(|expr| Annotation {
                        expr: Box::new(expr),
                    }),
            )
            .labelled("annotation");

        // Also need to handle new_line vs. start of file here
        let stmt_kind = new_line().repeated().at_least(1).ignore_then(choice((
            module_def,
            type_def(),
            import_def(),
            var_def(),
        )));

        // Currently doc comments need to be before the annotation; probably
        // should relax this?
        with_doc_comment(
            annotation
                .repeated()
                .then(stmt_kind)
                .map_with_span(into_stmt),
        )
        .repeated()
        // .separated_by(new_line().repeated().at_least(1))
        // .allow_leading()
        // .allow_trailing()
    })
}

fn query_def() -> impl Parser<TokenKind, Stmt, Error = PError> + Clone {
    new_line()
        .repeated()
        .at_least(1)
        .ignore_then(keyword("prql"))
        .ignore_then(
            // named arg
            ident_part().then_ignore(ctrl(':')).then(expr()).repeated(),
        )
        .then_ignore(new_line())
        .try_map(|args, span| {
            let mut args: HashMap<_, _> = args.into_iter().collect();

            let version = args
                .remove("version")
                .map(|v| match v.kind {
                    ExprKind::Literal(Literal::String(v)) => {
                        VersionReq::parse(&v).map_err(|e| e.to_string())
                    }
                    _ => Err("version must be a string literal".to_string()),
                })
                .transpose()
                .map_err(|msg| PError::custom(span, msg))?;

            // TODO: `QueryDef` is currently implemented as `version` & `other`
            // fields. We want to raise an error if an unsupported field is
            // used, to avoid confusion (e.g. if someone passes `dialect`). So
            // at the moment we implement this as having a HashMap with 0 or 1
            // entries... We can decide how to implement `QueryDef` later, and
            // have this awkward construction in the meantime.
            let other = args
                .remove("target")
                .map(|v| {
                    match v.kind {
                        ExprKind::Ident(name) => return Ok(name.to_string()),
                        ExprKind::Indirection {
                            base,
                            field: IndirectionKind::Name(field),
                        } => {
                            if let ExprKind::Ident(name) = base.kind {
                                return Ok(name.to_string() + "." + &field);
                            }
                        }
                        _ => {}
                    };
                    Err("target must be a string literal".to_string())
                })
                .transpose()
                .map_err(|msg| PError::custom(span, msg))?
                .map_or_else(HashMap::new, |x| {
                    HashMap::from_iter(vec![("target".to_string(), x)])
                });

            if !args.is_empty() {
                return Err(PError::custom(
                    span,
                    format!(
                        "unknown query definition arguments {}",
                        args.keys().map(|x| format!("`{}`", x)).join(", ")
                    ),
                ));
            }

            Ok(StmtKind::QueryDef(Box::new(QueryDef { version, other })))
        })
        .map(|kind| (Vec::new(), kind))
        .map_with_span(into_stmt)
        .labelled("query header")
}

fn var_def() -> impl Parser<TokenKind, StmtKind, Error = PError> + Clone {
    let let_ = new_line()
        .repeated()
        .ignore_then(keyword("let"))
        .ignore_then(ident_part())
        .then(type_expr().delimited_by(ctrl('<'), ctrl('>')).or_not())
        .then(ctrl('=').ignore_then(expr_call()).map(Box::new).or_not())
        .map(|((name, ty), value)| {
            StmtKind::VarDef(VarDef {
                name,
                value,
                ty,
                kind: VarDefKind::Let,
            })
        })
        .labelled("variable definition");

    let main_or_into = pipeline(expr_call())
        .map(Box::new)
        .then(
            new_line()
                .repeated()
                .ignore_then(keyword("into").ignore_then(ident_part()))
                .or_not(),
        )
        .map(|(value, name)| {
            let kind = if name.is_none() {
                VarDefKind::Main
            } else {
                VarDefKind::Into
            };
            let name = name.unwrap_or_else(|| "main".to_string());

            StmtKind::VarDef(VarDef {
                name,
                kind,
                value: Some(value),
                ty: None,
            })
        })
        // TODO: this isn't really accurate, since a standard `from artists`
        // also counts as this; we should change
        .labelled("variable definition");

    let_.or(main_or_into)
}

fn type_def() -> impl Parser<TokenKind, StmtKind, Error = PError> + Clone {
    keyword("type")
        .ignore_then(ident_part())
        .then(ctrl('=').ignore_then(type_expr()).or_not())
        .map(|(name, value)| StmtKind::TypeDef(TypeDef { name, value }))
        .labelled("type definition")
}

fn import_def() -> impl Parser<TokenKind, StmtKind, Error = PError> + Clone {
    keyword("import")
        .ignore_then(ident_part().then_ignore(ctrl('=')).or_not())
        .then(ident())
        .map(|(alias, name)| StmtKind::ImportDef(ImportDef { name, alias }))
        .labelled("import statement")
}

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use super::*;
    use crate::test::parse_with_parser;

    #[test]
    fn test_module_def() {
        assert_yaml_snapshot!(parse_with_parser(r#"module hello {

            let world = 1

            let man = module.world

          }
        "#, module_contents()).unwrap(), @r###"
    ---
    - ModuleDef:
        name: hello
        stmts:
          - VarDef:
              kind: Let
              name: world
              value:
                Literal:
                  Integer: 1
                span: "0:50-51"
            span: "0:38-51"
          - VarDef:
              kind: Let
              name: man
              value:
                Indirection:
                  base:
                    Ident: module
                    span: "0:74-80"
                  field:
                    Name: world
                span: "0:74-86"
            span: "0:64-86"
      span: "0:11-98"
    "###);

        assert_yaml_snapshot!(parse_with_parser(r#"
          module hello {
            let world = 1
            let man = module.world
          }
        "#, module_contents()).unwrap(), @r###"
    ---
    - ModuleDef:
        name: hello
        stmts:
          - VarDef:
              kind: Let
              name: world
              value:
                Literal:
                  Integer: 1
                span: "0:50-51"
            span: "0:38-51"
          - VarDef:
              kind: Let
              name: man
              value:
                Indirection:
                  base:
                    Ident: module
                    span: "0:74-80"
                  field:
                    Name: world
                span: "0:74-86"
            span: "0:64-86"
      span: "0:11-98"
    "###);
    }

    #[test]
    fn test_doc_comment_module() {
        assert_yaml_snapshot!(parse_with_parser(r#"

        #! first doc comment
        from foo

        "#, module_contents()).unwrap(), @r###"
        ---
        - VarDef:
            kind: Main
            name: main
            value:
              FuncCall:
                name:
                  Ident: from
                  span: "0:39-43"
                args:
                  - Ident: foo
                    span: "0:44-47"
              span: "0:39-47"
          span: "0:30-47"
          doc_comment: " first doc comment"
        "###);

        assert_yaml_snapshot!(parse_with_parser(r#"


        #! first doc comment
        from foo
        into x

        #! second doc comment
        from bar

        "#, module_contents()).unwrap(), @r###"
        ---
        - VarDef:
            kind: Into
            name: x
            value:
              FuncCall:
                name:
                  Ident: from
                  span: "0:40-44"
                args:
                  - Ident: foo
                    span: "0:45-48"
              span: "0:40-48"
          span: "0:31-63"
          doc_comment: " first doc comment"
        - VarDef:
            kind: Main
            name: main
            value:
              FuncCall:
                name:
                  Ident: from
                  span: "0:103-107"
                args:
                  - Ident: bar
                    span: "0:108-111"
              span: "0:103-111"
          span: "0:94-111"
          doc_comment: " second doc comment"
        "###);
    }
}
