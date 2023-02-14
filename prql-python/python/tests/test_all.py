import prql_python as prql


def test_all():
    """
    Test the basic python functions

    Because the AST was in flux, we only test these don't throw exceptions. But we
    should write more tests at some point.
    """

    prql_query = "from employee"

    res = prql.prql_to_pl(prql_query)
    assert res is not None

    res = prql.pl_to_rq(res)
    assert res is not None

    res = prql.rq_to_sql(res)
    assert res is not None

    assert prql.__version__ is not None

    # Example from readme
    prql_query = """
        from employees
        join salaries [==emp_id]
        group [employees.dept_id, employees.gender] (
        aggregate [
            avg_salary = average salaries.salary
        ]
        )
    """

    options = prql.CompileOptions(
        format=True, signature_comment=True, target="sql.postgres"
    )

    assert prql.compile(prql_query)
    assert prql.compile(prql_query, options)


def test_compile_options():
    """
    Test the CompileOptions
    """
    query_mssql = "prql target:sql.mssql\nfrom a | take 3"
    options = prql.CompileOptions(format=False, signature_comment=False, target="foo")

    assert prql.compile(query_mssql).startswith("SELECT\n  TOP (3) *\nFROM\n  a")
    # TODO: This should be unknown target error?
    assert prql.compile(query_mssql, options) == "SELECT * FROM a LIMIT 3"
