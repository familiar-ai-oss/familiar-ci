from calc import parse


def test_parse():
    assert parse("x1") == 1


def test_parse_again():
    assert parse("y2") == 2
