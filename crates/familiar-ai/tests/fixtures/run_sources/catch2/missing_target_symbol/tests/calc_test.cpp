#include <catch2/catch_test_macros.hpp>

#include "calc.h"

TEST_CASE("Multiplies") {
    REQUIRE(mul(2, 3) == 6);
}
