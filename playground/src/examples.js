const examples = {
  "introduction.prql": [
    "sql",
    `from employees
filter country_code == "USA"   # Each line transforms the previous result.
derive [                       # This adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost  # Variables can use other variables.
]
filter gross_cost > 0
group [title, country_code] (  # For each group use a nested pipeline
  aggregate [                  # Aggregate each group to a single row
    average salary,
    average gross_salary,
    sum salary,
    sum gross_salary,
    average gross_cost,
    sum_gross_cost = sum gross_cost,
    ct = count,
  ]
)
sort sum_gross_cost
filter ct > 200
take 20
join countries side:left [==country_code]
derive [
  always_true = true,
  db_version = s"version()",    # An S-string, which transpiles directly into SQL
]`,
  ],

  "cte-0.prql": [
    "sql",
    `table newest_employees = (
  from employees
  sort tenure
  take 50
  select [name, salary, country]
)

table average_salaries = (
  from employees
  group country (
    aggregate average_country_salary = (average salary)
  )
)

from newest_employees
join average_salaries [==country]
select [name, salary, average_country_salary]
`,
  ],

  "employees-0.prql": [
    "sql",
    `from salaries
group [emp_no] (
  aggregate [emp_salary = average salary]
)
join t=titles [==emp_no]
join dept_emp side:left [==emp_no]
group [dept_emp.dept_no, t.title] (
  aggregate [avg_salary = average emp_salary]
)
join departments [==dept_no]
select [dept_name, title, avg_salary]
`,
  ],

  "artists-0.prql": [
    "arrow",
    `from tracks
select [album_id, name, unit_price]
sort [-unit_price, name]
group album_id (
    aggregate [
    track_count = count,
    album_price = sum unit_price
    ]
)
join albums [==album_id]
group artist_id (
    aggregate [
    track_count = sum track_count,
    artist_price = sum album_price
    ]
)
join artists [==artist_id]
select [artists.name, artist_price, track_count]
sort [-artist_price]
derive avg_track_price = artist_price / track_count
`,
  ],
};
export default examples;
