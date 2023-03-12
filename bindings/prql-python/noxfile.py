"""Nox session configuration."""
import os
from pathlib import Path
from typing import List

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
    print("CWD", os.getcwd())
    session.install(
        "-v", "--no-index", f"--find-links={Path('..', 'dist')}", "prql_python"
    )
    session.install("-v", "-r", "requirements.txt")
    session.run("pytest", str(Path("python", "tests")))
