use prql_compiler::*;
use prql_python::to_sql;

#[test]
fn parse_for_python() -> Result<()> {
    let sql = to_sql("from employees").unwrap();
    println!("{}", sql);
    Ok(())
}
