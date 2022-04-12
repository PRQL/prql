use insta::assert_snapshot;
use prql::*;

#[test]
fn parse_simple_string_to_ast() -> Result<()> {
    assert_eq!(
        serde_yaml::to_string(&parse("select 1")?)?,
        r#"---
version: ~
dialect: Generic
nodes:
  - Pipeline:
      - Select:
          - Raw: "1"
"#
    );
    Ok(())
}

#[test]
fn transpile_variables() {
    assert_snapshot!(compile("select 1").unwrap(), @r###"
    SELECT
      1
    "###);

    assert_snapshot!(compile(
r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:  gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    ct: count,
]
sort sum_gross_cost
filter ct > 200
take 20
"#).unwrap(), @r###"
    SELECT
      title,
      country,
      AVG(salary),
      SUM(salary),
      AVG(salary + payroll_tax),
      SUM(salary + payroll_tax),
      AVG(salary + payroll_tax + benefits_cost),
      SUM(salary + payroll_tax + benefits_cost) AS sum_gross_cost,
      COUNT(*) AS ct
    FROM
      employees
    WHERE
      country = 'USA'
      AND salary + payroll_tax + benefits_cost > 0
    GROUP BY
      title,
      country
    HAVING
      COUNT(*) > 200
    ORDER BY
      sum_gross_cost
    LIMIT
      20
    "###);
}

#[test]
fn transpile_functions() {
    // TODO: Compare to canonical example:
    // - Window func not yet built.
    let prql = r#"
    func lag_day x = s"lag_day_todo({x})"
    func ret x dividend_return = x / (lag_day x) - 1 + dividend_return
    func if_valid x is_valid_price = s"IF(is_valid_price, {x}, NULL)"

    from prices
    derive [
      return_total: if_valid (ret prices_adj dividend_return) is_valid_price,
    ]
    "#;
    let result = compile(prql).unwrap();

    assert_snapshot!(result, @r###"
    SELECT
      prices.*,
      IF(
        is_valid_price,
        prices_adj / lag_day_todo(prices_adj) - 1 + dividend_return,
        NULL
      ) AS return_total
    FROM
      prices
    "###);

    // Assert that the nested function has been run.
    assert!(!result.contains(&"ret prices_adj"));
    assert!(!result.contains(&"lag_day prices_adj"));
}

#[test]
fn transpile_joins() {
    let result = compile(
        r#"
from employees
join side:left salaries [emp_no]
aggregate by:[employees.emp_no] [
  emp_salary: average salary
]
join titles side:left [titles.emp_no = $.emp_no]
join dept_emp [dept_emp.emp_no = $.emp_no]
aggregate by:[dept_emp.dept_no, titles.title] [
  avg_salary: average emp_salary
]
join side:left departments [departments.dept_no = $.dept_no]
select [dept_name, title, avg_salary]
"#,
    )
    .unwrap();

    assert_snapshot!(result, @r###"
    WITH table_0 AS (
      SELECT
        employees.emp_no,
        AVG(salary) AS emp_salary
      FROM
        employees
        LEFT JOIN salaries USING(emp_no)
      GROUP BY
        employees.emp_no
    ),
    table_1 AS (
      SELECT
        dept_emp.dept_no,
        titles.title,
        AVG(emp_salary) AS avg_salary
      FROM
        table_0
        LEFT JOIN titles ON titles.emp_no = emp_no
        JOIN dept_emp ON dept_emp.emp_no = emp_no
      GROUP BY
        dept_emp.dept_no,
        titles.title
    )
    SELECT
      dept_name,
      title,
      avg_salary
    FROM
      table_1
      LEFT JOIN departments ON departments.dept_no = dept_no
    "###);

    // #213
    assert!(!result.to_lowercase().contains(&"avg(avg"));
}
