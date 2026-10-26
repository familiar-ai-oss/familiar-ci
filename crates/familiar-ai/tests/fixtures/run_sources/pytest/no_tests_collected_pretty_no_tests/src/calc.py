def add(a, b):
    return a + b


def parse(s):
    if not s.isdigit():
        raise ValueError(f"not a number: {s!r}")
    return int(s)
