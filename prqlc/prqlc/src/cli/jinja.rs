//! Handling of Jinja templates
//!
//! dbt is using the following pipeline: `Jinja+SQL -> SQL -> execution`.
//!
//! To prevent messing up the templates, we have create the following pipeline:
//! ```
//! Jinja+PRQL -> Jinja+SQL -> SQL -> execution
//! ```
//!
//! But because prqlc does not (and should not) know how to handle Jinja,
//! we have to extract the interpolations, replace them something that is valid PRQL,
//! compile the query and inject interpolations back in.
//!
//! Unfortunately, this requires parsing Jinja.

use std::collections::HashMap;

use anyhow::Result;
use minijinja::{
    machinery::{Span, Token},
    syntax::SyntaxConfig,
};
use regex::Regex;

const ANCHOR_PREFIX: &str = "_jinja_";
/// Closes an anchor, so text that directly follows an interpolation (such as
/// the digits in `{{ ref('orders') }}2024`) can't merge into its id.
const ANCHOR_SUFFIX: &str = "_";

#[derive(Debug)]
pub enum JinjaBlock<'a> {
    Data(&'a str),
    Interpolation(Vec<(Token<'a>, Span)>),
}

#[derive(Default)]
pub struct JinjaContext<'a> {
    anchor_map: HashMap<String, &'a str>,
    header: Vec<&'a str>,
}

/// Parse source as Jinja template, extract all interpolations
/// and replace them with anchors.
pub fn pre_process(source: &str) -> Result<(String, JinjaContext<'_>)> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();

    for res in minijinja::machinery::tokenize(source, false, SyntaxConfig::default()) {
        let (token, span) = res?;

        if let Token::TemplateData(data) = token {
            if !current_block.is_empty() {
                blocks.push(JinjaBlock::Interpolation(current_block));
                current_block = Vec::new();
            }
            blocks.push(JinjaBlock::Data(data))
        } else {
            current_block.push((token, span));
        }
    }
    if !current_block.is_empty() {
        blocks.push(JinjaBlock::Interpolation(current_block));
    }

    let mut anchored_source = String::new();
    let mut next_anchor_id = 0;
    let mut context = JinjaContext::default();
    for block in blocks {
        match block {
            JinjaBlock::Data(data) => anchored_source += data,
            JinjaBlock::Interpolation(block) => {
                let (tokens, spans): (Vec<_>, _) = block.into_iter().unzip();

                let source_span = find_span(source, spans);

                if let Some(Token::Ident("config" | "set")) = tokens.get(1) {
                    context.header.push(source_span);
                } else {
                    let id = format!("{ANCHOR_PREFIX}{next_anchor_id}{ANCHOR_SUFFIX}");
                    next_anchor_id += 1;

                    anchored_source += &id;

                    context.anchor_map.insert(id, source_span);
                }
            }
        }
    }

    Ok((anchored_source, context))
}

fn find_span(source: &str, spans: Vec<Span>) -> &str {
    let start = spans.first().unwrap();
    let end = spans.last().unwrap();

    // Span offsets are byte offsets into `source`, so the slice stays on char
    // boundaries even when `source` contains multi-byte characters.
    &source[start.start_offset as usize..end.end_offset as usize]
}

/// Replace anchors with their values.
pub fn post_process(source: &str, context: JinjaContext) -> String {
    let mut res = String::new();

    for stmt in context.header {
        res += stmt;
        res += "\n";
    }

    // An anchor compiles as an identifier, so a target that always quotes
    // identifiers (Snowflake, Oracle) wraps it in quotes. An interpolation
    // stands for SQL rather than for a name, so a quoted identifier whose
    // entire contents are one anchor is replaced along with its quotes. Each
    // quoted token (string literal or identifier, including escaped quotes
    // such as `''` or `""`) is matched whole before that check, so a quote
    // character inside one can't pair with a quote outside it, and a token
    // that merely starts with an anchor (`"_jinja_0_""x"`) keeps its quotes.
    // Anchors within any other quoted token are replaced in place.
    let anchor = format!(r"{ANCHOR_PREFIX}\d+{ANCHOR_SUFFIX}");
    let re = Regex::new(&format!(
        r#"'(?:[^']|'')*'|"(?:[^"]|"")*"|`(?:[^`]|``)*`|{anchor}"#
    ))
    .unwrap();
    let bare_anchor = Regex::new(&anchor).unwrap();
    let exact_anchor = Regex::new(&format!("^{anchor}$")).unwrap();
    let substitute = |anchor_id: &str| context.anchor_map.get(anchor_id).copied();

    let mut last_index = 0;
    for token in re.find_iter(source) {
        res += &source[last_index..token.start()];
        let text = token.as_str();

        // The text an identifier's quotes enclose, or the token itself when bare.
        let contents = match text.as_bytes()[0] {
            b'"' | b'`' => Some(&text[1..text.len() - 1]),
            b'\'' => None,
            _ => Some(text),
        };
        match contents
            .filter(|c| exact_anchor.is_match(c))
            .and_then(substitute)
        {
            Some(value) => res += value,
            None => {
                res += &bare_anchor.replace_all(text, |c: &regex::Captures| {
                    substitute(&c[0]).unwrap_or(&c[0]).to_string()
                })
            }
        }

        last_index = token.end();
    }
    res += &source[last_index..];

    res
}

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use super::{post_process, pre_process, Span};

    #[test]
    fn test_find_span() {
        let text = "line 1\n  {{ foo }} line 2";

        assert_eq!(
            super::find_span(
                text,
                vec![
                    Span {
                        start_line: 2,
                        start_col: 2,
                        start_offset: 9,
                        end_line: 2,
                        end_col: 4,
                        end_offset: 11
                    },
                    Span {
                        start_line: 2,
                        start_col: 9,
                        start_offset: 16,
                        end_line: 2,
                        end_col: 11,
                        end_offset: 18
                    }
                ]
            ),
            "{{ foo }}"
        );
    }

    #[test]
    fn test_interpolation_at_line_start() {
        // An interpolation in the first column must capture only itself, not
        // everything from the start of the file.
        let src = "from a\n{{ foo }}\nfrom b = {{ bar }}";
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        insta::assert_yaml_snapshot!(pre_proc_text, @r#""from a\n_jinja_0_\nfrom b = _jinja_1_""#);
        insta::assert_yaml_snapshot!(ctx.anchor_map["_jinja_0_"], @r#""{{ foo }}""#);
        insta::assert_yaml_snapshot!(ctx.anchor_map["_jinja_1_"], @r#""{{ bar }}""#);
        let post_proc_text = post_process(&pre_proc_text, ctx);
        insta::assert_snapshot!(post_proc_text, @"
        from a
        {{ foo }}
        from b = {{ bar }}
        ");
    }

    #[test]
    fn test_pre_process() {
        let src = r###"from in_process = {{ source('salesforce', 'in_process') }}"###;
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        insta::assert_yaml_snapshot!(pre_proc_text, @"from in_process = _jinja_0_");
        insta::assert_yaml_snapshot!(ctx.anchor_map["_jinja_0_"], @r#""{{ source('salesforce', 'in_process') }}""#);
    }

    #[test]
    fn test_post_process() {
        let src = r###"from in_process = {{ source('salesforce', 'in_process') }}"###;
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        let post_proc_text = post_process(&pre_proc_text, ctx);
        insta::assert_yaml_snapshot!(post_proc_text, @r#""from in_process = {{ source('salesforce', 'in_process') }}""#);
    }

    #[test]
    fn test_non_ascii_before_interpolation() {
        // Non-ASCII bytes before an interpolation must not cause a panic or a
        // corrupted span. `é` is two bytes but one char, so a char-indexed
        // slice would land mid-character.
        let src = r###"from café = {{ source('salesforce', 'in_process') }}"###;
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        insta::assert_yaml_snapshot!(pre_proc_text, @"from café = _jinja_0_");
        insta::assert_yaml_snapshot!(ctx.anchor_map["_jinja_0_"], @r#""{{ source('salesforce', 'in_process') }}""#);
    }

    fn compile_with_jinja(src: &str) -> String {
        let (prql, ctx) = pre_process(src).unwrap();
        let opts = prqlc::Options::default().no_signature();
        let sql = prqlc::compile(&prql, &opts).unwrap();
        post_process(&sql, ctx)
    }

    #[test]
    fn test_interpolation_followed_by_digits() {
        assert_snapshot!(compile_with_jinja("from {{ ref('orders') }}2024"), @"
        SELECT
          *
        FROM
          {{ ref('orders') }}2024
        ");
    }

    #[test]
    fn test_interpolation_not_quoted_on_always_quoting_target() {
        assert_snapshot!(compile_with_jinja(
            "prql target:sql.snowflake\nfrom {{ ref('orders') }}\nfilter amount > {{ var('min') }}"
        ), @r#"
        SELECT
          *
        FROM
          {{ ref('orders') }}
        WHERE
          "amount" > {{ var('min') }}
        "#);
        assert_snapshot!(compile_with_jinja(
            "prql target:sql.oracle\nfrom {{ ref('orders') }}"
        ), @"
        SELECT
          *
        FROM
          {{ ref('orders') }}
        ");
    }

    #[test]
    fn test_double_quotes_in_string_literal_kept() {
        assert_snapshot!(compile_with_jinja(
            r#"prql target:sql.snowflake
from orders
filter name == "\"{{ var('n') }}\" it's""#
        ), @r#"
        SELECT
          *
        FROM
          "orders"
        WHERE
          "name" = '"{{ var('n') }}" it''s'
        "#);
    }

    #[test]
    fn test_apostrophe_in_quoted_identifier() {
        assert_snapshot!(compile_with_jinja(
            r#"prql target:sql.snowflake
from {{ ref('orders') }}
select {`it's`, amount}
filter amount != 'x'"#
        ), @r#"
        SELECT
          "it's",
          "amount"
        FROM
          {{ ref('orders') }}
        WHERE
          "amount" <> 'x'
        "#);
    }

    #[test]
    fn test_interpolation_joined_to_text_in_quoted_identifier() {
        assert_snapshot!(compile_with_jinja(
            "prql target:sql.snowflake\nfrom {{ ref('orders') }}2024"
        ), @r#"
        SELECT
          *
        FROM
          "{{ ref('orders') }}2024"
        "#);
    }

    #[test]
    fn test_interpolation_in_string_literal_keeps_quotes() {
        assert_snapshot!(compile_with_jinja(
            "prql target:sql.snowflake\nfrom orders\nfilter name == '{{ var(\"n\") }}'"
        ), @r#"
        SELECT
          *
        FROM
          "orders"
        WHERE
          "name" = '{{ var("n") }}'
        "#);
    }

    #[test]
    fn test_anchor_prefix_of_quoted_identifier() {
        // `"_jinja_0_""suffix"` is one identifier containing an escaped quote;
        // its quotes enclose more than the anchor, so they stay.
        let src = r#"SELECT "_jinja_0_""suffix", `_jinja_0_``x`, "_jinja_0_" FROM t"#;
        let mut ctx = super::JinjaContext::default();
        ctx.anchor_map
            .insert("_jinja_0_".to_string(), r#"{{ var("name") }}"#);
        assert_snapshot!(post_process(src, ctx), @r#"SELECT "{{ var("name") }}""suffix", `{{ var("name") }}``x`, {{ var("name") }} FROM t"#);
    }

    #[test]
    fn test_interpolation_in_brackets_keeps_brackets() {
        // No target quotes identifiers with `[…]`, so brackets around an
        // anchor come from SQL such as an s-string index, and stay.
        let (prql, ctx) =
            pre_process("prql target:sql.duckdb\nfrom t\nselect {x = s\"arr[{{ var('i') }}]\"}")
                .unwrap();
        let opts = prqlc::Options::default().no_signature().no_format();
        let sql = prqlc::compile(&prql, &opts).unwrap();
        assert_snapshot!(post_process(&sql, ctx), @"SELECT arr[{{ var('i') }}] AS x FROM t");
    }

    #[test]
    fn test_config_interpolation() {
        let src = r###"{{ config(materialized = "table") }}\nfrom in_process = {{ source('salesforce', 'in_process') }}"###;
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        insta::assert_yaml_snapshot!(ctx.header, @r#"- "{{ config(materialized = \"table\") }}""#);
        let post_proc_text = post_process(&pre_proc_text, ctx);
        insta::assert_yaml_snapshot!(post_proc_text, @r#""{{ config(materialized = \"table\") }}\n\\nfrom in_process = {{ source('salesforce', 'in_process') }}""#);
    }
}
