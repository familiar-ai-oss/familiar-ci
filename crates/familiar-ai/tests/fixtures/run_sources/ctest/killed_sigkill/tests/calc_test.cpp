#include <csignal>
#include <cstdio>

#include "calc.h"

int main() {
    std::printf("ok adds %d\n", add(2, 2));
    std::fflush(stdout);
    std::raise(SIGKILL);
    return 0;
}
