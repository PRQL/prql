import prql_python as prql
import json


def test_all():
    """
    Test the basic python functions
    """

    # Since the AST is so in flux lets just take these dont throw exceptions
    prql_query = "from employee"
    res = json.loads(prql.to_json(prql_query))
    assert res is not None

    res = prql.compile(prql_query)
    assert res is not None


if __name__ == '__main__':
    test_all()
