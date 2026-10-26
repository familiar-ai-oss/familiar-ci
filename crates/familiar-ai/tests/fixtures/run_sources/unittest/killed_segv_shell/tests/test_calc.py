import os
import signal
import unittest

from calc import add


class CalcTest(unittest.TestCase):
    def test_add(self):
        self.assertEqual(add(2, 2), 4)

    def test_crash(self):
        os.kill(os.getpid(), signal.SIGSEGV)
