use std::fs::read_dir;

use regex::Regex;
use serde_yaml::Value;
use similar_asserts::assert_eq;

use super::compile;

fn sql_normalize(sql: &str) -> String {
    let re = Regex::new(r"\n\s+").unwrap();
    re.replace_all(sql, " ").trim().to_string()
}

#[test]
fn test_website_examples() {
    for example in read_dir("../website/data/examples").unwrap().flatten() {
        let file = std::fs::File::open(example.path()).unwrap();
        let example: Value = serde_yaml::from_reader(file).unwrap();
        let prql = example.get("prql").unwrap().as_str().unwrap();

        let compiled_sql = compile(prql).unwrap();

        if let Some(sql) = example.get("sql") {
            assert_eq!(
                sql_normalize(&compiled_sql),
                sql_normalize(sql.as_str().unwrap())
            );
        }
    }
}
