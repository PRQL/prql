use insta::{assert_snapshot, glob};
use std::fs;
use std::path::Path;

use prql::compile;

#[test]
fn run_examples() {
    glob!("examples/*.prql", |path| {
        let name = path.file_stem().unwrap().to_str().unwrap();

        let prql = fs::read_to_string(path).unwrap();

        if prql.contains("skip_test") {
            return;
        }

        let sql = compile(&prql).unwrap();
        assert_snapshot!(sql);

        let md = format!("```elm\n{prql}```\n\n```sql\n{sql}\n```\n");

        let path = format!("../examples/{name}.md");
        fs::write(Path::new(&path), md).unwrap();
    });
}
