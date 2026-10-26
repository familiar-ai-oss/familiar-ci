import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 2), 5)

    def test_add_again(self):
        self.assertEqual(add(1, 1), 3)
