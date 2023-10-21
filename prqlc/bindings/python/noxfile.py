"""Nox session configuration."""
import os
from pathlib import Path
from typing import List

import nox
from nox.sessions import Session

VERSIONS: List[str] = [
    "3.7",
    "3.11",
]

nox.options.stop_on_first_error = False
nox.options.reuse_existing_virtualenvs = False


def _install_prql_python(session: Session) -> None:
    session.install(
        "-v",
        "--no-index",
        f"--find-links={Path('..', '..', '..', 'target', 'python')}",
        "prql_python",
    )


@nox.session(python=VERSIONS)  # type: ignore[misc]
def tests(session: Session) -> None:
    """Run the test suite with pytest."""
    print("CWD", os.getcwd())
    _install_prql_python(session)
    session.install("-v", "-r", "requirements.txt")
    session.run("pytest", str(Path("python", "tests")))


@nox.session(python=VERSIONS)  # type: ignore[misc]
def typing(session: Session) -> None:
    """Check types with mypy"""
    _install_prql_python(session)
    session.install("mypy==1.4.0")
    session.run("mypy")
