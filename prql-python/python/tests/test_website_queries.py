import re

import prql_python as prql


def normalize(sql: str) -> str:
    """helper function to remove SQL comments and extra whitespace"""
    comment_regex = re.compile(r"--.*$")
    whitespace_regex = re.compile(r"\s?\s+")
    no_comment = comment_regex.sub(string=sql, repl="")
    return whitespace_regex.sub(string=no_comment, repl=" ").strip(" ")


def test_examples_expected(example_queries):
    """
    This test ensures that all the statements mentioned in the book are tested.
    If it fails, it means we may have added or removed a test from the book without
    updating this test suite.
    """
    website_examples = [test["id"] for test in example_queries].sort()
    # update this list after adding a new test.
    EXPECTED_EXAMPLES = [
        "basics",
        "friendly-syntax",
        "dates",
        "orthogonal",
        "f-strings",
        "windows",
        "functions",
        "top-n",
        "s-string",
        "joins",
        "null-handling",
        "dialects",
    ].sort()
    assert EXPECTED_EXAMPLES == website_examples


def test_basics(example_queries):
    for item in example_queries:
        if item["id"] == "basics":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_friendly_syntax(example_queries):
    for item in example_queries:
        if item["id"] == "friendly-syntax":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_dates(example_queries):
    for item in example_queries:
        if item["id"] == "dates":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_orthogonal(example_queries):
    for item in example_queries:
        if item["id"] == "orthogonal":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_f_strings(example_queries):
    for item in example_queries:
        if item["id"] == "f-strings":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_windows(example_queries):
    for item in example_queries:
        if item["id"] == "windows":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_functions(example_queries):
    for item in example_queries:
        if item["id"] == "functions":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_top_n(example_queries):
    for item in example_queries:
        if item["id"] == "top-n":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_s_string(example_queries):
    for item in example_queries:
        if item["id"] == "s-string":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_joins(example_queries):
    for item in example_queries:
        if item["id"] == "joins":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_null_handling(example_queries):
    for item in example_queries:
        if item["id"] == "null-handling":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized


def test_dialects(example_queries):
    for item in example_queries:
        if item["id"] == "dialects":
            compiled = prql.compile(item["prql"])
            compiled_normalized = normalize(compiled)
            truth_normalized = normalize(item["sql"])
    assert truth_normalized == compiled_normalized
