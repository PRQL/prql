use prql_compiler::Options;
use regex::Regex;
use serde_yaml::Value;
use std::fs;

const WEBSITE_TOPPAGE: &str = "../web/website/content/_index.md";

fn compile(prql: &str) -> Result<String, prql_compiler::ErrorMessages> {
    prql_compiler::compile(prql, &Options::default().no_signature())
}

fn sql_normalize(sql: &str) -> String {
    let re = Regex::new(r"\n\s+").unwrap();
    re.replace_all(sql, " ").trim().to_string()
}

fn website_contents() -> Value {
    let contents = fs::read_to_string(WEBSITE_TOPPAGE)
        .unwrap()
        .replace("---", "");

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
fn test_readme_examples() {
    let contents = fs::read_to_string("../README.md").unwrap();
    let re = Regex::new(r"(?s)```(elm|prql)\n(?P<prql>.+?)\n```").unwrap();
    for cap in re.captures_iter(&contents) {
        let prql = &cap["prql"];
        compile(prql).unwrap();
    }
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
