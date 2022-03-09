use prql::{parse, Result};

#[test]
fn parse_simple_string_to_ast() -> Result<()> {
    assert_eq!(
        serde_yaml::to_string(&parse("select 1")?)?,
        r#"---
Query:
  items:
    - Pipeline:
        - Select:
            - Raw: "1"
"#
    );
    Ok(())
}
