import calc
from calc import add


def test_add():
    assert add(2, 2) == 5


def test_mul():
    assert calc.mull(2, 3) == 6
