#include <gtest/gtest.h>
#include <csignal>

#include "calc.h"

TEST(Calc, Adds) {
    EXPECT_EQ(add(2, 2), 4);
}

TEST(Calc, Crashes) {
    std::raise(SIGSEGV);
}
