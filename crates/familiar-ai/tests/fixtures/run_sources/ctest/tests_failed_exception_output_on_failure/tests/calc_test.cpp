#include <cstdio>

#include "calc.h"

int main() {
    int got = parse("x1");
    std::printf("parsed %d\n", got);
    return got == 1 ? 0 : 1;
}
