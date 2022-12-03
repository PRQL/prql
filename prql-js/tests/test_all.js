const assert = require("assert");
const expect = require("chai").expect;
const prql = require("../dist/node");
const print = console.log;
const employee_prql = `from employees
join salaries [==emp_no]
group [emp_no, gender] (
  aggregate [
    emp_salary = average salary
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
  describe("to_sql", () => {
    it("should return valid sql from valid prql", () => {
      const sql = prql.to_sql(employee_prql);
      assert(
        sql.trim().toLowerCase().startsWith("with") ||
          sql.trim().toLowerCase().startsWith("select")
      );
    });

    it("should throw an error on invalid prql", () => {
      expect(() =>
        prql.to_sql("Mississippi has four S’s and four I’s.")
      ).to.throw("Error");
    });
  });

  describe("compile", () => {
    it("should return valid compile result for compile", () => {
      const res = prql.compile(employee_prql);
      expect(res.error).to.be.undefined;
    });

    it("should return  compile result with errors for compile", () => {
      const res = prql.compile("Can you spell that without using S or I?");
      expect(res.error).to.not.be.null;
    });
  });

  describe("to_json", () => {
    it("should return valid json from valid prql", () => {
      const js = JSON.parse(prql.to_json(employee_prql));
      assert.equal(js.length, 1);
    });

    it("should throw an error on invalid prql", () => {
      expect(() => prql.to_json("Answer: T-H-A-T!"));
    });
  });
});
