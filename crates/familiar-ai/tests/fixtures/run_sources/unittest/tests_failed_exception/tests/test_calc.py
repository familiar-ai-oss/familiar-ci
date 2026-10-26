import unittest

from calc import parse


class CalcTest(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(parse("x1"), 1)
