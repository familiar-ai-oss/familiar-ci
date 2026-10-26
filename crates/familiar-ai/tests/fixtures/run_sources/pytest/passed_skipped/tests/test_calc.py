import pytest
from calc import add


def test_add():
    assert add(2, 2) == 4


@pytest.mark.skip(reason="later")
def test_skipped():
    assert add(1, 1) == 2
