# -*- coding: utf-8 -*-
"""Nox session configuration."""
import os
from typing import Any, List
from pathlib import Path

import nox
from nox.sessions import Session

VERSIONS: List[str] = [
    "3.7",
    "3.8",
    "3.9",
    "3.10",
]

nox.options.stop_on_first_error = False
nox.options.reuse_existing_virtualenvs = False


@nox.session(python=VERSIONS)
def tests(session: Session) -> None:
    """Run the test suite with pytest."""
    session.install("-v", f"--find-links={Path('..', 'target' + os.sep + 'wheels')}", "prql_python")
    session.install("-v", "-r", "requirements.txt")
    session.run("pytest", str(Path("python", "tests")))
