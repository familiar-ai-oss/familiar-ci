#include <catch2/catch_test_macros.hpp>

#include "calc.h"

TEST_CASE("Adds") {
    int got = add(2, 2)
    REQUIRE(got == 4);
}
