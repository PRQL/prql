use prql_compiler::*;
use prql_python::compile_prql;

#[test]
fn parse_for_python() -> Result<()> {
    compile_prql("from employees").unwrap();
    Ok(())
}
