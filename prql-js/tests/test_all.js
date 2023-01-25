const assert = require("assert");
const expect = require("chai").expect;
const prql = require("../dist/node");
const employee_prql = `from employees
join salaries [==emp_no]
group [employees.emp_no, employees.gender] (
  aggregate [
    emp_salary = average salaries.salary
  ]
)
join de=dept_emp [==emp_no]
join dm=dept_manager [
  (dm.dept_no == de.dept_no) and s"(de.from_date, de.to_date) OVERLAPS (dm.from_date, dm.to_date)"
]
group [dm.emp_no, gender] (
  aggregate [
    salary_avg = average emp_salary,
    salary_sd = stddev emp_salary
  ]
)
derive mng_no = emp_no
join managers=employees [==emp_no]
derive mng_name = s"managers.first_name || ' ' || managers.last_name"
select [mng_name, managers.gender, salary_avg, salary_sd]`;

describe("prql-js", () => {
  describe("compile", () => {
    it("should return valid sql from valid prql", () => {
      const sql = prql.compile(employee_prql);
      assert(
        sql.trim().toLowerCase().startsWith("with") ||
          sql.trim().toLowerCase().startsWith("select")
      );
    });

    it("should throw an error on invalid prql", () => {
      expect(() =>
        prql.compile("Mississippi has four S’s and four I’s.")
      ).to.throw("Error");
    });
  });

  describe("prql_to_pl", () => {
    it("should return valid json from valid prql", () => {
      const json = JSON.parse(prql.prql_to_pl(employee_prql));
      assert.equal(json.length, 1);
    });

    it("should throw an error on invalid prql", () => {
      expect(() => prql.prql_to_pl("Answer: T-H-A-T!")).to.throw("Error");
    });
  });

  describe("SQLCompileOptions", () => {
    it("should be able to create from default_compile_options", () => {
      const opts = prql.default_compile_options();
      expect(() => {
        opts.dialect = prql.Dialect.Sqlite;
        assert.equal(opts.dialect, prql.Dialect.Sqlite);
      });
    });

    it("should be able to create from constructor", () => {
      const opts = new prql.SQLCompileOptions();
      expect(() => {
        opts.dialect = prql.Dialect.Sqlite;
        assert.equal(opts.dialect, prql.Dialect.Sqlite);
      });
    })
  });
});
