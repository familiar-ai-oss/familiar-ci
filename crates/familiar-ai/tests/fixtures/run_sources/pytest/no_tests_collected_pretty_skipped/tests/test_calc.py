import pytest
from calc import add


@pytest.mark.skip(reason="later")
def test_add():
    assert add(2, 2) == 4
