import pytest

import yaml


@pytest.fixture()
def example_queries():
    book_path = "../website/content/_index.md"
    with open(book_path, "r") as f:
        book = f.read()
    book_yaml = yaml.safe_load(book.replace("---", ""))
    showcase_section = book_yaml["showcase_section"]["examples"]
    return showcase_section
