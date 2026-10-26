import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 2), 4)

    @unittest.skip("later")
    def test_skipped(self):
        self.assertEqual(add(1, 1), 2)
