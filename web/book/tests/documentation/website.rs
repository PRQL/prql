use regex::Regex;
use serde_yaml::Value;

use super::compile;

fn sql_normalize(sql: &str) -> String {
    let re = Regex::new(r"\n\s+").unwrap();
    re.replace_all(sql, " ").trim().to_string()
}

fn website_contents() -> Value {
    let contents = include_str!("../../../website/content/_index.md").replace("---", "");
    serde_yaml::from_str::<Value>(&contents).unwrap()
}

fn website_examples() -> Vec<Value> {
    let value = website_contents();

    value
        .get("showcase_section")
        .unwrap()
        .get("examples")
        .unwrap()
        .as_sequence()
        .unwrap()
        .to_vec()
}

fn website_hero_example() -> String {
    let value = website_contents();

    value
        .get("hero_section")
        .unwrap()
        .get("prql_example")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn test_website_examples() {
    for example in website_examples() {
        let prql = example.get("prql").unwrap().as_str().unwrap();
        let sql = example.get("sql").unwrap().as_str().unwrap();
        assert_eq!(sql_normalize(&compile(prql).unwrap()), sql_normalize(sql));
    }
}

#[test]
fn test_website_hero_example() {
    let prql = website_hero_example();
    compile(&prql).unwrap();
}
