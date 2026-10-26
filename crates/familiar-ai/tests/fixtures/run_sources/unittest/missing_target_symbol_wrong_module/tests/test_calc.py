import os
import unittest


class CalcTest(unittest.TestCase):
    def test_mul(self):
        self.assertEqual(os.mul(2, 3), 6)
