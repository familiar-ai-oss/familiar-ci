#include <catch2/catch_test_macros.hpp>
#include <csignal>

#include "calc.h"

TEST_CASE("Adds") {
    REQUIRE(add(2, 2) == 4);
}

TEST_CASE("Crashes") {
    std::raise(SIGSEGV);
}
