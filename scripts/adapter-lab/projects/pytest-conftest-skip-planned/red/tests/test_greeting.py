import app


def test_greet():
    assert app.greet() == "hello"


def test_greet_is_text():
    assert isinstance(app.greet(), str)
