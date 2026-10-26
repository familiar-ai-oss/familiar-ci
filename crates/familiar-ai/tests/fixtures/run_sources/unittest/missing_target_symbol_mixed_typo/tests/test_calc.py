import unittest

import calc
from calc import add


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 2), 5)

    def test_mul(self):
        self.assertEqual(calc.mull(2, 3), 6)
