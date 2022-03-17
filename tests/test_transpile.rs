use insta::assert_snapshot;
use prql::transpile;
use prql::Result;

#[test]
fn parse_transpile() -> Result<()> {
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

    let prql = r#"
    func lag_day x = s"lag_day_todo({x})"
    func ret x = x / (lag_day x) - 1 + dividend_return
    func excess x = (x - interest_rate) / 252
    func if_valid x = s"IF(is_valid_price, {x}, NULL)"

    from prices
    derive [
      return_total:      if_valid (ret prices_adj),
      return_usd:        if_valid (ret prices_usd),
      return_excess:     excess return_total,
      return_usd_excess: excess return_usd,
    ]
    select [
      date,
      sec_id,
      return_total,
      return_usd,
      return_excess,
      return_usd_excess,
    ]
    "#;

    // TODO: Compare to canoncial example:
    // - Window func not yet built.
    // - Inline pipeline not working.
    // - Function-in-function not working (i.e. lag_day is unreferenced).
    assert_snapshot!(transpile(prql)?, @r###"
    SELECT
      date,
      sec_id,
      IF(is_valid_price, ret prices_adj, NULL),
      IF(is_valid_price, ret prices_usd, NULL),
      IF(is_valid_price, ret prices_adj, NULL) - interest_rate / 252,
      IF(is_valid_price, ret prices_usd, NULL) - interest_rate / 252
    FROM
      prices
    "###);

    Ok(())
}
