import os
import signal

from calc import add


def test_add():
    assert add(2, 2) == 4


def test_crash():
    os.kill(os.getpid(), signal.SIGSEGV)
