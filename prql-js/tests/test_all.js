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

    it("should compile to dialect", () => {
      const opts = new prql.CompileOptions();
      opts.target = "sql.mssql";
      opts.format = false;
      opts.signature_comment = false;

      const res = prql.compile("from a | take 10", opts);
      assert.equal(res, "SELECT TOP (10) * FROM a");
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

  describe("CompileOptions", () => {
    it("should be able to create from constructor", () => {
      const opts = new prql.CompileOptions();

      opts.target = "sql.sqlite";
      assert.equal(opts.target, "sql.sqlite");
    });

    it("should fallback to generic dialect", () => {
      const opts = new prql.CompileOptions();

      opts.target = "sql.not_existing";
      const res = prql.compile("from a", opts);

      // target should appear in signature comment
      assert(res.includes("target:sql.generic"));
    });
  });
});
