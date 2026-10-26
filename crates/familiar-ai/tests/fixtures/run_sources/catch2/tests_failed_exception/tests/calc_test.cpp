#include <catch2/catch_test_macros.hpp>

#include "calc.h"

TEST_CASE("Parses") {
    REQUIRE(parse("x1") == 1);
}
