use prql::{ast_of_string, Result, Rule};

#[test]
fn parse_simple_string_to_ast() -> Result<()> {
    assert_eq!(
        serde_yaml::to_string(&ast_of_string("select 1", Rule::query)?)?,
        r#"---
Query:
  - Pipeline:
      - Select:
          - Raw: "1"
"#
    );
    Ok(())
}
