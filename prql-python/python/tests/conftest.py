import pytest

import yaml


@pytest.fixture()
def example_queries():
    website_path = "../website/content/_index.md"
    with open(website_path, "r") as f:
        website = f.read()
    website_yaml = yaml.safe_load(website.replace("---", ""))
    showcase_section = website_yaml["showcase_section"]["examples"]
    return showcase_section
