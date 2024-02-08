import assert from "assert";
import { expect } from "chai";
import prql from "../dist/node/prql_js.js";

const employee_prql = `from employees
join salaries (==emp_no)
group {employees.emp_no, employees.gender} (
  aggregate {
    emp_salary = average salaries.salary
  }
)
join de=dept_emp (==emp_no)
join dm=dept_manager (
  (dm.dept_no == de.dept_no) && s"(de.from_date, de.to_date) OVERLAPS (dm.from_date, dm.to_date)"
)
group {dm.emp_no, gender} (
  aggregate {
    salary_avg = average emp_salary,
    salary_sd = stddev emp_salary
  }
)
derive mng_no = emp_no
join managers=employees (==emp_no)
derive mng_name = s"managers.first_name || ' ' || managers.last_name"
select {mng_name, managers.gender, salary_avg, salary_sd}`;

describe("prql-js", () => {
  describe("compile", () => {
    it("should return valid sql from valid prql", () => {
      const sql = prql.compile(employee_prql);
      assert(
        sql.trim().toLowerCase().startsWith("with") ||
          sql.trim().toLowerCase().startsWith("select"),
      );
    });

    it("should throw an error on invalid prql", () => {
      expect(() =>
        prql.compile("Mississippi has four Ss and four Is."),
      ).to.throw("Error");
    });

    it("should compile to dialect", () => {
      const opts = new prql.CompileOptions();
      opts.target = "sql.mssql";
      opts.format = false;
      opts.signature_comment = false;

      const res = prql.compile("from a | take 10", opts);
      assert.equal(
        res,
        "SELECT * FROM a ORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH FIRST 10 ROWS ONLY",
      );
    });

    it("CompileOptions should be preferred and should ignore target in header", () => {
      const opts = new prql.CompileOptions();
      opts.target = "sql.mssql";
      opts.format = false;
      opts.signature_comment = true;

      const res = prql.compile(
        "prql target:sql.sqlite\nfrom a | take 10",
        opts,
      );
      assert(
        res.includes(
          "SELECT * FROM a ORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH FIRST 10 ROWS ONLY",
        ),
      );
      assert(res.includes("target:sql.mssql"));
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

    it("should fallback to the target in header", () => {
      const opts = new prql.CompileOptions();

      opts.target = "sql.any";
      const res = prql.compile("prql target:sql.mssql\nfrom a | take 1", opts);
      assert(res.includes("1 ROWS ONLY"));
    });
  });

  describe("get_targets", () => {
    it("return a list of targets", () => {
      const targets = new prql.get_targets();
      assert(targets.length > 0);
      assert(targets.includes("sql.sqlite"));
    });
  });

  describe("compile error", () => {
    it("should contain json", () => {
      try {
        prql.compile("from x | select a | select b");
      } catch (error) {
        const errorMessages = JSON.parse(error.message).inner;

        assert(errorMessages.length > 0);
        assert(errorMessages[0].display.includes("\n"));
        assert(!errorMessages[0].reason.includes("\n"));
      }
    });

    it("should contain error code", () => {
      try {
        prql.compile("let a = (from x)");
      } catch (error) {
        const errorMessages = JSON.parse(error.message).inner;

        assert(errorMessages[0].code == "E0001");
      }
    });
  });
});
