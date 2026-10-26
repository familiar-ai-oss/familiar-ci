. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
TC='cmake -S . -B build && cmake --build build && ctest --test-dir build'
base() {
  fx ctest "$1"
  mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
enable_testing()
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
add_executable(calc_test tests/calc_test.cpp)
target_link_libraries(calc_test calc)
add_test(NAME calc_test COMMAND calc_test)
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
  setpre 'rm -rf build'
  setver 'ctest --version | head -1; cmake --version | head -1; c++ --version | head -1'
  setcmd "$TC"
}
okt() { mk tests/calc_test.cpp <<'X'
#include <cstdio>

#include "calc.h"

int main() {
    int got = add(2, 2);
    if (got != 4) {
        std::fprintf(stderr, "FAIL adds: expected 4, got %d\n", got);
        return 1;
    }
    std::printf("ok adds\n");
    return 0;
}
X
}
failt() { mk tests/calc_test.cpp <<'X'
#include <cstdio>

#include "calc.h"

int main() {
    int got = add(2, 2);
    if (got != 5) {
        std::fprintf(stderr, "tests/calc_test.cpp:7: FAIL adds: expected 5, got %d\n", got);
        return 1;
    }
    std::printf("ok adds\n");
    return 0;
}
X
}
exct() { mk tests/calc_test.cpp <<'X'
#include <cstdio>

#include "calc.h"

int main() {
    int got = parse("x1");
    std::printf("parsed %d\n", got);
    return got == 1 ? 0 : 1;
}
X
}
base passed; okt; rec
base tests_failed; failt; rec
base tests_failed_output_on_failure; failt; setcmd "$(echo "$TC" | sed "s/ctest --test-dir/CTEST_OUTPUT_ON_FAILURE=1 ctest --test-dir/")"; rec
base tests_failed_exception; exct; rec
base tests_failed_exception_output_on_failure; exct; setcmd "$(echo "$TC" | sed "s/ctest --test-dir/CTEST_OUTPUT_ON_FAILURE=1 ctest --test-dir/")"; rec
base collection_error; mk tests/calc_test.cpp <<'X'
#include "calc.h"

int main() {
    int got = add(2, 2)
    return got == 4 ? 0 : 1;
}
X
rec
base missing_target_symbol; mk tests/calc_test.cpp <<'X'
#include "calc.h"

int main() {
    int got = mul(2, 3);
    return got == 6 ? 0 : 1;
}
X
rec
base missing_target_symbol_link; mk tests/calc_test.cpp <<'X'
#include "calc.h"

int mul(int a, int b);

int main() {
    int got = mul(2, 3);
    return got == 6 ? 0 : 1;
}
X
rec
base no_tests_collected_file; okt; sed -i '/add_test/d' CMakeLists.txt; rec
base no_tests_collected_filter; okt; setcmd "$TC -R nomatch"; rec
base setup_error; okt; mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
enable_testing()
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
add_executable(calc_test tests/calc_test.cpp)
target_link_libraries(calc_test calc)
add_test(NAME db_setup COMMAND sh -c "echo 'db down' >&2; exit 1")
set_tests_properties(db_setup PROPERTIES FIXTURES_SETUP db)
add_test(NAME calc_test COMMAND calc_test)
set_tests_properties(calc_test PROPERTIES FIXTURES_REQUIRED db)
X
rec
base usage_error; okt; setcmd "$TC --nosuchflag"; rec
base environment_error_cmd_not_found; okt; setcmd 'cmake -S . -B build && cmake --build build && ctestx --test-dir build'; rec
base environment_error_missing_dep; okt; mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
enable_testing()
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
add_test(NAME calc_tool_test COMMAND nosuch_tool --check)
X
rec
base runner_crash; okt; mkdir -p broken; mk broken/CTestTestfile.cmake <<'X'
add_test([=[calc_test]=] "calc_test"
X
setcmd 'ctest --test-dir broken'; setpre 'true'; rec
for k in segv segv_shell abort sigkill; do
  base killed_$k
  case $k in
    segv|segv_shell) sig=SIGSEGV;; abort) sig=SIGABRT;; sigkill) sig=SIGKILL;;
  esac
  mk tests/calc_test.cpp <<X
#include <csignal>
#include <cstdio>

#include "calc.h"

int main() {
    std::printf("ok adds %d\n", add(2, 2));
    std::fflush(stdout);
    std::raise($sig);
    return 0;
}
X
  [ $k = segv_shell ] && setcmd "$TC; exit \$?"
  rec
done
for o in tests_failed tests_failed_exception; do
  [ -d "$R/ctest/$o" ] && [ -z "$ONLY" -o -n "$(printf ' %s ' "$ONLY" | grep " $o ")" ] && printf '%s\n' 'mkdir -p report && cp build/Testing/Temporary/LastTest.log report/ 2>/dev/null; true' > "$R/ctest/$o/post.sh"
done
# big_run: 600 tests = 5 test executables (tests/test_partN.cpp) x 120 add_test
# registrations each (the index is the argument); exactly one failure, in the
# middle file (part2, index 60). Default command; then -V (big_run_verbose).
bigct() {
  base "$1"
  mk CMakeLists.txt <<'X'
cmake_minimum_required(VERSION 3.20)
project(calc CXX)
enable_testing()
add_library(calc src/calc.cpp)
target_include_directories(calc PUBLIC src)
foreach(part RANGE 0 4)
  add_executable(test_part${part} tests/test_part${part}.cpp)
  target_link_libraries(test_part${part} calc)
  foreach(i RANGE 0 119)
    add_test(NAME part${part}_add_${i} COMMAND test_part${part} ${i})
  endforeach()
endforeach()
X
  python3 - <<'X'
import os
os.makedirs('tests', exist_ok=True)
for f in range(5):
    want = '(i == 60) ? -1 : i + 1' if f == 2 else 'i + 1'
    src = ('#include <cstdio>\n#include <cstdlib>\n\n#include "calc.h"\n\n'
           'int main(int argc, char** argv) {\n'
           '    int i = argc > 1 ? std::atoi(argv[1]) : 0;\n'
           '    int want = %s;\n'
           '    int got = add(i, 1);\n'
           '    if (got != want) {\n'
           '        std::fprintf(stderr, "tests/test_part%d.cpp:11: FAIL add %%d: expected %%d, got %%d\\n", i, want, got);\n'
           '        return 1;\n'
           '    }\n'
           '    std::printf("ok add %%d\\n", i);\n'
           '    return 0;\n'
           '}\n') % (want, f)
    open('tests/test_part%d.cpp' % f, 'w').write(src)
X
}
bigct big_run; rec
[ -d "$R/ctest/big_run" ] && [ -z "$ONLY" -o -n "$(printf ' %s ' "$ONLY" | grep " big_run ")" ] && printf '%s\n' 'mkdir -p report && cp build/Testing/Temporary/LastTest.log report/ 2>/dev/null; true' > "$R/ctest/big_run/post.sh"
bigct big_run_verbose; setcmd "$TC -V"; rec
# H4 (lane h4cppphp, 2026-10-19): ctest -Q / --quiet prints nothing at any exit (cmCTest.cxx: every line sits under `if (!Quiet)`)
QTC="$TC -Q"
base passed_quiet; okt; setcmd "$QTC"; rec
base tests_failed_quiet; failt; setcmd "$QTC"; rec
base no_tests_collected_quiet; okt; sed -i '/add_test/d' CMakeLists.txt; setcmd "$QTC"; rec
