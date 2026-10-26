#include "calc.h"

int mul(int a, int b);

int main() {
    int got = mul(2, 3);
    return got == 6 ? 0 : 1;
}
