use regex::Regex;

use super::compile;

#[test]
fn test_readme_examples() {
    let contents = include_str!("../../../../README.md");
    // Similar to code at https://github.com/PRQL/prql/blob/65706a115a84997c608eaeda38b1aef1240fcec3/web/book/tests/snapshot.rs#L152, but specialized for the Readme.
    let re = Regex::new(r"(?s)```(elm|prql)\r?\n(?P<prql>.+?)\r?\n```").unwrap();
    assert_ne!(re.find_iter(contents).count(), 0);
    re.captures_iter(contents).for_each(|capture| {
        compile(&capture["prql"]).unwrap();
    });
}
