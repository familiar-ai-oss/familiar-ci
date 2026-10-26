#!/bin/bash
check() {
  if ! grep -q gamma "$1"; then
    echo "FAIL"
    exit 1
  fi
}
printf 'alpha\n' > /tmp/a0-sh-$$.txt
check /tmp/a0-sh-$$.txt
