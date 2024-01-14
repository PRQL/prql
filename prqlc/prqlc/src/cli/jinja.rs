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
//!
//! use `crate::compiler::tokens::{Span`, Token};

use std::collections::HashMap;

use anyhow::Result;
use minijinja::machinery::{Span, Token};
use regex::Regex;

const ANCHOR_PREFIX: &str = "_jinja_";

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
pub fn pre_process(source: &str) -> Result<(String, JinjaContext)> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();

    for res in minijinja::machinery::tokenize(source, false) {
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
                    let id = format!("{ANCHOR_PREFIX}{next_anchor_id}");
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

    let mut start_index = 0;
    let mut end_index = source.len();

    let mut line = 1;
    let mut col = 0;
    for (index, char) in source.chars().enumerate() {
        if char == '\n' {
            line += 1;
            col = 0;
            continue;
        } else {
            col += 1;
        }

        if line == start.start_line && col == start.start_col {
            start_index = index;
        }
        if line == end.end_line && col == end.end_col {
            end_index = index + 1;
        }
    }
    &source[start_index..end_index]
}

/// Replace anchors with their values.
pub fn post_process(source: &str, context: JinjaContext) -> String {
    let mut res = String::new();

    for stmt in context.header {
        res += stmt;
        res += "\n";
    }

    let re = Regex::new(&format!(r"{ANCHOR_PREFIX}\d+")).unwrap();

    let mut last_index = 0;
    for cap in re.captures_iter(source) {
        let cap = cap.get(0).unwrap();
        let index = cap.start();
        let anchor_id = cap.as_str();

        res += &source[last_index..index];
        res += context.anchor_map.get(anchor_id).unwrap_or(&anchor_id);

        last_index = index + anchor_id.len();
    }
    res += &source[last_index..];

    res
}

#[cfg(test)]
mod test {
    use super::{post_process, pre_process, Span};

    #[test]
    fn test_find_span() {
        let text = r#"line 1 col 13
        line 2 col 21
        some text line 3 col 31 more text
        "#;

        assert_eq!(
            super::find_span(
                text,
                vec![
                    Span {
                        start_line: 2,
                        start_col: 9,
                        end_line: 12_123_123,
                        end_col: 2_930_293,
                    },
                    Span {
                        start_line: 7_893_648,
                        start_col: 79678,
                        end_line: 3,
                        end_col: 31,
                    }
                ]
            ),
            r#"line 2 col 21
        some text line 3 col 31"#
        );
    }

    #[test]
    fn test_pre_process() {
        let src = r###"from in_process = {{ source('salesforce', 'in_process') }}"###;
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        insta::assert_yaml_snapshot!(pre_proc_text, @r###"
        ---
        from in_process = _jinja_0
        "###);
        insta::assert_yaml_snapshot!(ctx.anchor_map["_jinja_0"], @r###"
        ---
        " {{ source('salesforce', 'in_process') }}"
        "###);
    }

    #[test]
    fn test_post_process() {
        let src = r###"from in_process = {{ source('salesforce', 'in_process') }}"###;
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        let post_proc_text = post_process(&pre_proc_text, ctx);
        insta::assert_yaml_snapshot!(post_proc_text, @r###"
        ---
        "from in_process =  {{ source('salesforce', 'in_process') }}"
        "###);
    }

    #[test]
    fn test_config_interpolation() {
        let src = r###"{{ config(materialized = "table") }}\nfrom in_process = {{ source('salesforce', 'in_process') }}"###;
        let (pre_proc_text, ctx) = pre_process(src).unwrap();
        insta::assert_yaml_snapshot!(ctx.header, @r###"
        ---
        - "{{ config(materialized = \"table\") }}"
        "###);
        let post_proc_text = post_process(&pre_proc_text, ctx);
        insta::assert_yaml_snapshot!(post_proc_text, @r###"
        ---
        "{{ config(materialized = \"table\") }}\n\\nfrom in_process =  {{ source('salesforce', 'in_process') }}"
        "###);
    }
}
