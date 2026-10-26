. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/cpp-common.sh
FW=googletest
gen_cpp
cbase setup_error; mk tests/calc_test.cpp <<'X'
#include <gtest/gtest.h>

#include <stdexcept>

#include "calc.h"

class CalcDb : public ::testing::Test {
protected:
    void SetUp() override { throw std::runtime_error("db down"); }
};

TEST_F(CalcDb, Adds) {
    EXPECT_EQ(add(2, 2), 4);
}
X
rec
cbase setup_error_suite; mk tests/calc_test.cpp <<'X'
#include <gtest/gtest.h>

#include "calc.h"

class CalcDb : public ::testing::Test {
protected:
    static void SetUpTestSuite() { FAIL() << "db down"; }
};

TEST_F(CalcDb, Adds) {
    EXPECT_EQ(add(2, 2), 4);
}
X
rec
cbase usage_error_gtest_flag; mk tests/calc_test.cpp <<'X'
#include <gtest/gtest.h>

#include "calc.h"

TEST(Calc, Adds) {
    EXPECT_EQ(add(2, 2), 4);
}
X
setcmd "$TC --gtest_nosuchflag"; rec
cbase via_ctest_tests_failed; mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
set(CMAKE_CXX_STANDARD 17)
enable_testing()
find_package(GTest CONFIG REQUIRED)
include(GoogleTest)
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
add_executable(calc_test tests/calc_test.cpp)
target_link_libraries(calc_test calc GTest::gtest_main)
gtest_discover_tests(calc_test)
X
mk tests/calc_test.cpp <<'X'
#include <gtest/gtest.h>

#include "calc.h"

TEST(Calc, Adds) {
    EXPECT_EQ(add(2, 2), 5);
}

TEST(Calc, Parses) {
    EXPECT_EQ(parse("x1"), 1);
}
X
setcmd "$CFG && CTEST_OUTPUT_ON_FAILURE=1 ctest --test-dir build"; rec
# big_run: 600 TESTs in 5 test files (tests/part0_test.cpp .. part4_test.cpp,
# one suite each: CalcPart0..CalcPart4, 120 tests per suite) linked into the one
# calc_test binary; exactly one failing assertion, in the middle file
# (CalcPart2.Add060). Default command (gtest has no verbose mode: the default
# already prints every test).
cbase big_run
sed -i 's#^add_executable(calc_test tests/calc_test.cpp)$#add_executable(calc_test tests/part0_test.cpp tests/part1_test.cpp tests/part2_test.cpp tests/part3_test.cpp tests/part4_test.cpp)#' CMakeLists.txt
python3 - <<'X'
import os
os.makedirs('tests', exist_ok=True)
for f in range(5):
    s = '#include <gtest/gtest.h>\n\n#include "calc.h"\n\n'
    for i in range(120):
        want = -1 if (f == 2 and i == 60) else i + 1
        s += 'TEST(CalcPart%d, Add%03d) {\n    EXPECT_EQ(add(%d, 1), %d);\n}\n\n' % (f, i, i, want)
    open('tests/part%d_test.cpp' % f, 'w').write(s)
X
rec
# H4 (lane h4cppphp, 2026-10-19): --gtest_brief prints no `listed below:` blocks and no `N FAILED TEST` line
g1() { mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' "$1"; printf '}\n'; } > tests/calc_test.cpp; }
g2() { mkdir -p tests; { hdr; printf '\n#include "calc.h"\n\n'; tcase Adds; eq 'add(2, 2)' 4; printf '}\n\nTEST(Calc, Later) {\n    GTEST_SKIP() << "later";\n}\n'; } > tests/calc_test.cpp; }
cbase passed_brief; g1 4; setcmd "$TC --gtest_brief=1"; rec
cbase passed_brief_skipped; g2; setcmd "$TC --gtest_brief=1"; rec
cbase passed_skipped; g2; rec
cbase tests_failed_brief; g1 5; setcmd "$TC --gtest_brief=1"; rec
cbase tests_failed_brief_no_time; g1 5; setcmd "$TC --gtest_brief=1 --gtest_print_time=0"; rec
