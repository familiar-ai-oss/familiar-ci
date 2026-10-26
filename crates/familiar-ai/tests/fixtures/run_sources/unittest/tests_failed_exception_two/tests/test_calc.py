import unittest

from calc import parse


class CalcTest(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(parse("x1"), 1)

    def test_parse_again(self):
        self.assertEqual(parse("y2"), 2)
