#include <gtest/gtest.h>

#include "calc.h"

TEST(Calc, Adds) {
    int got = add(2, 2)
    EXPECT_EQ(got, 4);
}
