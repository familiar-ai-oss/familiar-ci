from calc import add


def test_add():
    assert add(2, 2) == 4


def test_add_two():
    assert add(1, 1) == 2


def test_add_three():
    assert add(3, 3) == 6
