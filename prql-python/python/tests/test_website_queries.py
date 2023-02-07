import re

import prql_python as prql


def normalize(sql: str) -> str:
    """helper function to remove SQL comments and extra whitespace"""
    comment_regex = re.compile(r"--.*$")
    whitespace_regex = re.compile(r"\s?\s+")
    no_comment = comment_regex.sub(string=sql, repl="")
    return whitespace_regex.sub(string=no_comment, repl=" ").strip(" ")


def test_all_examples(example_queries):
    """Compile and compare each example PRQL query to the expected SQL"""
    for query in example_queries:
        compiled = prql.compile(query["prql"])
        compiled_normalized = normalize(compiled)
        truth_normalized = normalize(query["sql"])
        assert (
            truth_normalized == compiled_normalized
        ), f"Failed on Query ID: '{query['id']}'"
