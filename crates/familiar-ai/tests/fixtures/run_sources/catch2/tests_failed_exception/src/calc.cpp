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
