"""Nox session configuration."""

import os
from pathlib import Path
from typing import List

import nox
from nox.sessions import Session

VERSIONS: List[str] = [
    "3.10",
    "3.12",
]

nox.options.stop_on_first_error = False
nox.options.reuse_existing_virtualenvs = False


def _install_prqlc(session: Session) -> None:
    session.install(
        "-v",
        # We'd like to prevent `prqlc` from being installed from PyPI, but we do
        # want to install its dependencies from there, and currently there's no way in
        # plain pip of doing that (https://github.com/pypa/pip/issues/11440).
        # "--no-index",
        f"--find-links={Path('..', '..', '..', 'target', 'python')}",
        "prqlc[dev]",
    )


@nox.session(python=VERSIONS)  # type: ignore[misc]
def tests(session: Session) -> None:
    """Run the test suite with pytest."""
    print("CWD", os.getcwd())
    _install_prqlc(session)
    session.run("pytest", str(Path("python", "tests")))


@nox.session(python=VERSIONS)  # type: ignore[misc]
def typing(session: Session) -> None:
    """Check types with mypy"""
    _install_prqlc(session)
    session.run("mypy")
