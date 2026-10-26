#include <gtest/gtest.h>

#include "calc.h"

TEST(Calc, Multiplies) {
    EXPECT_EQ(mul(2, 3), 6);
}
