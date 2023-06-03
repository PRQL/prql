use prql_compiler::Options;
use regex::Regex;
use serde_yaml::Value;
use std::fs;

const WEBSITE_TOPPAGE: &str = "../web/website/content/_index.md";

fn compile(prql: &str) -> Result<String, prql_compiler::ErrorMessages> {
    prql_compiler::compile(prql, &Options::default().no_signature())
}

fn normalize(sql: &str) -> String {
    let re = Regex::new(r"\n\s+").unwrap();
    re.replace_all(sql, " ").trim().to_string()
}

fn example_queries() -> Vec<Value> {
    let website_contents = fs::read_to_string(WEBSITE_TOPPAGE)
        .unwrap()
        .replace("---", "");
    let yaml = serde_yaml::from_str::<Value>(&website_contents).unwrap();
    let examples = yaml
        .get("showcase_section")
        .unwrap()
        .get("examples")
        .unwrap()
        .as_sequence()
        .unwrap();

    examples.to_vec()
}

#[test]
fn test_website_examples() {
    for example in example_queries() {
        let prql = example.get("prql").unwrap().as_str().unwrap();
        let sql = example.get("sql").unwrap().as_str().unwrap();
        assert_eq!(normalize(&compile(prql).unwrap()), normalize(sql));
    }
}
