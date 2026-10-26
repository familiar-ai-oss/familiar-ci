. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
# FW=googletest|catch2
CFG='cmake -S . -B build -DCMAKE_PREFIX_PATH=$PWD/../../.cache/cpp-prefix && cmake --build build'
TC="$CFG && ./build/calc_test"
cbase() {
  fx "$FW" "$1"
  if [ "$FW" = googletest ]; then
  mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
set(CMAKE_CXX_STANDARD 17)
find_package(GTest CONFIG REQUIRED)
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
add_executable(calc_test tests/calc_test.cpp)
target_link_libraries(calc_test calc GTest::gtest_main)
X
  setver 'echo "googletest $(sed -n "s/.*PACKAGE_VERSION \"\([0-9.]*\)\".*/\1/p" ../../.cache/cpp-prefix/lib*/cmake/GTest/GTestConfigVersion.cmake | head -1)"; c++ --version | head -1'
  else
  mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
set(CMAKE_CXX_STANDARD 17)
find_package(Catch2 3 REQUIRED)
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
add_executable(calc_test tests/calc_test.cpp)
target_link_libraries(calc_test calc Catch2::Catch2WithMain)
X
  setver 'echo "Catch2 $(sed -n "s/.*PACKAGE_VERSION \"\([0-9.]*\)\".*/\1/p" ../../.cache/cpp-prefix/lib*/cmake/Catch2/Catch2ConfigVersion.cmake | head -1)"; c++ --version | head -1'
  fi
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
  mkdir -p tests
  setpre 'rm -rf build'
  setcmd "$TC"
}
# t NAME BODY-LINES... : write tests/calc_test.cpp with one test (framework-specific wrapper)
hdr() { if [ "$FW" = googletest ]; then printf '#include <gtest/gtest.h>\n'; else printf '#include <catch2/catch_test_macros.hpp>\n'; fi; }
tcase() { if [ "$FW" = googletest ]; then printf 'TEST(Calc, %s) {\n' "$1"; else printf 'TEST_CASE("%s") {\n' "$1"; fi; }
eq() { if [ "$FW" = googletest ]; then printf '    EXPECT_EQ(%s, %s);\n' "$1" "$2"; else printf '    REQUIRE(%s == %s);\n' "$1" "$2"; fi; }
gen_cpp() {
cbase passed; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n'; } > tests/calc_test.cpp; rec
cbase tests_failed; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 5; printf '}\n'; } > tests/calc_test.cpp; rec
cbase tests_failed_exception; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Parses; eq 'parse("x1")' 1; printf '}\n'; } > tests/calc_test.cpp; rec
cbase collection_error; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; printf '    int got = add(2, 2)\n'; eq got 4; printf '}\n'; } > tests/calc_test.cpp; rec
cbase missing_target_symbol; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Multiplies; eq 'mul(2, 3)' 6; printf '}\n'; } > tests/calc_test.cpp; rec
cbase no_tests_collected_file; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\nstatic int helper() { return add(1, 1); }\n'; } > tests/calc_test.cpp; rec
cbase no_tests_collected_filter; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n'; } > tests/calc_test.cpp
if [ "$FW" = googletest ]; then setcmd "$TC --gtest_filter=Nomatch*"; else setcmd "$TC nomatch"; fi; rec
cbase usage_error; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n'; } > tests/calc_test.cpp; setcmd "$TC --nosuchflag"; rec
cbase environment_error_cmd_not_found; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n'; } > tests/calc_test.cpp; setcmd "$CFG && ./build/calc_testx"; rec
cbase environment_error_missing_dep; mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n'; } > tests/calc_test.cpp; setcmd 'cmake -S . -B build && cmake --build build && ./build/calc_test'; rec
for k in segv segv_shell abort sigkill; do
  cbase killed_$k
  case $k in segv|segv_shell) sig=SIGSEGV;; abort) sig=SIGABRT;; sigkill) sig=SIGKILL;; esac
  mkdir -p tests; { hdr; printf '#include <csignal>\n\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n\n'; tcase Crashes; printf '    std::raise(%s);\n}\n' "$sig"; } > tests/calc_test.cpp
  [ $k = segv_shell ] && setcmd "$TC; exit \$?"
  rec
done
}
