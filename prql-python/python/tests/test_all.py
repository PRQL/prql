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

    assert prql.compile(prql_query)
