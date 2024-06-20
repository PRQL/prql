use crate::lexer::lex_source;
use crate::lexer::lr::TokenKind;
use crate::parser::prepare_stream;
use crate::span::Span;

#[test]
fn test_prepare_stream() {
    use insta::assert_yaml_snapshot;

    let input = "from artists | filter name == 'John'";
    let tokens = lex_source(input).unwrap();

    let mut stream = prepare_stream(tokens.0.into_iter(), input, 0);
    assert_yaml_snapshot!(stream.fetch_tokens().collect::<Vec<(TokenKind, Span)>>(), @r###"
    ---
    - - Ident: from
      - "0:0-4"
    - - Ident: artists
      - "0:5-12"
    - - Control: "|"
      - "0:13-14"
    - - Ident: filter
      - "0:15-21"
    - - Ident: name
      - "0:22-26"
    - - Eq
      - "0:27-29"
    - - Literal:
          String: John
      - "0:30-36"
    "###);
}
