use std::collections::HashMap;

use chumsky::prelude::*;
use itertools::Itertools;
use semver::VersionReq;

use super::expr::{expr, expr_call, ident, pipeline};
use super::{ctrl, ident_part, into_stmt, keyword, new_line, pipe, with_doc_comment};
use crate::lexer::lr::{Literal, TokenKind};
use crate::parser::perror::PError;
use crate::parser::pr::*;
use crate::parser::types::type_expr;

/// The top-level parser for a PRQL file
pub fn source() -> impl Parser<TokenKind, Vec<Stmt>, Error = PError> {
    with_doc_comment(query_def())
        .or_not()
        .chain(module_contents())
        // This is the only instance we can consume newlines at the end of something, since
        // this is the end of the file
        .then_ignore(new_line().repeated())
        .then_ignore(end())
}

fn module_contents() -> impl Parser<TokenKind, Vec<Stmt>, Error = PError> {
    recursive(|module_contents| {
        let module_def = keyword("module")
            .ignore_then(ident_part())
            .then(
                module_contents
                    .then_ignore(new_line().repeated())
                    .delimited_by(ctrl('{'), ctrl('}')),
            )
            .map(|(name, stmts)| StmtKind::ModuleDef(ModuleDef { name, stmts }))
            .labelled("module definition");

        let annotation = new_line()
            .repeated()
            .at_least(1)
            .ignore_then(
                just(TokenKind::Annotate)
                    .ignore_then(expr())
                    .map(|expr| Annotation {
                        expr: Box::new(expr),
                    }),
            )
            .labelled("annotation");

        // TODO: we want to confirm that we're not allowing things on the same
        // line that should't be; e.g. `let foo = 5 let bar = 6`. We can't
        // enforce a new line here because then `module two {let houses =
        // both.alike}` fails (though we could force a new line after the
        // `module` if we wanted to?)
        //
        // let stmt_kind = new_line().repeated().at_least(1).ignore_then(choice((
        let stmt_kind = new_line().repeated().ignore_then(choice((
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

/// A variable definition could be any of:
/// - `let foo = 5`
/// - `from artists` — captured as a "main"
/// - `from artists | into x` — captured as an "into"`
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

    let main_or_into = new_line()
        .repeated()
        .ignore_then(pipeline(expr_call()))
        .map(Box::new)
        .then(
            pipe()
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
        });

    let_.or(main_or_into)
}

fn type_def() -> impl Parser<TokenKind, StmtKind, Error = PError> + Clone {
    keyword("type")
        .ignore_then(ident_part())
        .then(ctrl('=').ignore_then(type_expr()))
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
    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    use super::*;
    use crate::test::parse_with_parser;

    #[test]
    fn test_module_contents() {
        assert_yaml_snapshot!(parse_with_parser(r#"
            let world = 1
            let man = module.world
        "#, module_contents()).unwrap(), @r#"
        - VarDef:
            kind: Let
            name: world
            value:
              Literal:
                Integer: 1
              span: "0:25-26"
          span: "0:0-26"
        - VarDef:
            kind: Let
            name: man
            value:
              Indirection:
                base:
                  Ident: module
                  span: "0:49-55"
                field:
                  Name: world
              span: "0:49-61"
          span: "0:26-61"
        "#);
    }

    #[test]
    fn into() {
        assert_yaml_snapshot!(parse_with_parser(r#"
            from artists
            into x
        "#, var_def()).unwrap(), @r#"
        VarDef:
          kind: Into
          name: x
          value:
            FuncCall:
              name:
                Ident: from
                span: "0:13-17"
              args:
                - Ident: artists
                  span: "0:18-25"
            span: "0:13-25"
        "#);

        assert_yaml_snapshot!(parse_with_parser(r#"
            from artists | into x
        "#, var_def()).unwrap(), @r#"
        VarDef:
          kind: Into
          name: x
          value:
            FuncCall:
              name:
                Ident: from
                span: "0:13-17"
              args:
                - Ident: artists
                  span: "0:18-25"
            span: "0:13-25"
        "#);
    }

    #[test]
    fn let_into() {
        assert_debug_snapshot!(parse_with_parser(r#"
        let y = (
            from artists
            into x
        )
        "#, module_contents().then_ignore(end())).unwrap_err(), @r#"
        [
            Error {
                kind: Error,
                span: Some(
                    0:56-60,
                ),
                reason: Simple(
                    "unexpected keyword into while parsing pipeline",
                ),
                hints: [],
                code: None,
            },
            Error {
                kind: Error,
                span: Some(
                    0:73-73,
                ),
                reason: Simple(
                    "unexpected end of input",
                ),
                hints: [],
                code: None,
            },
        ]
        "#);
    }

    #[test]
    fn test_module() {
        let parse_module = |s| parse_with_parser(s, module_contents()).unwrap();

        let module_ast = parse_module(
            r#"
          module hello {
            let world = 1
            let man = module.world
          }
        "#,
        );

        assert_yaml_snapshot!(module_ast, @r#"
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
                span: "0:25-51"
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
                span: "0:51-86"
          span: "0:0-98"
        "#);

        // Check this parses OK. (We tried comparing it to the AST of the result
        // above, but the span information was different, so we just check it.
        // Would be nice to be able to strip spans...)
        parse_module(
            r#"

          module hello {


            let world = 1

            let man = module.world

          }
        "#,
        );
    }

    #[test]
    fn test_module_def() {
        // Same line
        assert_yaml_snapshot!(parse_with_parser(r#"module two {let houses = both.alike}
        "#, module_contents()).unwrap(), @r#"
        - ModuleDef:
            name: two
            stmts:
              - VarDef:
                  kind: Let
                  name: houses
                  value:
                    Indirection:
                      base:
                        Ident: both
                        span: "0:25-29"
                      field:
                        Name: alike
                    span: "0:25-35"
                span: "0:12-35"
          span: "0:0-36"
        "#);

        assert_yaml_snapshot!(parse_with_parser(r#"
          module dignity {
            let fair = 1
            let verona = we.lay
         }
        "#, module_contents()).unwrap(), @r#"
        - ModuleDef:
            name: dignity
            stmts:
              - VarDef:
                  kind: Let
                  name: fair
                  value:
                    Literal:
                      Integer: 1
                    span: "0:51-52"
                span: "0:27-52"
              - VarDef:
                  kind: Let
                  name: verona
                  value:
                    Indirection:
                      base:
                        Ident: we
                        span: "0:78-80"
                      field:
                        Name: lay
                    span: "0:78-84"
                span: "0:52-84"
          span: "0:0-95"
        "#);
    }

    #[test]
    fn doc_comment_module() {
        assert_yaml_snapshot!(parse_with_parser(r#"

        #! first doc comment
        from foo

        "#, module_contents()).unwrap(), @r#"
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
        "#);

        assert_yaml_snapshot!(parse_with_parser(r#"


        #! first doc comment
        from foo
        into x

        #! second doc comment
        from bar

        "#, module_contents()).unwrap(), @r#"
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
        "#);
    }

    #[test]
    fn doc_comment_inline_module() {
        // Check the newline doesn't get eated by the `{}` of the module
        // TODO: could give a better error when we forget the module name
        assert_yaml_snapshot!(parse_with_parser(r#"
        module bar {
          #! first doc comment
          from foo
        }
        "#, module_contents()).unwrap(), @r#"
        - ModuleDef:
            name: bar
            stmts:
              - VarDef:
                  kind: Main
                  name: main
                  value:
                    FuncCall:
                      name:
                        Ident: from
                        span: "0:63-67"
                      args:
                        - Ident: foo
                          span: "0:68-71"
                    span: "0:63-71"
                span: "0:52-71"
                doc_comment: " first doc comment"
          span: "0:0-81"
        "#);
    }

    #[test]
    fn lambdas() {
        assert_yaml_snapshot!(parse_with_parser(r#"
        let first = column <array> -> internal std.first
        "#, module_contents()).unwrap(), @r#"
        - VarDef:
            kind: Let
            name: first
            value:
              Func:
                return_ty: ~
                body:
                  Internal: std.first
                  span: "0:39-57"
                params:
                  - name: column
                    ty:
                      kind:
                        Ident:
                          - array
                      span: "0:29-34"
                      name: ~
                    default_value: ~
                named_params: []
              span: "0:21-57"
          span: "0:0-57"
        "#);

        assert_yaml_snapshot!(parse_with_parser(r#"
      module defs {
        let first = column <array> -> internal std.first
        let last  = column <array> -> internal std.last
    }
        "#, module_contents()).unwrap(), @r#"
        - ModuleDef:
            name: defs
            stmts:
              - VarDef:
                  kind: Let
                  name: first
                  value:
                    Func:
                      return_ty: ~
                      body:
                        Internal: std.first
                        span: "0:59-77"
                      params:
                        - name: column
                          ty:
                            kind:
                              Ident:
                                - array
                            span: "0:49-54"
                            name: ~
                          default_value: ~
                      named_params: []
                    span: "0:41-77"
                span: "0:20-77"
              - VarDef:
                  kind: Let
                  name: last
                  value:
                    Func:
                      return_ty: ~
                      body:
                        Internal: std.last
                        span: "0:116-133"
                      params:
                        - name: column
                          ty:
                            kind:
                              Ident:
                                - array
                            span: "0:106-111"
                            name: ~
                          default_value: ~
                      named_params: []
                    span: "0:98-133"
                span: "0:77-133"
          span: "0:0-139"
        "#);
    }
}
