import lutra


def test_basic() -> None:
    res = lutra.execute_one("../../example-project", "main")

    assert (
        str(res) + "\n"
        == """\
pyarrow.RecordBatch
aid: int64
name: large_string
last_listen: large_string
----
aid: [240,14]
name: ["Pink Floyd","Apocalyptica"]
last_listen: ["2023-05-18","2023-05-16"]
"""
    )
