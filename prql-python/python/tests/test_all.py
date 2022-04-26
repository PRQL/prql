import prql_python as prql
import json


def test_all():
    """
    Test the basic python functions
    """

    # Since the AST is so in flux lets just take these dont throw exceptions
    print(dir(prql))
    dir(prql)
    print(prql)
    prql_query = "from employee"
    res = json.loads(prql.to_json(prql_query))
    assert res is not None

    res = prql.to_sql(prql_query)
    assert res is not None


if __name__ == '__main__':
    test_all()
