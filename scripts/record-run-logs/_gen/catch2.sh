. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/cpp-common.sh
FW=catch2
gen_cpp
cbase setup_error; mk tests/calc_test.cpp <<'X'
#include <catch2/catch_test_macros.hpp>

#include <stdexcept>

#include "calc.h"

struct Db {
    Db() { throw std::runtime_error("db down"); }
};

TEST_CASE_METHOD(Db, "Adds") {
    REQUIRE(add(2, 2) == 4);
}
X
rec
# big_run: 600 TEST_CASEs in 5 test files (tests/part0_test.cpp .. part4_test.cpp,
# 120 each) linked into the one calc_test binary; exactly one failing assertion,
# in the middle file ("add 2-60"). Default command; then -s (--success: report
# passing assertions too), Catch2's verbose mode.
bigc2() {
cbase "$1"
sed -i 's#^add_executable(calc_test tests/calc_test.cpp)$#add_executable(calc_test tests/part0_test.cpp tests/part1_test.cpp tests/part2_test.cpp tests/part3_test.cpp tests/part4_test.cpp)#' CMakeLists.txt
python3 - <<'X'
import os
os.makedirs('tests', exist_ok=True)
for f in range(5):
    s = '#include <catch2/catch_test_macros.hpp>\n\n#include "calc.h"\n\n'
    for i in range(120):
        want = -1 if (f == 2 and i == 60) else i + 1
        s += 'TEST_CASE("add %d-%d") {\n    REQUIRE(add(%d, 1) == %d);\n}\n\n' % (f, i, i, want)
    open('tests/part%d_test.cpp' % f, 'w').write(s)
X
}
bigc2 big_run; rec
bigc2 big_run_verbose; setcmd "$TC -s"; rec
# H4 (lane h4cppphp, 2026-10-19): the non-console reporters, a skip beside a pass, and Catch2 v2
c1() { mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' "$1"; printf '}\n'; } > tests/calc_test.cpp; }
for r in compact xml junit tap teamcity automake; do
  cbase passed_$r; c1 4; setcmd "$TC -r $r"; rec
  cbase tests_failed_$r; c1 5; setcmd "$TC -r $r"; rec
done
cbase passed_skipped; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n\nTEST_CASE("Later") {\n    SKIP("later");\n}\n'; } > tests/calc_test.cpp; rec
# Catch2 2.13.10 (single header): exit code = the number of failed assertions, not 42
cbase2() {
  fx catch2 "$1"
  mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
set(CMAKE_CXX_STANDARD 17)
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
add_executable(calc_test tests/main.cpp tests/calc_test.cpp)
target_include_directories(calc_test PRIVATE ../../.cache/catch2v2)
target_link_libraries(calc_test calc)
X
  mk tests/main.cpp <<'X'
#define CATCH_CONFIG_MAIN
#include <catch2/catch.hpp>
X
  mk src/calc.h <<'X'
#pragma once
#include <string>

int add(int a, int b);
int parse(const std::string& s);
X
  mk src/calc.cpp <<'X'
#include "calc.h"

#include <stdexcept>

int add(int a, int b) { return a + b; }

int parse(const std::string& s) {
    for (char c : s) {
        if (c < '0' || c > '9') {
            throw std::invalid_argument("not a number: " + s);
        }
    }
    return std::stoi(s);
}
X
  setver 'echo "Catch2 2.13.10"; c++ --version | head -1'
  setpre 'rm -rf build'
  setcmd "$TC"
}
v2() { mkdir -p tests; { printf '#include <catch2/catch.hpp>\n\n#include "calc.h"\n\n'; for t in "$@"; do printf 'TEST_CASE("Adds %s") {\n    REQUIRE(add(2, 2) == %s);\n}\n\n' "$t" "$t"; done; } > tests/calc_test.cpp; }
cbase2 passed_v2; v2 4; rec
cbase2 tests_failed_v2; v2 5; rec
cbase2 tests_failed_v2_two; v2 5 6; rec
