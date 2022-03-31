// TODO:
// - It would be a bit nicer to have the script be in rust; so it didn't
//   depend on hoping we get a recently compiled version in `./target/debug` /
//   could run on any platform.
// - Currently it mixes "not yet implemented" with "an actual error". We should
//   really disaggregate those; e.g. add a comment of "not yet implemented" for
//   those that error, rather than just ignoring them.

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
