use std::collections::HashMap;

use chumsky;
use chumsky::input::BorrowInput;
use chumsky::prelude::*;
use itertools::Itertools;
use semver::VersionReq;

use super::expr::{expr, expr_call, ident, pipeline};
use super::{ctrl, ident_part, into_stmt, keyword, new_line, pipe, with_doc_comment};
use crate::lexer::lr;
use crate::lexer::lr::{Literal, TokenKind};
use crate::parser::pr::*;
use crate::parser::types::type_expr;
use crate::span::Span;

use super::ParserError;

/// The top-level parser for a PRQL file
pub fn source<'a, I>() -> impl Parser<'a, I, Vec<Stmt>, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a> + chumsky::input::ValueInput<'a>,
{
    with_doc_comment(query_def())
        .or_not()
        .map(|opt| opt.into_iter().collect::<Vec<_>>())
        .then(module_contents())
        .map(|(mut first, mut second)| {
            first.append(&mut second);
            first
        })
        // This is the only instance we can consume newlines at the end of something, since
        // this is the end of the file
        .then_ignore(new_line().repeated().collect::<Vec<_>>())
        .then_ignore(end())
        .boxed()
}

fn module_contents<'a, I>() -> impl Parser<'a, I, Vec<Stmt>, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a> + chumsky::input::ValueInput<'a>,
{
    recursive(|module_contents| {
        let module_def = keyword("module")
            .ignore_then(ident_part())
            .then(
                module_contents
                    .then_ignore(new_line().repeated().collect::<Vec<_>>())
                    .delimited_by(ctrl('{'), ctrl('}')),
            )
            .map(|(name, stmts)| StmtKind::ModuleDef(ModuleDef { name, stmts }))
            .labelled("module definition");

        let annotation = new_line()
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .ignore_then(
                select_ref! { lr::Token { kind: TokenKind::Annotate, .. } => () }
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
        let stmt_kind = new_line()
            .repeated()
            .collect::<Vec<_>>()
            .ignore_then(choice((module_def, type_def(), import_def(), var_def())));

        // Currently doc comments need to be before the annotation; probably
        // should relax this?
        with_doc_comment(
            annotation
                .repeated()
                .collect::<Vec<_>>()
                .then(stmt_kind)
                .map_with(|(annotations, kind), extra| {
                    into_stmt((annotations, kind), extra.span())
                }),
        )
        .repeated()
        .collect()
    })
    .boxed()
}

fn query_def<'a, I>() -> impl Parser<'a, I, Stmt, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a> + chumsky::input::ValueInput<'a>,
{
    new_line()
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .ignore_then(keyword("prql"))
        .ignore_then(
            // named arg
            ident_part()
                .then_ignore(ctrl(':'))
                .then(expr())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(new_line())
        .validate(|args, extra, emit| {
            let span = extra.span();
            let mut args: HashMap<_, _> = args.into_iter().collect();

            let version = args.remove("version").and_then(|v| match v.kind {
                ExprKind::Literal(Literal::String(v)) => match VersionReq::parse(&v) {
                    Ok(ver) => Some(ver),
                    Err(e) => {
                        emit.emit(Rich::custom(span, e.to_string()));
                        None
                    }
                },
                _ => {
                    emit.emit(Rich::custom(span, "version must be a string literal"));
                    None
                }
            });

            let other = args
                .remove("target")
                .and_then(|v| {
                    if let ExprKind::Ident(name) = v.kind {
                        Some(name.to_string())
                    } else {
                        emit.emit(Rich::custom(span, "target must be a string literal"));
                        None
                    }
                })
                .map_or_else(HashMap::new, |x| {
                    HashMap::from_iter(vec![("target".to_string(), x)])
                });

            if !args.is_empty() {
                emit.emit(Rich::custom(
                    span,
                    format!(
                        "unknown query definition arguments {}",
                        args.keys().map(|x| format!("`{x}`")).join(", ")
                    ),
                ));
            }

            StmtKind::QueryDef(Box::new(QueryDef { version, other }))
        })
        .map(|kind| (Vec::new(), kind))
        .map_with(|(annotations, kind), extra| into_stmt((annotations, kind), extra.span()))
        .labelled("query header")
        .boxed()
}

/// A variable definition could be any of:
/// - `let foo = 5`
/// - `from artists` — captured as a "main"
/// - `from artists | into x` — captured as an "into"`
fn var_def<'a, I>() -> impl Parser<'a, I, StmtKind, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a> + chumsky::input::ValueInput<'a>,
{
    let let_ = new_line()
        .repeated()
        .collect::<Vec<_>>()
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
        .collect::<Vec<_>>()
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

    let_.or(main_or_into).boxed()
}

fn type_def<'a, I>() -> impl Parser<'a, I, StmtKind, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a> + chumsky::input::ValueInput<'a>,
{
    keyword("type")
        .ignore_then(ident_part())
        .then(ctrl('=').ignore_then(type_expr()))
        .map(|(name, value)| StmtKind::TypeDef(TypeDef { name, value }))
        .labelled("type definition")
}

fn import_def<'a, I>() -> impl Parser<'a, I, StmtKind, ParserError<'a>> + Clone
where
    I: Input<'a, Token = lr::Token, Span = Span> + BorrowInput<'a> + chumsky::input::ValueInput<'a>,
{
    keyword("import")
        .ignore_then(ident_part().then_ignore(ctrl('=')).or_not())
        .then(ident())
        .map(|(alias, name)| StmtKind::ImportDef(ImportDef { name, alias }))
        .labelled("import statement")
}

#[cfg(test)]
mod tests {
    use chumsky::prelude::*;
    use insta::{assert_debug_snapshot, assert_yaml_snapshot};

    use super::*;
    use crate::error::Error;

    fn parse_module_contents(source: &str) -> Result<Vec<Stmt>, Vec<Error>> {
        crate::parse_test!(
            source,
            module_contents()
                .then_ignore(new_line().repeated())
                .then_ignore(end())
        )
    }

    fn parse_var_def(source: &str) -> Result<StmtKind, Vec<Error>> {
        crate::parse_test!(
            source,
            var_def()
                .then_ignore(new_line().repeated())
                .then_ignore(end())
        )
    }

    fn parse_module_contents_complete(source: &str) -> Result<Vec<Stmt>, Vec<Error>> {
        crate::parse_test!(source, module_contents().then_ignore(end()))
    }

    #[test]
    fn test_module_contents() {
        assert_yaml_snapshot!(parse_module_contents(r#"
            let world = 1
            let man = module.world
        "#).unwrap(), @r#"
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
              Ident:
                - module
                - world
              span: "0:49-61"
          span: "0:26-61"
        "#);
    }

    #[test]
    fn into() {
        assert_yaml_snapshot!(parse_var_def(r#"
            from artists
            into x
        "#).unwrap(), @r#"
        VarDef:
          kind: Into
          name: x
          value:
            FuncCall:
              name:
                Ident:
                  - from
                span: "0:13-17"
              args:
                - Ident:
                    - artists
                  span: "0:18-25"
            span: "0:13-25"
        "#);

        assert_yaml_snapshot!(parse_var_def(r#"
            from artists | into x
        "#).unwrap(), @r#"
        VarDef:
          kind: Into
          name: x
          value:
            FuncCall:
              name:
                Ident:
                  - from
                span: "0:13-17"
              args:
                - Ident:
                    - artists
                  span: "0:18-25"
            span: "0:13-25"
        "#);
    }

    #[test]
    fn let_into() {
        assert_debug_snapshot!(parse_module_contents_complete(r#"
        let y = (
            from artists
            into x
        )
        "#).unwrap_err(), @r#"
        [
            Error {
                kind: Error,
                span: Some(
                    0:56-60,
                ),
                reason: Expected {
                    who: None,
                    expected: "one of doc comment, function call, function definition, new line or something else",
                    found: "keyword into",
                },
                hints: [],
                code: None,
            },
            Error {
                kind: Error,
                span: Some(
                    0:0-73,
                ),
                reason: Simple(
                    "Expected one of import statement, module definition, new line, pipeline, something else, type definition or variable definition, but didn't find anything before the end.",
                ),
                hints: [],
                code: None,
            },
        ]
        "#);
    }

    #[test]
    fn test_module() {
        let parse_module = |s| parse_module_contents(s).unwrap();

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
                    Ident:
                      - module
                      - world
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
        assert_yaml_snapshot!(parse_module_contents(r#"module two {let houses = both.alike}
        "#).unwrap(), @r#"
        - ModuleDef:
            name: two
            stmts:
              - VarDef:
                  kind: Let
                  name: houses
                  value:
                    Ident:
                      - both
                      - alike
                    span: "0:25-35"
                span: "0:12-35"
          span: "0:0-36"
        "#);

        assert_yaml_snapshot!(parse_module_contents(r#"
          module dignity {
            let fair = 1
            let verona = we.lay
         }
        "#).unwrap(), @r#"
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
                    Ident:
                      - we
                      - lay
                    span: "0:78-84"
                span: "0:52-84"
          span: "0:0-95"
        "#);
    }

    #[test]
    fn doc_comment_module() {
        assert_yaml_snapshot!(parse_module_contents(r#"

        #! first doc comment
        from foo

        "#).unwrap(), @r#"
        - VarDef:
            kind: Main
            name: main
            value:
              FuncCall:
                name:
                  Ident:
                    - from
                  span: "0:39-43"
                args:
                  - Ident:
                      - foo
                    span: "0:44-47"
              span: "0:39-47"
          span: "0:30-47"
          doc_comment: " first doc comment"
        "#);

        assert_yaml_snapshot!(parse_module_contents(r#"


        #! first doc comment
        from foo
        into x

        #! second doc comment
        from bar

        "#).unwrap(), @r#"
        - VarDef:
            kind: Into
            name: x
            value:
              FuncCall:
                name:
                  Ident:
                    - from
                  span: "0:40-44"
                args:
                  - Ident:
                      - foo
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
                  Ident:
                    - from
                  span: "0:103-107"
                args:
                  - Ident:
                      - bar
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
        assert_yaml_snapshot!(parse_module_contents(r#"
        module bar {
          #! first doc comment
          from foo
        }
        "#).unwrap(), @r#"
        - ModuleDef:
            name: bar
            stmts:
              - VarDef:
                  kind: Main
                  name: main
                  value:
                    FuncCall:
                      name:
                        Ident:
                          - from
                        span: "0:63-67"
                      args:
                        - Ident:
                            - foo
                          span: "0:68-71"
                    span: "0:63-71"
                span: "0:52-71"
                doc_comment: " first doc comment"
          span: "0:0-81"
        "#);
    }

    #[test]
    fn lambdas() {
        assert_yaml_snapshot!(parse_module_contents(r#"
        let first = column <array> -> internal std.first
        "#).unwrap(), @r#"
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

        assert_yaml_snapshot!(parse_module_contents(r#"
      module defs {
        let first = column <array> -> internal std.first
        let last  = column <array> -> internal std.last
    }
        "#).unwrap(), @r#"
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
