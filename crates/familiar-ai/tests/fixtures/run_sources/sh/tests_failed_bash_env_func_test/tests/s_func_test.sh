#!/bin/bash
set -e
assert_eq() { [ "$1" = "$2" ]; }
assert_eq "$(echo 2)" 3
echo never
