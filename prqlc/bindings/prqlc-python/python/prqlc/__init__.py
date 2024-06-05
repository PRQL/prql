# ruff: noqa: F403, F405
#
# This is the default module init provided automatically by Maturin.
from .prqlc import *

__doc__ = prqlc.__doc__
if hasattr(prqlc, "__all__"):
    __all__ = prqlc.__all__
