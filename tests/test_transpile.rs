use insta::assert_snapshot;
use prql::transpile;
use prql::Result;

#[test]
fn transpile_variables() -> Result<()> {
    assert_snapshot!(transpile("select 1")?, @r###"
    SELECT
      1
    "###);

    assert_snapshot!(transpile(
r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    count: count,
]
sort sum_gross_cost
filter count > 200
take 20
"#)?, @r###"
    SELECT
      TOP (20) salary + payroll_tax AS gross_salary,
      salary + payroll_tax + benefits_cost AS gross_cost,
      SUM(salary + payroll_tax + benefits_cost) AS sum_gross_cost,
      COUNT(*) AS count,
      AVG(salary),
      SUM(salary),
      AVG(salary + payroll_tax),
      SUM(salary + payroll_tax),
      AVG(salary + payroll_tax + benefits_cost),
      *
    FROM
      employees
    WHERE
      country = 'USA'
      and salary + payroll_tax + benefits_cost > 0
    GROUP BY
      title,
      country
    HAVING
      COUNT(*) > 200
    ORDER BY
      SUM(salary + payroll_tax + benefits_cost)
    "###);

    Ok(())
}

#[test]
fn transpile_functions() -> Result<()> {
    // TODO: Compare to canoncial example:
    // - Window func not yet built.
    let prql = r#"
    func lag_day x = s"lag_day_todo({x})"
    func ret x = x / (lag_day x) - 1 + dividend_return
    func if_valid x = s"IF(is_valid_price, {x}, NULL)"

    from prices
    derive [
      return_total:      if_valid (ret prices_adj),
    ]
    "#;
    let result = transpile(prql)?;

    assert_snapshot!(result, @r###"
    SELECT
      IF(
        is_valid_price,
        prices_adj / lag_day_todo(prices_adj) - 1 + dividend_return,
        NULL
      ) AS return_total,
      *
    FROM
      prices
    "###);

    // Assert that the nested function has been run.
    assert!(!result.contains(&"ret prices_adj"));
    assert!(!result.contains(&"lag_day prices_adj"));

    Ok(())
}
