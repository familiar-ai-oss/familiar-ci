import warnings

from calc import add


def test_add():
    warnings.warn("careful", UserWarning)
    assert add(2, 2) == 4
