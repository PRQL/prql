pub mod ast;
#[cfg(feature = "cli")]
mod cli;
mod error;
mod parser;
mod semantic;
mod sql;
mod utils;

pub use anyhow::Result;
pub use ast::display;
#[cfg(feature = "cli")]
pub use cli::Cli;
pub use error::{format_error, SourceLocation};
pub use parser::parse;
pub use semantic::*;
pub use sql::{resolve_and_translate, translate};

/// Compile a PRQL string into a SQL string.
///
/// This has three stages:
/// - [parse] — Build an AST from a PRQL query string.
/// - [resolve] — Finds variable references, validates functions calls, determines frames.
/// - [translate] — Write a SQL string from a PRQL AST.
pub fn compile(prql: &str) -> Result<String> {
    parse(prql).and_then(resolve_and_translate)
}

/// Format an PRQL query
///
/// This has two stages:
/// - [parse] — Build an AST from a PRQL query string.
/// - [display] — Write a AST back to string.
pub fn format(prql: &str) -> Result<String> {
    parse(prql).map(display)
}

/// Compile a PRQL string into a JSON version of the Query.
pub fn to_json(prql: &str) -> Result<String> {
    Ok(serde_json::to_string(&parse(prql)?)?)
}

/// Convert JSON AST back to PRQL string
pub fn from_json(json: &str) -> Result<String> {
    Ok(display(serde_json::from_str(json)?))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::to_json;

    #[test]
    fn test_to_json() -> Result<()> {
        let json = to_json("from employees | take 10")?;
        // Since the AST is so in flux right now just test that the brackets are present
        assert_eq!(json.chars().next().unwrap(), '{');
        assert_eq!(json.chars().nth(json.len() - 1).unwrap(), '}');

        Ok(())
    }

    #[test]
    fn test_from_json() -> Result<()> {
        // Test that the SQL generated from the JSON of the PRQL is the same as the raw PRQL
        let original_prql = r#"from employees
join salaries [emp_no]
group [emp_no, gender] (
  aggregate [
    emp_salary = average salary
  ]
)
join de=dept_emp [emp_no]
join dm=dept_manager [
  (dm.dept_no == de.dept_no) and s"(de.from_date, de.to_date) OVERLAPS (dm.from_date, dm.to_date)"
]
group [dm.emp_no, gender] (
  aggregate [
    salary_avg = average emp_salary,
    salary_sd = stddev emp_salary
  ]
)
derive mng_no = dm.emp_no
join managers=employees [emp_no]
derive mng_name = s"managers.first_name || ' ' || managers.last_name"
select [mng_name, managers.gender, salary_avg, salary_sd]"#;

        let sql_from_prql = compile(original_prql)?;

        let json = to_json(original_prql)?;
        let prql_from_json = from_json(&json)?;
        let sql_from_json = compile(&prql_from_json)?;

        assert_eq!(sql_from_prql, sql_from_json);
        Ok(())
    }
}
