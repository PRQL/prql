#
# This is the default module init provided automatically by Maturin.
from .prqlc import *  # ty: ignore[import-not-found]

__doc__ = prqlc.__doc__
if hasattr(prqlc, "__all__"):
    __all__ = prqlc.__all__
