#include <gtest/gtest.h>

#include "calc.h"

TEST(Calc, Parses) {
    EXPECT_EQ(parse("x1"), 1);
}
