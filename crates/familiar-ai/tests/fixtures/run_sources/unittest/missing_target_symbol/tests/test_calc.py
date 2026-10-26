import unittest

from calc import mul


class CalcTest(unittest.TestCase):
    def test_mul(self):
        self.assertEqual(mul(2, 3), 6)
