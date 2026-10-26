import unittest

import calc


class CalcTest(unittest.TestCase):
    def test_mul(self):
        self.assertEqual(calc.mul(2, 3), 6)
